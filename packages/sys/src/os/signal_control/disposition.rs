use core::ffi::c_int;
use std::io;

pub(crate) fn create_signal_set<T>(
    signals: &[c_int],
    create: impl FnOnce() -> T,
    mut empty: impl FnMut(&mut T) -> c_int,
    mut add: impl FnMut(&mut T, c_int) -> c_int,
    mut last_error: impl FnMut() -> io::Error,
) -> io::Result<T> {
    let mut set = create();
    if empty(&mut set) != 0 {
        return Err(last_error());
    }
    for &signal in signals {
        if add(&mut set, signal) != 0 {
            return Err(last_error());
        }
    }
    Ok(set)
}

pub(crate) fn install_signal_disposition<T>(
    action: &mut T,
    empty_mask: impl FnOnce(&mut T) -> c_int,
    install: impl FnOnce(&T) -> c_int,
    mut last_error: impl FnMut() -> io::Error,
) -> io::Result<()> {
    if empty_mask(action) != 0 {
        return Err(last_error());
    }
    if install(action) != 0 {
        return Err(last_error());
    }
    Ok(())
}

pub(crate) fn block_control_signal_mask(result: c_int) -> io::Result<()> {
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result));
    }
    Ok(())
}
