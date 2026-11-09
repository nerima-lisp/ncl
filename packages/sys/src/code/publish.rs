use super::{CodeError, CodePtr, REGISTRY_ACTIVE, REGISTRY_PREPARED};
use crate::os::declarations;
use std::sync::atomic::Ordering;

#[cfg(not(target_os = "macos"))]
const PROT_READ: i32 = 1;
#[cfg(not(target_os = "macos"))]
const PROT_EXEC: i32 = 4;

impl CodePtr {
    fn activate_registry(&self) -> Result<(), CodeError> {
        if !self.is_published() {
            return Err(CodeError::NotPublished);
        }
        match self.registry_state.compare_exchange(
            REGISTRY_PREPARED,
            REGISTRY_ACTIVE,
            Ordering::Release,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(()),
            Err(REGISTRY_ACTIVE) => Err(CodeError::AlreadyActivated),
            Err(_) => Err(CodeError::NotRegistered),
        }
    }

    fn seal(&mut self) -> Result<(), CodeError> {
        if self.is_published() {
            return Err(CodeError::AlreadyPublished);
        }
        if self.sealed {
            return Err(CodeError::AlreadySealed);
        }
        #[cfg(not(target_os = "macos"))]
        {
            // SAFETY: the mapping is page-aligned and mapping_len is its exact length.
            let result = unsafe {
                declarations::mprotect(
                    self.ptr.as_ptr().cast(),
                    self.mapping_len,
                    PROT_READ | PROT_EXEC,
                )
            };
            if result != 0 {
                return Err(CodeError::ProtectionFailed);
            }
        }
        #[cfg(target_os = "macos")]
        // SAFETY: the current thread owns this JIT mapping and ends its write scope.
        unsafe {
            declarations::pthread_jit_write_protect_np(1);
        }
        self.sealed = true;
        Ok(())
    }

    const fn validate_cache_synchronization(&self) -> Result<(), CodeError> {
        if !self.sealed {
            return Err(CodeError::NotSealed);
        }
        if self.cache_synchronized {
            return Err(CodeError::AlreadySynchronized);
        }
        Ok(())
    }

    #[cfg(target_arch = "x86_64")]
    const fn synchronize_cache(&mut self) -> Result<(), CodeError> {
        match self.validate_cache_synchronization() {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        self.cache_synchronized = true;
        Ok(())
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn synchronize_cache(&mut self) -> Result<(), CodeError> {
        self.validate_cache_synchronization()?;
        #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
        synchronize_aarch64_linux(self.ptr.as_ptr(), self.len)?;
        #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
        // SAFETY: the sealed JIT range is live and contains `len` initialized bytes.
        unsafe {
            declarations::sys_icache_invalidate(self.ptr.as_ptr().cast(), self.len);
        }
        #[cfg(not(any(
            target_arch = "x86_64",
            all(target_arch = "aarch64", any(target_os = "linux", target_os = "macos"))
        )))]
        return Err(CodeError::UnsupportedTarget);
        self.cache_synchronized = true;
        Ok(())
    }

    fn activate(&self) -> Result<(), CodeError> {
        if self.is_published() {
            return Err(CodeError::AlreadyPublished);
        }
        if !self.sealed {
            return Err(CodeError::NotSealed);
        }
        if !self.cache_synchronized {
            return Err(CodeError::CacheNotSynchronized);
        }
        self.entry.store(self.address(), Ordering::Release);
        self.published.store(true, Ordering::Release);
        Ok(())
    }
}

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn synchronize_aarch64_linux(start: *mut u8, len: usize) -> Result<(), CodeError> {
    use core::arch::asm;

    let start = start.addr();
    let end = start.checked_add(len).ok_or(CodeError::OutOfBounds)?;
    let ctr: u64;
    // SAFETY: reading CTR_EL0 is permitted at EL0 on the supported Linux AArch64 targets.
    unsafe {
        asm!("mrs {ctr}, ctr_el0", ctr = out(reg) ctr, options(nostack));
    }
    let d_line = 4_usize << ((ctr >> 16) & 0xf);
    let i_line = 4_usize << (ctr & 0xf);
    let mut address = start & !(d_line - 1);
    while address < end {
        // SAFETY: each address is a cache-line-aligned point within the owned code range.
        unsafe {
            asm!("dc cvau, {address}", address = in(reg) address, options(nostack));
        }
        address = address.checked_add(d_line).ok_or(CodeError::OutOfBounds)?;
    }
    // SAFETY: orders the data-cache clean before instruction-cache invalidation.
    unsafe {
        asm!("dsb ish", options(nostack));
    }
    address = start & !(i_line - 1);
    while address < end {
        // SAFETY: each address is a cache-line-aligned point within the owned code range.
        unsafe {
            asm!("ic ivau, {address}", address = in(reg) address, options(nostack));
        }
        address = address.checked_add(i_line).ok_or(CodeError::OutOfBounds)?;
    }
    // SAFETY: completes instruction-cache invalidation before subsequent execution.
    unsafe {
        asm!("dsb ish", "isb sy", options(nostack));
    }
    Ok(())
}

/// Publish code after relocations and metadata have been installed.
///
/// # Errors
///
/// Returns an error when the allocation was already published or its permissions cannot be changed.
pub fn publish_code(code: &mut CodePtr) -> Result<(), CodeError> {
    seal_code(code)?;
    synchronize_code(code)?;
    activate_code(code)
}

/// Seal a code range read-execute, or restore macOS JIT write protection.
///
/// # Errors
/// Returns an error when the range was already published or the operating system rejected protection changes.
pub fn seal_code(code: &mut CodePtr) -> Result<(), CodeError> {
    code.seal()
}

/// Synchronize instruction caches after the final write and protection transition.
///
/// # Errors
/// Returns an error when the range is not sealed or cache synchronization cannot be performed.
#[cfg(target_arch = "x86_64")]
pub const fn synchronize_code(code: &mut CodePtr) -> Result<(), CodeError> {
    code.synchronize_cache()
}

/// Synchronize instruction caches after the final write and protection transition.
///
/// # Errors
/// Returns an error when the range is not sealed or cache synchronization cannot be performed.
#[cfg(not(target_arch = "x86_64"))]
pub fn synchronize_code(code: &mut CodePtr) -> Result<(), CodeError> {
    code.synchronize_cache()
}

/// Activate a sealed, synchronized code range for registry and function-cell publication.
///
/// # Errors
/// Returns an error when sealing or cache synchronization has not completed.
pub fn activate_code(code: &mut CodePtr) -> Result<(), CodeError> {
    code.activate()
}

/// Activate prepared metadata for PC lookup after executable publication.
///
/// # Errors
/// Returns an error when the code is not published, has no prepared registry entry, or is already active.
pub fn activate_code_registry(code: &CodePtr) -> Result<(), CodeError> {
    code.activate_registry()
}
