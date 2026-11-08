//! Process signal setup and the sigwait event queue.

use std::cell::Cell;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::time::Duration;

static CONTROL_THREAD_STARTED: Mutex<bool> = Mutex::new(false);

pub(super) fn lock_signal_control_start_state<'a>(
    result: Result<MutexGuard<'a, bool>, PoisonError<MutexGuard<'a, bool>>>,
) -> Result<MutexGuard<'a, bool>, SignalControlError> {
    result.map_err(|_| SignalControlError::StartStatePoisoned)
}

pub(super) fn map_signal_setup_result(result: io::Result<()>) -> Result<(), SignalControlError> {
    result.map_err(SignalControlError::OperatingSystem)
}

pub(super) fn map_signal_worker_spawn(result: io::Result<()>) -> Result<(), SignalControlError> {
    result.map_err(SignalControlError::ThreadSpawn)
}

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
    /// A signal worker failed or the control event receiver was disconnected.
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
    failed: Cell<bool>,
}

impl std::fmt::Debug for SignalControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignalControl").finish_non_exhaustive()
    }
}

impl SignalControl {
    /// Install the SIGPIPE disposition, prepare the calling thread, and start sigwait.
    ///
    /// Call this on the main thread before creating runtime threads. The sigwait
    /// workers inherit their blocked signal mask from the caller.
    ///
    /// # Errors
    /// Returns an OS, startup-state, or thread-creation error. On failure, the control signals
    /// remain blocked so they cannot be delivered asynchronously during partial setup. A failure
    /// after one macOS worker starts is terminal; the caller must stop runtime startup and let
    /// the process exit.
    pub fn start() -> Result<Self, SignalControlError> {
        let mut started = lock_signal_control_start_state(CONTROL_THREAD_STARTED.lock())?;
        if *started {
            return Err(SignalControlError::AlreadyStarted);
        }

        map_signal_setup_result(ncl_sys::Thread::block_signal_control_set())?;
        map_signal_setup_result(ncl_sys::Thread::install_sigchld_wait_disposition())?;
        map_signal_setup_result(ncl_sys::Thread::ignore_sigpipe())?;
        map_signal_setup_result(ncl_sys::Thread::ensure_alt_signal_stack().map(|_| ()))?;
        let (sender, receiver) = mpsc::channel();
        let worker_failed = Arc::new(AtomicBool::new(false));
        #[cfg(target_os = "linux")]
        map_signal_worker_spawn(spawn_signal_worker(
            "ncl-sigwait",
            sender,
            worker_failed,
            ncl_sys::Thread::wait_signal_control_set,
        ))?;
        #[cfg(target_os = "macos")]
        {
            map_signal_worker_spawn(spawn_signal_worker(
                "ncl-sigwait-process",
                sender.clone(),
                Arc::clone(&worker_failed),
                ncl_sys::Thread::wait_process_signal,
            ))?;
            *started = true;
            if let Err(error) = map_signal_worker_spawn(spawn_signal_worker(
                "ncl-sigwait-children",
                sender,
                Arc::clone(&worker_failed),
                ncl_sys::Thread::wait_child_signal,
            )) {
                worker_failed.store(true, Ordering::Release);
                return Err(error);
            }
        }
        #[cfg(target_os = "linux")]
        {
            *started = true;
        }
        drop(started);
        Ok(Self::with_receiver(receiver))
    }

    pub(super) fn with_receiver(
        receiver: mpsc::Receiver<Result<SignalEvent, SignalControlError>>,
    ) -> Self {
        Self {
            receiver,
            failed: Cell::new(false),
        }
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
    /// Returns the first worker failure; later calls report that the worker stopped.
    /// After a worker failure, callers must stop runtime work and exit the process.
    pub fn receive(&self) -> Result<SignalEvent, SignalControlError> {
        if self.failed.get() {
            return Err(SignalControlError::WorkerStopped);
        }
        self.receiver.recv().map_or_else(
            |_| self.finish_receive(Err(SignalControlError::WorkerStopped)),
            |result| self.finish_receive(result),
        )
    }

    /// Wait up to `timeout` for a signal event.
    ///
    /// # Errors
    /// Returns the first worker failure; later calls report that the worker stopped.
    /// After a worker failure, callers must stop runtime work and exit the process.
    pub fn receive_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Option<SignalEvent>, SignalControlError> {
        if self.failed.get() {
            return Err(SignalControlError::WorkerStopped);
        }
        match self.receiver.recv_timeout(timeout) {
            Ok(result) => self.finish_receive(result).map(Some),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => self
                .finish_receive(Err(SignalControlError::WorkerStopped))
                .map(Some),
        }
    }

    /// Read a queued signal event without blocking.
    ///
    /// # Errors
    /// Returns the first worker failure; later calls report that the worker stopped.
    /// After a worker failure, callers must stop runtime work and exit the process.
    pub fn try_receive(&self) -> Result<Option<SignalEvent>, SignalControlError> {
        if self.failed.get() {
            return Err(SignalControlError::WorkerStopped);
        }
        match self.receiver.try_recv() {
            Ok(result) => self.finish_receive(result).map(Some),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => self
                .finish_receive(Err(SignalControlError::WorkerStopped))
                .map(Some),
        }
    }

    fn finish_receive(
        &self,
        result: Result<SignalEvent, SignalControlError>,
    ) -> Result<SignalEvent, SignalControlError> {
        if result.is_err() {
            self.failed.set(true);
        }
        result
    }
}

fn prepare_current_thread() -> Result<(), SignalControlError> {
    map_signal_setup_result(ncl_sys::Thread::block_signal_control_set())?;
    map_signal_setup_result(ncl_sys::Thread::ensure_alt_signal_stack().map(|_| ()))?;
    Ok(())
}

fn spawn_signal_worker(
    name: &'static str,
    sender: mpsc::Sender<Result<SignalEvent, SignalControlError>>,
    worker_failed: Arc<AtomicBool>,
    wait: fn() -> io::Result<i32>,
) -> io::Result<()> {
    std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || sigwait_loop(&sender, &worker_failed, wait))
        .map(|_| ())
}

pub(super) fn sigwait_loop(
    sender: &mpsc::Sender<Result<SignalEvent, SignalControlError>>,
    worker_failed: &AtomicBool,
    mut wait: impl FnMut() -> io::Result<i32>,
) {
    loop {
        if worker_failed.load(Ordering::Acquire) {
            return;
        }
        let signal = match wait() {
            Ok(signal) => signal,
            Err(error) => {
                report_worker_failure(sender, worker_failed, SignalControlError::SignalWait(error));
                return;
            }
        };
        if worker_failed.load(Ordering::Acquire) {
            return;
        }
        let Some(event) = control_event(signal) else {
            report_worker_failure(
                sender,
                worker_failed,
                SignalControlError::UnexpectedSignal(signal),
            );
            return;
        };
        if sender.send(Ok(event)).is_err() {
            return;
        }
    }
}

fn report_worker_failure(
    sender: &mpsc::Sender<Result<SignalEvent, SignalControlError>>,
    worker_failed: &AtomicBool,
    error: SignalControlError,
) {
    worker_failed.store(true, Ordering::Release);
    let _ = sender.send(Err(error));
}

pub(super) const fn control_event(signal: i32) -> Option<SignalEvent> {
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
