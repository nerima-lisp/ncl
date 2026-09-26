//! Failures reported by the thread and synchronization layer.

use ncl_object::ObjectError;

/// A failure raised by the thread, synchronization, timer, or deadline layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ThreadError {
    /// The object layer reported a failure.
    Object(ObjectError),
    /// The condition layer reported a failure.
    Condition(ncl_conditions::ConditionError),
    /// The object was not a thread instance.
    NotAThread,
    /// The object was not a mutex instance.
    NotAMutex,
    /// The object was not a semaphore instance.
    NotASemaphore,
    /// The object was not a wait queue instance.
    NotAWaitQueue,
    /// The object was not a timer instance.
    NotATimer,
    /// The object was not a read/write lock instance.
    NotARwLock,
    /// The object was not a process instance.
    NotAProcess,
    /// The referenced thread has already exited.
    NotRunning,
    /// A join timed out before the target thread exited.
    JoinTimeout,
    /// A blocking wait reached its deadline.
    Timeout,
    /// The operation was interrupted before it completed.
    Interrupted,
    /// The lock operation would deadlock or the wait queue is inconsistent.
    Deadlock,
    /// The OS refused to create the underlying thread.
    SpawnFailed,
    /// The registered class descriptor is missing.
    MissingClass,
    /// A root was removed in a different order than it was added.
    RootStackCorrupted,
}

impl std::fmt::Display for ThreadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Object(error) => write!(f, "object error: {error}"),
            Self::Condition(error) => write!(f, "condition error: {error}"),
            Self::NotAThread => f.write_str("not a thread"),
            Self::NotAMutex => f.write_str("not a mutex"),
            Self::NotASemaphore => f.write_str("not a semaphore"),
            Self::NotAWaitQueue => f.write_str("not a wait queue"),
            Self::NotATimer => f.write_str("not a timer"),
            Self::NotARwLock => f.write_str("not a read/write lock"),
            Self::NotAProcess => f.write_str("not a process"),
            Self::NotRunning => f.write_str("thread is not running"),
            Self::JoinTimeout => f.write_str("thread join timed out"),
            Self::Timeout => f.write_str("operation timed out"),
            Self::Interrupted => f.write_str("operation interrupted"),
            Self::Deadlock => f.write_str("operation would deadlock"),
            Self::SpawnFailed => f.write_str("thread spawn failed"),
            Self::MissingClass => f.write_str("registered class is missing"),
            Self::RootStackCorrupted => f.write_str("root stack is corrupted"),
        }
    }
}

impl std::error::Error for ThreadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Object(error) => Some(error),
            Self::Condition(error) => Some(error),
            Self::NotAThread
            | Self::NotAMutex
            | Self::NotASemaphore
            | Self::NotAWaitQueue
            | Self::NotATimer
            | Self::NotARwLock
            | Self::NotAProcess
            | Self::NotRunning
            | Self::JoinTimeout
            | Self::Timeout
            | Self::Interrupted
            | Self::Deadlock
            | Self::SpawnFailed
            | Self::MissingClass
            | Self::RootStackCorrupted => None,
        }
    }
}

impl From<ObjectError> for ThreadError {
    fn from(value: ObjectError) -> Self {
        Self::Object(value)
    }
}

impl From<ncl_conditions::ConditionError> for ThreadError {
    fn from(value: ncl_conditions::ConditionError) -> Self {
        Self::Condition(value)
    }
}
