//! Process signal setup and the sigwait event queue.

use std::io;
use std::sync::{Mutex, mpsc};
use std::time::Duration;

static CONTROL_THREAD_STARTED: Mutex<bool> = Mutex::new(false);

/// A process signal observed by the dedicated sigwait thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalEvent {
    /// An interactive interrupt is ready for normal runtime delivery.
    Interrupt,
    /// An orderly process termination was requested.
    TerminationRequested,
    /// One or more child statuses may need reaping.
    ChildrenChanged,
}

/// Failure to configure or receive a process signal event.
#[derive(Debug)]
pub enum SignalControlError {
    /// An OS signal operation failed.
    OperatingSystem(io::Error),
    /// The control thread could not be created.
    ThreadSpawn(io::Error),
    /// The control thread was already started in this process.
    AlreadyStarted,
    /// The control thread failed while waiting for a signal.
    SignalWait(io::Error),
    /// The OS returned a signal outside the configured control set.
    UnexpectedSignal(i32),
    /// The control event receiver was disconnected.
    WorkerStopped,
    /// Startup state could not be read because its mutex was poisoned.
    StartStatePoisoned,
}

impl std::fmt::Display for SignalControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OperatingSystem(error) => write!(f, "signal setup failed: {error}"),
            Self::ThreadSpawn(error) => write!(f, "signal control thread creation failed: {error}"),
            Self::AlreadyStarted => f.write_str("signal control thread is already started"),
            Self::SignalWait(error) => write!(f, "sigwait failed: {error}"),
            Self::UnexpectedSignal(signal) => {
                write!(f, "sigwait returned unexpected signal {signal}")
            }
            Self::WorkerStopped => f.write_str("signal control thread stopped"),
            Self::StartStatePoisoned => f.write_str("signal control startup state is poisoned"),
        }
    }
}

impl std::error::Error for SignalControlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::OperatingSystem(error) | Self::ThreadSpawn(error) | Self::SignalWait(error) => {
                Some(error)
            }
            Self::AlreadyStarted
            | Self::UnexpectedSignal(_)
            | Self::WorkerStopped
            | Self::StartStatePoisoned => None,
        }
    }
}

/// Owns the receiver for process-wide signal events.
pub struct SignalControl {
    receiver: mpsc::Receiver<Result<SignalEvent, SignalControlError>>,
}

impl std::fmt::Debug for SignalControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignalControl").finish_non_exhaustive()
    }
}

impl SignalControl {
    /// Install the SIGPIPE disposition, prepare the calling thread, and start sigwait.
    ///
    /// Call this on the main thread before creating runtime threads. The new sigwait
    /// thread inherits its blocked signal mask from the caller.
    ///
    /// # Errors
    /// Returns an OS, startup-state, or thread-creation error. On failure, the control signals
    /// remain blocked so they cannot be delivered asynchronously during partial setup.
    pub fn start() -> Result<Self, SignalControlError> {
        let mut started = CONTROL_THREAD_STARTED
            .lock()
            .map_err(|_| SignalControlError::StartStatePoisoned)?;
        if *started {
            return Err(SignalControlError::AlreadyStarted);
        }

        ncl_sys::Thread::block_signal_control_set().map_err(SignalControlError::OperatingSystem)?;
        ncl_sys::Thread::install_sigchld_wait_disposition()
            .map_err(SignalControlError::OperatingSystem)?;
        ncl_sys::Thread::ignore_sigpipe().map_err(SignalControlError::OperatingSystem)?;
        ncl_sys::Thread::ensure_alt_signal_stack().map_err(SignalControlError::OperatingSystem)?;
        let (sender, receiver) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("ncl-sigwait".to_owned())
            .spawn(move || sigwait_loop(&sender));
        worker.map_err(SignalControlError::ThreadSpawn)?;
        *started = true;
        drop(started);
        Ok(Self { receiver })
    }

    /// Prepare a runtime thread before Lisp execution.
    ///
    /// This blocks the control signals and ensures the current OS thread has an
    /// alternate signal stack. Thread startup calls this after `start` has blocked
    /// signals on the main thread.
    ///
    /// # Errors
    /// Returns an OS signal-mask or alternate-stack error.
    pub fn prepare_current_thread() -> Result<(), SignalControlError> {
        prepare_current_thread()
    }

    /// Wait for the next signal event.
    ///
    /// # Errors
    /// Returns the sigwait failure or when the control thread has stopped.
    pub fn receive(&self) -> Result<SignalEvent, SignalControlError> {
        self.receiver
            .recv()
            .map_err(|_| SignalControlError::WorkerStopped)?
    }

    /// Wait up to `timeout` for a signal event.
    ///
    /// # Errors
    /// Returns the sigwait failure or when the control thread has stopped.
    pub fn receive_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Option<SignalEvent>, SignalControlError> {
        match self.receiver.recv_timeout(timeout) {
            Ok(event) => event.map(Some),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(SignalControlError::WorkerStopped),
        }
    }

    /// Read a queued signal event without blocking.
    ///
    /// # Errors
    /// Returns the sigwait failure or when the control thread has stopped.
    pub fn try_receive(&self) -> Result<Option<SignalEvent>, SignalControlError> {
        match self.receiver.try_recv() {
            Ok(event) => event.map(Some),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(SignalControlError::WorkerStopped),
        }
    }
}

fn prepare_current_thread() -> Result<(), SignalControlError> {
    ncl_sys::Thread::block_signal_control_set().map_err(SignalControlError::OperatingSystem)?;
    ncl_sys::Thread::ensure_alt_signal_stack().map_err(SignalControlError::OperatingSystem)?;
    Ok(())
}

fn sigwait_loop(sender: &mpsc::Sender<Result<SignalEvent, SignalControlError>>) {
    loop {
        let signal = match ncl_sys::Thread::wait_signal_control_set() {
            Ok(signal) => signal,
            Err(error) => {
                let _ = sender.send(Err(SignalControlError::SignalWait(error)));
                return;
            }
        };
        let Some(event) = control_event(signal) else {
            let _ = sender.send(Err(SignalControlError::UnexpectedSignal(signal)));
            return;
        };
        if sender.send(Ok(event)).is_err() {
            return;
        }
    }
}

const fn control_event(signal: i32) -> Option<SignalEvent> {
    if signal == 2 {
        return Some(SignalEvent::Interrupt);
    }
    if signal == 15 {
        return Some(SignalEvent::TerminationRequested);
    }
    #[cfg(target_os = "linux")]
    if signal == 17 {
        return Some(SignalEvent::ChildrenChanged);
    }
    #[cfg(target_os = "macos")]
    if signal == 20 {
        return Some(SignalEvent::ChildrenChanged);
    }
    None
}
