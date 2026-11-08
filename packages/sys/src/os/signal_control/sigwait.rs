use core::ffi::c_int;
use std::io;

pub(crate) fn wait_signal_set<T>(
    set: &T,
    mut wait: impl FnMut(&T, &mut c_int) -> c_int,
) -> io::Result<c_int> {
    loop {
        let mut signal = 0;
        let result = wait(set, &mut signal);
        if result == 0 {
            return Ok(signal);
        }
        let error = io::Error::from_raw_os_error(result);
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}
