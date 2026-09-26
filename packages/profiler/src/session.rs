//! Thread-safe cooperative profiling session service.

use std::sync::{Arc, Mutex};

use ncl_sys::{CodeRegistry, Thread};

use crate::adapter::{FrameCatalog, ReportFormat, ReportText, ThreadSampler};
use crate::domain::{Profile, Sample, SampleError};

#[derive(Debug)]
struct SessionState {
    active: bool,
    started: bool,
    profile: Profile,
    catalog: FrameCatalog,
}

/// Errors returned by the profile session service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionError {
    /// The operation requires an active session.
    NotActive,
    /// The operation requires a session that has been started.
    NotStarted,
    /// Another thread poisoned the session lock.
    Poisoned,
    /// The runtime sample could not be adapted into a domain sample.
    Sampling(SampleError),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotActive => f.write_str("profile session is not active"),
            Self::NotStarted => f.write_str("profile session has not started"),
            Self::Poisoned => f.write_str("profile session lock is poisoned"),
            Self::Sampling(error) => write!(f, "sampling failed: {error}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<SampleError> for SessionError {
    fn from(error: SampleError) -> Self {
        Self::Sampling(error)
    }
}

/// Immutable result retained after stopping a profile session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileSnapshot {
    profile: Profile,
    catalog: FrameCatalog,
}

impl ProfileSnapshot {
    /// Number of samples retained in this snapshot.
    #[must_use]
    pub fn sample_count(&self) -> u64 {
        self.profile.sample_count()
    }

    /// Render the snapshot without accessing mutable session state.
    #[must_use]
    pub fn report(&self, format: ReportFormat) -> ReportText {
        ReportText::render(&self.profile, &self.catalog, format)
    }
}

/// A cloneable, thread-safe, cooperative profiling service.
#[derive(Clone, Debug)]
pub struct ProfileSession {
    state: Arc<Mutex<SessionState>>,
}

impl ProfileSession {
    /// Create a stopped session with no retained samples.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(SessionState {
                active: false,
                started: false,
                profile: Profile::new(),
                catalog: FrameCatalog::new(),
            })),
        }
    }

    /// Start a new session, discarding the previous session and catalog.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Poisoned`] if another thread poisoned the lock.
    pub fn start(&self) -> Result<(), SessionError> {
        let mut state = self.state.lock().map_err(|_| SessionError::Poisoned)?;
        state.active = true;
        state.started = true;
        state.profile = Profile::new();
        state.catalog = FrameCatalog::new();
        drop(state);
        Ok(())
    }

    /// Stop sampling and return an immutable snapshot of collected samples.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::NotStarted`], [`SessionError::NotActive`], or
    /// [`SessionError::Poisoned`] when the session cannot be stopped.
    pub fn stop(&self) -> Result<ProfileSnapshot, SessionError> {
        let mut state = self.state.lock().map_err(|_| SessionError::Poisoned)?;
        if !state.active {
            return Err(if state.started {
                SessionError::NotActive
            } else {
                SessionError::NotStarted
            });
        }
        state.active = false;
        Ok(ProfileSnapshot {
            profile: state.profile.clone(),
            catalog: state.catalog.clone(),
        })
    }

    /// Record one already-adapted sample from a cooperative sampler.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::NotStarted`], [`SessionError::NotActive`], or
    /// [`SessionError::Poisoned`] when the sample cannot be recorded.
    pub fn record(&self, sample: Sample) -> Result<(), SessionError> {
        let mut state = self.state.lock().map_err(|_| SessionError::Poisoned)?;
        if !state.active {
            return Err(if state.started {
                SessionError::NotActive
            } else {
                SessionError::NotStarted
            });
        }
        state.profile.push(sample);
        drop(state);
        Ok(())
    }

    /// Sample one published thread snapshot and record it when active.
    ///
    /// # Errors
    ///
    /// Returns a session or sampling error when the snapshot cannot be recorded.
    pub fn sample_thread(
        &self,
        sampler: &ThreadSampler,
        thread: &Thread,
        registry: &CodeRegistry,
    ) -> Result<(), SessionError> {
        let mut state = self.state.lock().map_err(|_| SessionError::Poisoned)?;
        if !state.active {
            return Err(if state.started {
                SessionError::NotActive
            } else {
                SessionError::NotStarted
            });
        }
        let sample = sampler.sample(thread, registry, &mut state.catalog)?;
        state.profile.push(sample);
        drop(state);
        Ok(())
    }

    /// Render the current session without stopping it.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::NotStarted`] or [`SessionError::Poisoned`].
    pub fn report(&self, format: ReportFormat) -> Result<ReportText, SessionError> {
        let (profile, catalog) = {
            let state = self.state.lock().map_err(|_| SessionError::Poisoned)?;
            if !state.started {
                return Err(SessionError::NotStarted);
            }
            (state.profile.clone(), state.catalog.clone())
        };
        Ok(ReportText::render(&profile, &catalog, format))
    }
}

impl Default for ProfileSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Lisp-facing typed facade for the profiler extension functions.
#[derive(Clone, Debug, Default)]
pub struct LispProfileApi {
    session: ProfileSession,
}

impl LispProfileApi {
    /// Create a facade backed by a fresh session.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Equivalent of `NCL-PROFILER:PROFILE-START`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Poisoned`] if the session lock is poisoned.
    pub fn profile_start(&self) -> Result<(), SessionError> {
        self.session.start()
    }

    /// Equivalent of `NCL-PROFILER:PROFILE-STOP`.
    ///
    /// # Errors
    ///
    /// Returns a session state error when no active session exists.
    pub fn profile_stop(&self) -> Result<ProfileSnapshot, SessionError> {
        self.session.stop()
    }

    /// Equivalent of `NCL-PROFILER:PROFILE-REPORT`.
    ///
    /// # Errors
    ///
    /// Returns a session state error when no session exists.
    pub fn profile_report(&self, format: ReportFormat) -> Result<ReportText, SessionError> {
        self.session.report(format)
    }

    /// Access the underlying service for runtime sampler integration.
    #[must_use]
    pub const fn session(&self) -> &ProfileSession {
        &self.session
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;
    use std::thread;
    use std::time::Duration;

    use ncl_sys::{CodeRegistry, Thread};

    use super::{LispProfileApi, ProfileSession, SessionError};
    use crate::adapter::{ReportFormat, ThreadSampler};
    use crate::domain::{FrameId, Sample, SampleError};

    fn sample(leaf: u32) -> Option<Sample> {
        Sample::new(vec![FrameId::new(1), FrameId::new(leaf)]).ok()
    }

    #[test]
    fn start_stop_report_and_lisp_facade_are_usable() {
        let api = LispProfileApi::new();
        assert_eq!(
            api.profile_report(ReportFormat::Flat),
            Err(SessionError::NotStarted)
        );
        assert!(api.profile_start().is_ok());
        let sample = sample(2);
        assert!(sample.is_some());
        if let Some(sample) = sample {
            assert!(api.session().record(sample).is_ok());
        }
        let report = api.profile_report(ReportFormat::Flat);
        assert!(report.is_ok());
        if let Ok(report) = report {
            assert_eq!(report.as_str(), "<unknown> 1");
        }
        let snapshot = api.profile_stop();
        assert!(snapshot.is_ok());
        if let Ok(snapshot) = snapshot {
            assert_eq!(snapshot.sample_count(), 1);
        }
    }

    #[test]
    fn session_state_errors_are_explicit() {
        for error in [
            SessionError::NotActive,
            SessionError::NotStarted,
            SessionError::Poisoned,
            SessionError::Sampling(SampleError::EmptyStack),
        ] {
            assert!(!error.to_string().is_empty());
        }
        let session = ProfileSession::new();
        assert_eq!(session.stop(), Err(SessionError::NotStarted));
        assert_eq!(
            session.report(ReportFormat::Flat),
            Err(SessionError::NotStarted)
        );
        let sample = Sample::new(vec![FrameId::new(1)]);
        assert!(sample.is_ok());
        if let Ok(sample) = sample {
            assert_eq!(session.record(sample), Err(SessionError::NotStarted));
        }
        assert!(session.start().is_ok());
        let sampler = ThreadSampler::new(1, 1);
        assert!(sampler.is_ok());
        let thread = Thread::new();
        let registry = CodeRegistry::default();
        if let Ok(sampler) = sampler {
            assert_eq!(
                session.sample_thread(&sampler, &thread, &registry),
                Err(SessionError::Sampling(SampleError::EmptyStack))
            );
        }
        let snapshot = session.stop();
        assert!(snapshot.is_ok());
        if let Ok(snapshot) = snapshot {
            assert!(snapshot.report(ReportFormat::Folded).as_str().is_empty());
        }
        assert_eq!(session.stop(), Err(SessionError::NotActive));
        assert!(session.report(ReportFormat::Flat).is_ok());
    }

    #[test]
    fn multiple_recorders_settle_before_watchdog_deadline() {
        let session = ProfileSession::new();
        assert!(session.start().is_ok());
        let (sender, receiver) = channel();
        let mut handles = Vec::new();
        for leaf in [2_u32, 3_u32, 4_u32, 5_u32] {
            let worker = session.clone();
            let sender = sender.clone();
            handles.push(thread::spawn(move || {
                let result = sample(leaf).is_some_and(|sample| worker.record(sample).is_ok());
                let _ = sender.send(result);
            }));
        }
        drop(sender);
        for _ in 0..4 {
            assert_eq!(receiver.recv_timeout(Duration::from_secs(1)), Ok(true));
        }
        for handle in handles {
            assert!(handle.join().is_ok());
        }
        let snapshot = session.stop();
        assert!(snapshot.is_ok());
        if let Ok(snapshot) = snapshot {
            assert_eq!(snapshot.sample_count(), 4);
        }
    }
}
