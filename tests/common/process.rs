use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const CHILD_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn output(command: &mut Command) -> Output {
    let child = match command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => panic!("failed to run ncl: {error}"),
    };
    wait_with_timeout(child)
}

pub(super) fn wait_with_timeout(mut child: Child) -> Output {
    let deadline = Instant::now() + CHILD_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return match child.wait_with_output() {
                    Ok(output) => output,
                    Err(error) => panic!("failed to collect NCL output: {error}"),
                };
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                if let Err(error) = child.kill() {
                    panic!("NCL timed out and could not be killed: {error}");
                }
                let _ = child.wait();
                panic!("NCL did not exit within {CHILD_TIMEOUT:?}");
            }
            Err(error) => panic!("failed to poll NCL: {error}"),
        }
    }
}
