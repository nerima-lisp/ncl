//! Optimization qualities and safety-driven runtime-check decisions.

use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::cl_type::SubtypeRelation;

/// A Common Lisp optimization quality in the inclusive range 0 through 3.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct OptimizationQuality(u8);

impl OptimizationQuality {
    /// Creates a quality value after checking its supported range.
    ///
    /// # Errors
    ///
    /// Returns [`OptimizationQualityError::OutOfRange`] when `value` exceeds 3.
    pub const fn new(value: u8) -> Result<Self, OptimizationQualityError> {
        if value <= 3 {
            Ok(Self(value))
        } else {
            Err(OptimizationQualityError::OutOfRange(value))
        }
    }

    /// Returns the numeric quality.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl Default for OptimizationQuality {
    fn default() -> Self {
        Self(3)
    }
}

impl TryFrom<u8> for OptimizationQuality {
    type Error = OptimizationQualityError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// An invalid Common Lisp optimization-quality value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OptimizationQualityError {
    /// The value is greater than 3.
    OutOfRange(u8),
}

impl Display for OptimizationQualityError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfRange(value) => {
                write!(
                    formatter,
                    "optimization quality {value} is outside 0 through 3"
                )
            }
        }
    }
}

impl Error for OptimizationQualityError {}

/// Effective, independent `SPEED`, `SAFETY`, `DEBUG`, and `SPACE` qualities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OptimizationPolicy {
    speed: OptimizationQuality,
    safety: OptimizationQuality,
    debug: OptimizationQuality,
    space: OptimizationQuality,
}

impl OptimizationPolicy {
    /// Creates a policy from validated qualities.
    #[must_use]
    pub const fn new(
        speed: OptimizationQuality,
        safety: OptimizationQuality,
        debug: OptimizationQuality,
        space: OptimizationQuality,
    ) -> Self {
        Self {
            speed,
            safety,
            debug,
            space,
        }
    }

    /// Returns the effective `SPEED` quality.
    #[must_use]
    pub const fn speed(self) -> OptimizationQuality {
        self.speed
    }

    /// Returns the effective `SAFETY` quality.
    #[must_use]
    pub const fn safety(self) -> OptimizationQuality {
        self.safety
    }

    /// Returns the effective `DEBUG` quality.
    #[must_use]
    pub const fn debug(self) -> OptimizationQuality {
        self.debug
    }

    /// Returns the effective `SPACE` quality.
    #[must_use]
    pub const fn space(self) -> OptimizationQuality {
        self.space
    }

    /// Returns a copy with a new `SPEED` quality.
    #[must_use]
    pub const fn with_speed(self, speed: OptimizationQuality) -> Self {
        Self { speed, ..self }
    }

    /// Returns a copy with a new `SAFETY` quality.
    #[must_use]
    pub const fn with_safety(self, safety: OptimizationQuality) -> Self {
        Self { safety, ..self }
    }

    /// Returns a copy with a new `DEBUG` quality.
    #[must_use]
    pub const fn with_debug(self, debug: OptimizationQuality) -> Self {
        Self { debug, ..self }
    }

    /// Returns a copy with a new `SPACE` quality.
    #[must_use]
    pub const fn with_space(self, space: OptimizationQuality) -> Self {
        Self { space, ..self }
    }

    /// Whether a declared or asserted type check must remain in the HIR.
    ///
    /// An unknown subtype result is not proof and therefore retains the check
    /// whenever `SAFETY` is at least 1. An equivalent existing guard is proof.
    #[must_use]
    pub const fn requires_type_check(
        self,
        subtype: SubtypeRelation,
        equivalent_guard: bool,
    ) -> bool {
        self.safety.0 >= 1 && subtype != SubtypeRelation::Yes && !equivalent_guard
    }

    /// Whether a dynamic argument-count check must remain in the HIR.
    ///
    /// Statically invalid calls are diagnosed separately at safe-call levels.
    #[must_use]
    pub const fn requires_arity_check(
        self,
        call_validity_proven: bool,
        equivalent_guard: bool,
    ) -> bool {
        self.checks_enabled() && !call_validity_proven && !equivalent_guard
    }

    /// Whether dynamic keyword arguments must be validated in the HIR.
    #[must_use]
    pub const fn requires_keyword_check(
        self,
        keys_validity_proven: bool,
        equivalent_guard: bool,
    ) -> bool {
        self.checks_enabled() && !keys_validity_proven && !equivalent_guard
    }

    /// Whether an array-bound check must remain in the HIR.
    #[must_use]
    pub const fn requires_array_bounds_check(
        self,
        in_bounds_proven: bool,
        equivalent_guard: bool,
    ) -> bool {
        self.checks_enabled() && !in_bounds_proven && !equivalent_guard
    }

    /// Whether a statically known invalid safe call is a compile-time error.
    #[must_use]
    pub const fn diagnoses_invalid_call(self, invalidity_proven: bool) -> bool {
        self.checks_enabled() && invalidity_proven
    }

    const fn checks_enabled(self) -> bool {
        self.safety.0 >= 1
    }
}

impl Default for OptimizationPolicy {
    fn default() -> Self {
        Self::new(
            OptimizationQuality::default(),
            OptimizationQuality::default(),
            OptimizationQuality::default(),
            OptimizationQuality::default(),
        )
    }
}
