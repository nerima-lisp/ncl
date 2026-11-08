//! Bounded numeric domains used by the HIR type lattice.

use std::cmp::Ordering;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// A numeric representation whose range can be bounded in HIR.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum NumericDomain {
    /// Integers represented by signed 64-bit boundaries.
    Integer,
    /// Single-precision floating-point values.
    SingleFloat,
    /// Double-precision floating-point values.
    DoubleFloat,
}

/// A numeric value used as a range endpoint or member.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum NumericValue {
    /// An integer value.
    Integer(i64),
    /// A single-precision float's IEEE 754 bit pattern.
    SingleFloat(u32),
    /// A double-precision float's IEEE 754 bit pattern.
    DoubleFloat(u64),
}

impl NumericValue {
    /// Returns the representation used by this value.
    #[must_use]
    pub const fn domain(self) -> NumericDomain {
        match self {
            Self::Integer(_) => NumericDomain::Integer,
            Self::SingleFloat(_) => NumericDomain::SingleFloat,
            Self::DoubleFloat(_) => NumericDomain::DoubleFloat,
        }
    }

    const fn is_nan(self) -> bool {
        match self {
            Self::Integer(_) => false,
            Self::SingleFloat(bits) => f32::from_bits(bits).is_nan(),
            Self::DoubleFloat(bits) => f64::from_bits(bits).is_nan(),
        }
    }

    fn cmp(self, other: Self) -> Ordering {
        match (self, other) {
            (Self::Integer(left), Self::Integer(right)) => left.cmp(&right),
            (Self::SingleFloat(left), Self::SingleFloat(right)) => {
                f32::from_bits(left).total_cmp(&f32::from_bits(right))
            }
            (Self::DoubleFloat(left), Self::DoubleFloat(right)) => {
                f64::from_bits(left).total_cmp(&f64::from_bits(right))
            }
            (Self::Integer(_), Self::SingleFloat(_) | Self::DoubleFloat(_))
            | (Self::SingleFloat(_), Self::DoubleFloat(_)) => Ordering::Less,
            (Self::SingleFloat(_), Self::Integer(_))
            | (Self::DoubleFloat(_), Self::Integer(_) | Self::SingleFloat(_)) => Ordering::Greater,
        }
    }

    fn partial_cmp(self, other: Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Integer(left), Self::Integer(right)) => Some(left.cmp(&right)),
            (Self::SingleFloat(left), Self::SingleFloat(right)) => {
                f32::from_bits(left).partial_cmp(&f32::from_bits(right))
            }
            (Self::DoubleFloat(left), Self::DoubleFloat(right)) => {
                f64::from_bits(left).partial_cmp(&f64::from_bits(right))
            }
            (Self::Integer(_), Self::SingleFloat(_) | Self::DoubleFloat(_))
            | (Self::SingleFloat(_), Self::Integer(_) | Self::DoubleFloat(_))
            | (Self::DoubleFloat(_), Self::Integer(_) | Self::SingleFloat(_)) => None,
        }
    }
}

/// An inclusive or exclusive endpoint in a numeric range.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NumericBound {
    value: NumericValue,
    inclusive: bool,
}

impl NumericBound {
    /// Creates an inclusive endpoint.
    #[must_use]
    pub const fn inclusive(value: NumericValue) -> Self {
        Self {
            value,
            inclusive: true,
        }
    }

    /// Creates an exclusive endpoint.
    #[must_use]
    pub const fn exclusive(value: NumericValue) -> Self {
        Self {
            value,
            inclusive: false,
        }
    }

    /// Returns this endpoint's numeric value.
    #[must_use]
    pub const fn value(self) -> NumericValue {
        self.value
    }

    /// Returns whether this endpoint includes its value.
    #[must_use]
    pub const fn is_inclusive(self) -> bool {
        self.inclusive
    }

    const fn canonicalized(self) -> Self {
        let value = match self.value {
            NumericValue::SingleFloat(bits) if bits.trailing_zeros() >= 31 => {
                NumericValue::SingleFloat(0)
            }
            NumericValue::DoubleFloat(bits) if bits.trailing_zeros() >= 63 => {
                NumericValue::DoubleFloat(0)
            }
            _ => self.value, // check-added-lines: allow(wildcard) integers and nonzero floats remain unchanged
        };
        Self {
            value,
            inclusive: self.inclusive,
        }
    }
}

/// An interval over one numeric representation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NumericRange {
    domain: NumericDomain,
    lower: Option<NumericBound>,
    upper: Option<NumericBound>,
}

impl NumericRange {
    /// Creates a range after checking endpoint domains and interval order.
    ///
    /// # Errors
    ///
    /// Returns `NumericRangeError` when an endpoint has the wrong representation,
    /// is NaN, or makes the interval empty.
    pub fn new(
        domain: NumericDomain,
        lower: Option<NumericBound>,
        upper: Option<NumericBound>,
    ) -> Result<Self, NumericRangeError> {
        let lower = lower.map(NumericBound::canonicalized);
        let upper = upper.map(NumericBound::canonicalized);
        for bound in lower.into_iter().chain(upper) {
            if bound.value.domain() != domain {
                return Err(NumericRangeError::DomainMismatch {
                    expected: domain,
                    actual: bound.value.domain(),
                });
            }
            if bound.value.is_nan() {
                return Err(NumericRangeError::NaNBoundary);
            }
        }
        if let (Some(lower), Some(upper)) = (lower, upper) {
            match lower.value.cmp(upper.value) {
                Ordering::Greater => return Err(NumericRangeError::EmptyInterval),
                Ordering::Equal => {
                    if !(lower.inclusive && upper.inclusive) {
                        return Err(NumericRangeError::EmptyInterval);
                    }
                }
                Ordering::Less => {}
            }
        }
        if domain == NumericDomain::Integer && integer_interval_is_empty(lower, upper) {
            return Err(NumericRangeError::EmptyInterval);
        }
        if domain == NumericDomain::SingleFloat && single_float_interval_is_empty(lower, upper) {
            return Err(NumericRangeError::EmptyInterval);
        }
        if domain == NumericDomain::DoubleFloat && double_float_interval_is_empty(lower, upper) {
            return Err(NumericRangeError::EmptyInterval);
        }
        Ok(Self {
            domain,
            lower,
            upper,
        })
    }

    /// Returns the numeric representation covered by this range.
    #[must_use]
    pub const fn domain(self) -> NumericDomain {
        self.domain
    }

    /// Returns the lower endpoint, or `None` when unbounded below.
    #[must_use]
    pub const fn lower(self) -> Option<NumericBound> {
        self.lower
    }

    /// Returns the upper endpoint, or `None` when unbounded above.
    #[must_use]
    pub const fn upper(self) -> Option<NumericBound> {
        self.upper
    }

    pub(super) fn contains_range(self, other: Self) -> bool {
        match self.domain {
            NumericDomain::Integer => {
                let (container_lower, container_upper) =
                    integer_range_bounds(self.lower, self.upper);
                let (candidate_lower, candidate_upper) =
                    integer_range_bounds(other.lower, other.upper);
                lower_bound_contains(container_lower, candidate_lower)
                    && upper_bound_contains(container_upper, candidate_upper)
            }
            NumericDomain::SingleFloat => {
                let (container_lower, container_upper) =
                    single_float_range_bounds(self.lower, self.upper);
                let (candidate_lower, candidate_upper) =
                    single_float_range_bounds(other.lower, other.upper);
                lower_bound_contains(Some(container_lower), Some(candidate_lower))
                    && upper_bound_contains(Some(container_upper), Some(candidate_upper))
            }
            NumericDomain::DoubleFloat => {
                let (container_lower, container_upper) =
                    double_float_range_bounds(self.lower, self.upper);
                let (candidate_lower, candidate_upper) =
                    double_float_range_bounds(other.lower, other.upper);
                lower_bound_contains(Some(container_lower), Some(candidate_lower))
                    && upper_bound_contains(Some(container_upper), Some(candidate_upper))
            }
        }
    }

    pub(super) fn is_disjoint(self, other: Self) -> bool {
        match self.domain {
            NumericDomain::Integer => {
                let (left_lower, left_upper) = integer_range_bounds(self.lower, self.upper);
                let (right_lower, right_upper) = integer_range_bounds(other.lower, other.upper);
                ends_before(left_upper, right_lower) || ends_before(right_upper, left_lower)
            }
            NumericDomain::SingleFloat => {
                let (left_lower, left_upper) = single_float_range_bounds(self.lower, self.upper);
                let (right_lower, right_upper) =
                    single_float_range_bounds(other.lower, other.upper);
                left_upper < right_lower || right_upper < left_lower
            }
            NumericDomain::DoubleFloat => {
                let (left_lower, left_upper) = double_float_range_bounds(self.lower, self.upper);
                let (right_lower, right_upper) =
                    double_float_range_bounds(other.lower, other.upper);
                left_upper < right_lower || right_upper < left_lower
            }
        }
    }

    pub(super) fn contains_value(self, value: NumericValue) -> Option<bool> {
        if self.domain != value.domain() {
            return None;
        }
        let above_lower = self
            .lower
            .is_none_or(|bound| match value.partial_cmp(bound.value) {
                Some(Ordering::Greater) => true,
                Some(Ordering::Equal) => bound.inclusive,
                Some(Ordering::Less) | None => false,
            });
        let below_upper = self
            .upper
            .is_none_or(|bound| match value.partial_cmp(bound.value) {
                Some(Ordering::Less) => true,
                Some(Ordering::Equal) => bound.inclusive,
                Some(Ordering::Greater) | None => false,
            });
        Some(above_lower && below_upper)
    }

    pub(super) fn singleton(self) -> Option<NumericValue> {
        match self.domain {
            NumericDomain::Integer => {
                let (Some(lower), Some(upper)) = integer_range_bounds(self.lower, self.upper)
                else {
                    return None;
                };
                if lower != upper {
                    return None;
                }
                i64::try_from(lower).ok().map(NumericValue::Integer)
            }
            NumericDomain::SingleFloat => {
                let (lower, upper) = single_float_range_bounds(self.lower, self.upper);
                (lower == upper)
                    .then(|| NumericValue::SingleFloat(single_float_from_order_key(lower)))
            }
            NumericDomain::DoubleFloat => {
                let (lower, upper) = double_float_range_bounds(self.lower, self.upper);
                (lower == upper)
                    .then(|| NumericValue::DoubleFloat(double_float_from_order_key(lower)))
            }
        }
    }

    pub(super) fn covers_single_float_domain(self) -> bool {
        single_float_range_bounds(self.lower, self.upper)
            == (
                single_float_order_key(f32::NEG_INFINITY.to_bits()),
                single_float_order_key(f32::INFINITY.to_bits()),
            )
    }

    pub(super) fn covers_double_float_domain(self) -> bool {
        double_float_range_bounds(self.lower, self.upper)
            == (
                double_float_order_key(f64::NEG_INFINITY.to_bits()),
                double_float_order_key(f64::INFINITY.to_bits()),
            )
    }
}

fn integer_interval_is_empty(lower: Option<NumericBound>, upper: Option<NumericBound>) -> bool {
    let (lower, upper) = integer_range_bounds(lower, upper);
    match (lower, upper) {
        (Some(lower), Some(upper)) => lower > upper,
        _ => false, // check-added-lines: allow(wildcard) missing endpoints cannot prove emptiness
    }
}

fn integer_range_bounds(
    lower: Option<NumericBound>,
    upper: Option<NumericBound>,
) -> (Option<i128>, Option<i128>) {
    let lower = lower.and_then(|bound| match bound.value {
        NumericValue::Integer(value) => Some(i128::from(value) + i128::from(!bound.inclusive)),
        _ => None, // check-added-lines: allow(wildcard) constructor enforces integer endpoint domains
    });
    let upper = upper.and_then(|bound| match bound.value {
        NumericValue::Integer(value) => Some(i128::from(value) - i128::from(!bound.inclusive)),
        _ => None, // check-added-lines: allow(wildcard) constructor enforces integer endpoint domains
    });
    (lower, upper)
}

fn single_float_range_bounds(
    lower: Option<NumericBound>,
    upper: Option<NumericBound>,
) -> (u32, u32) {
    let lower = lower.map_or_else(
        || single_float_order_key(f32::NEG_INFINITY.to_bits()),
        |bound| match bound.value {
            NumericValue::SingleFloat(bits) => {
                single_float_order_key(bits) + u32::from(!bound.inclusive)
            }
            _ => u32::MAX, // check-added-lines: allow(wildcard) constructor enforces single-float endpoint domains
        },
    );
    let upper = upper.map_or_else(
        || single_float_order_key(f32::INFINITY.to_bits()),
        |bound| match bound.value {
            NumericValue::SingleFloat(bits) => {
                single_float_order_key(bits) - u32::from(!bound.inclusive)
            }
            _ => 0, // check-added-lines: allow(wildcard) constructor enforces single-float endpoint domains
        },
    );
    (lower, upper)
}

fn double_float_range_bounds(
    lower: Option<NumericBound>,
    upper: Option<NumericBound>,
) -> (u64, u64) {
    let lower = lower.map_or_else(
        || double_float_order_key(f64::NEG_INFINITY.to_bits()),
        |bound| match bound.value {
            NumericValue::DoubleFloat(bits) => {
                double_float_order_key(bits) + u64::from(!bound.inclusive)
            }
            _ => u64::MAX, // check-added-lines: allow(wildcard) constructor enforces double-float endpoint domains
        },
    );
    let upper = upper.map_or_else(
        || double_float_order_key(f64::INFINITY.to_bits()),
        |bound| match bound.value {
            NumericValue::DoubleFloat(bits) => {
                double_float_order_key(bits) - u64::from(!bound.inclusive)
            }
            _ => 0, // check-added-lines: allow(wildcard) constructor enforces double-float endpoint domains
        },
    );
    (lower, upper)
}

fn single_float_interval_is_empty(
    lower: Option<NumericBound>,
    upper: Option<NumericBound>,
) -> bool {
    let (lower, upper) = single_float_range_bounds(lower, upper);
    lower > upper
}

fn double_float_interval_is_empty(
    lower: Option<NumericBound>,
    upper: Option<NumericBound>,
) -> bool {
    let (lower, upper) = double_float_range_bounds(lower, upper);
    lower > upper
}

const fn single_float_order_key(bits: u32) -> u32 {
    if bits & 0x8000_0000 != 0 {
        !bits
    } else if bits == 0 {
        0x7fff_ffff
    } else {
        0x8000_0000 | (bits - 1)
    }
}

const fn double_float_order_key(bits: u64) -> u64 {
    if bits & 0x8000_0000_0000_0000 != 0 {
        !bits
    } else if bits == 0 {
        0x7fff_ffff_ffff_ffff
    } else {
        0x8000_0000_0000_0000 | (bits - 1)
    }
}

const fn single_float_from_order_key(key: u32) -> u32 {
    if key <= 0x7fff_ffff {
        !key
    } else {
        (key & 0x7fff_ffff) + 1
    }
}

const fn double_float_from_order_key(key: u64) -> u64 {
    if key <= 0x7fff_ffff_ffff_ffff {
        !key
    } else {
        (key & 0x7fff_ffff_ffff_ffff) + 1
    }
}

fn lower_bound_contains<T: Copy + Ord>(container: Option<T>, candidate: Option<T>) -> bool {
    match (container, candidate) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(container), Some(candidate)) => candidate >= container,
    }
}

fn upper_bound_contains<T: Copy + Ord>(container: Option<T>, candidate: Option<T>) -> bool {
    match (container, candidate) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(container), Some(candidate)) => candidate <= container,
    }
}

fn ends_before<T: Ord>(upper: Option<T>, lower: Option<T>) -> bool {
    match (upper, lower) {
        (Some(upper), Some(lower)) => upper < lower,
        _ => false, // check-added-lines: allow(wildcard) unbounded endpoints cannot prove order
    }
}

/// Errors from constructing an invalid numeric interval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NumericRangeError {
    /// A boundary uses a different representation from the range.
    DomainMismatch {
        /// The representation required by the range.
        expected: NumericDomain,
        /// The representation supplied by the boundary.
        actual: NumericDomain,
    },
    /// NaN cannot define an ordered interval boundary.
    NaNBoundary,
    /// The endpoints describe no numeric values.
    EmptyInterval,
}

impl Display for NumericRangeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "numeric range error: {self:?}")
    }
}

impl Error for NumericRangeError {}
