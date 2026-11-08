#[cfg(target_os = "linux")]
use core::ffi::c_long;
use std::cell::RefCell;
use std::io;

pub(crate) struct AltSignalStackOwner {
    pub(crate) memory: Option<Box<[u8]>>,
}

#[cfg(target_os = "linux")]
pub(crate) fn positive_sysconf(value: c_long, fallback: usize) -> usize {
    usize::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

pub(crate) fn required_alt_signal_stack_size(
    minimum: usize,
    recommended: usize,
    page_size: i32,
    reserve: usize,
) -> io::Result<usize> {
    let page_size = usize::try_from(page_size)
        .ok()
        .filter(|size| *size > 0)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid page size"))?;
    let base_size = minimum.max(recommended);
    let rounded_size = base_size
        .checked_add(page_size - 1)
        .map(|size| size / page_size * page_size)
        .ok_or_else(|| io::Error::other("signal stack size overflow"))?;
    rounded_size
        .checked_add(reserve)
        .ok_or_else(|| io::Error::other("signal stack size overflow"))
}

pub(crate) fn query_alternate_signal_stack(
    result: i32,
    last_error: impl FnOnce() -> io::Error,
) -> io::Result<()> {
    if result != 0 {
        return Err(last_error());
    }
    Ok(())
}

pub(crate) fn ensure_alternate_signal_stack(
    required: usize,
    current_size: usize,
    current_flags: i32,
    disabled_flag: i32,
    on_stack_flag: i32,
    register: impl FnOnce(usize) -> io::Result<usize>,
) -> io::Result<usize> {
    if current_flags & disabled_flag == 0 && current_size >= required {
        return Ok(current_size);
    }
    if current_flags & on_stack_flag != 0 {
        return Err(io::Error::from_raw_os_error(1));
    }
    register(required)
}

pub(crate) fn register_alternate_signal_stack(
    owner: &RefCell<AltSignalStackOwner>,
    required: usize,
    register: impl FnOnce(*mut u8, usize) -> io::Result<()>,
) -> io::Result<usize> {
    let mut owner = owner.try_borrow_mut().map_err(|_| {
        io::Error::new(
            io::ErrorKind::WouldBlock,
            "alternate signal stack is borrowed",
        )
    })?;
    let mut memory = vec![0_u8; required].into_boxed_slice();
    register(memory.as_mut_ptr(), memory.len())?;
    owner.memory = Some(memory);
    Ok(required)
}

pub(crate) fn map_alternate_signal_stack_tls<T, E>(
    result: Result<io::Result<T>, E>,
) -> io::Result<T> {
    result.map_err(|_| io::Error::other("alternate signal stack TLS unavailable"))?
}

pub(crate) fn release_alternate_signal_stack(
    memory: Option<Box<[u8]>>,
    current: impl FnOnce() -> io::Result<(usize, i32)>,
    disabled_flag: i32,
    on_stack_flag: i32,
    disable: impl FnOnce() -> io::Result<()>,
    retain: impl FnOnce(Box<[u8]>),
) {
    let Some(memory) = memory else {
        return;
    };
    let Ok((current_address, flags)) = current() else {
        retain(memory);
        return;
    };
    if current_address != memory.as_ptr().addr() {
        return;
    }
    if flags & on_stack_flag != 0 {
        retain(memory);
        return;
    }
    if flags & disabled_flag == 0 && disable().is_err() {
        retain(memory);
    }
}
