//! Magnitude operations for signed little-endian 32-bit limb integers.

use core::cmp::Ordering;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BigInteger {
    pub(super) negative: bool,
    pub(super) limbs: Vec<u32>,
}

impl BigInteger {
    pub(super) fn from_i128(value: i128) -> Self {
        let bytes = value.unsigned_abs().to_le_bytes();
        let mut limbs = Vec::with_capacity(4);
        for chunk in bytes.as_chunks::<4>().0 {
            limbs.push(u32::from_le_bytes(*chunk));
        }
        Self::normalize(&mut limbs);
        Self {
            negative: value < 0,
            limbs,
        }
    }

    pub(super) const fn with_sign(mut self, negative: bool) -> Self {
        self.negative = negative && !self.limbs.is_empty();
        self
    }

    pub(super) fn to_i128(&self) -> Option<i128> {
        if self.limbs.len() > 4 {
            return None;
        }
        let mut magnitude = 0_u128;
        for (index, limb) in self.limbs.iter().enumerate() {
            let shift = u32::try_from(index.checked_mul(32)?).ok()?;
            magnitude |= u128::from(*limb).checked_shl(shift)?;
        }
        if self.negative {
            if magnitude == 1_u128 << 127 {
                Some(i128::MIN)
            } else {
                i128::try_from(magnitude).ok()?.checked_neg()
            }
        } else {
            i128::try_from(magnitude).ok()
        }
    }

    pub(super) fn normalize(limbs: &mut Vec<u32>) {
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
    }

    fn cmp_abs(left: &[u32], right: &[u32]) -> Ordering {
        left.len().cmp(&right.len()).then_with(|| {
            left.iter()
                .rev()
                .zip(right.iter().rev())
                .find_map(|(left, right)| (left != right).then(|| left.cmp(right)))
                .unwrap_or(Ordering::Equal)
        })
    }

    pub(super) fn cmp(&self, other: &Self) -> Ordering {
        match (self.negative, other.negative) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (true, true) => Self::cmp_abs(&self.limbs, &other.limbs).reverse(),
            (false, false) => Self::cmp_abs(&self.limbs, &other.limbs),
        }
    }

    pub(super) fn cmp_i128(&self, other: i128) -> Ordering {
        match (self.negative, other < 0) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (true, true) => self.cmp_abs_i128(other.unsigned_abs()).reverse(),
            (false, false) => self.cmp_abs_i128(other.unsigned_abs()),
        }
    }

    fn cmp_abs_i128(&self, magnitude: u128) -> Ordering {
        let bytes = magnitude.to_le_bytes();
        let small_limbs = bytes.as_chunks::<4>().0;
        let small_len = small_limbs
            .iter()
            .rposition(|limb_bytes| u32::from_le_bytes(*limb_bytes) != 0)
            .map_or(0, |index| index.saturating_add(1));
        self.limbs.len().cmp(&small_len).then_with(|| {
            self.limbs
                .iter()
                .rev()
                .zip(small_limbs.iter().rev().take(small_len))
                .find_map(|(left, right_bytes)| {
                    let right = u32::from_le_bytes(*right_bytes);
                    (left != &right).then(|| left.cmp(&right))
                })
                .unwrap_or(Ordering::Equal)
        })
    }

    fn add_abs(left: &[u32], right: &[u32]) -> Option<Vec<u32>> {
        let len = left.len().max(right.len());
        let mut result = Vec::new();
        result.try_reserve_exact(len.checked_add(1)?).ok()?;
        let mut carry = 0_u64;
        for index in 0..len {
            let a = u64::from(left.get(index).map_or(0, |value| *value));
            let b = u64::from(right.get(index).map_or(0, |value| *value));
            let sum = a.checked_add(b)?.checked_add(carry)?;
            result.push(u32::try_from(sum & u64::from(u32::MAX)).ok()?);
            carry = sum >> 32;
        }
        if carry != 0 {
            result.push(u32::try_from(carry).ok()?);
        }
        Some(result)
    }

    fn sub_abs(left: &[u32], right: &[u32]) -> Option<Vec<u32>> {
        if Self::cmp_abs(left, right) == Ordering::Less {
            return None;
        }
        let mut result = Vec::new();
        result.try_reserve_exact(left.len()).ok()?;
        let mut borrow = 0_i64;
        for (index, left_limb) in left.iter().enumerate() {
            let right_limb = i64::from(right.get(index).map_or(0, |value| *value));
            let difference = i64::from(*left_limb)
                .checked_sub(right_limb)?
                .checked_sub(borrow)?;
            if difference < 0 {
                result.push(u32::try_from(difference + (1_i64 << 32)).ok()?);
                borrow = 1;
            } else {
                result.push(u32::try_from(difference).ok()?);
                borrow = 0;
            }
        }
        if borrow != 0 {
            return None;
        }
        Self::normalize(&mut result);
        Some(result)
    }

    pub(super) fn add(&self, other: &Self) -> Option<Self> {
        if self.negative == other.negative {
            return Some(Self {
                negative: self.negative,
                limbs: Self::add_abs(&self.limbs, &other.limbs)?,
            });
        }
        match Self::cmp_abs(&self.limbs, &other.limbs) {
            Ordering::Equal => Some(Self {
                negative: false,
                limbs: Vec::new(),
            }),
            Ordering::Greater => Some(Self {
                negative: self.negative,
                limbs: Self::sub_abs(&self.limbs, &other.limbs)?,
            }),
            Ordering::Less => Some(Self {
                negative: other.negative,
                limbs: Self::sub_abs(&other.limbs, &self.limbs)?,
            }),
        }
    }

    pub(super) fn sub(&self, other: &Self) -> Option<Self> {
        self.add(&other.clone().with_sign(!other.negative))
    }

    pub(super) fn mul(&self, other: &Self) -> Option<Self> {
        let len = self.limbs.len().checked_add(other.limbs.len())?;
        if len == 0 {
            return Some(Self {
                negative: false,
                limbs: Vec::new(),
            });
        }
        let mut result = Vec::new();
        result.try_reserve_exact(len).ok()?;
        result.resize(len, 0);
        for (i, left) in self.limbs.iter().enumerate() {
            let mut carry = 0_u64;
            for (j, right) in other.limbs.iter().enumerate() {
                let index = i.checked_add(j)?;
                let product = u64::from(*left)
                    .checked_mul(u64::from(*right))?
                    .checked_add(u64::from(*result.get(index)?))?
                    .checked_add(carry)?;
                *result.get_mut(index)? = u32::try_from(product & u64::from(u32::MAX)).ok()?;
                carry = product >> 32;
            }
            let index = i.checked_add(other.limbs.len())?;
            *result.get_mut(index)? = u32::try_from(carry).ok()?;
        }
        Self::normalize(&mut result);
        Some(Self {
            negative: self.negative != other.negative,
            limbs: result,
        })
    }

    pub(super) fn div_rem_abs(
        numerator: &[u32],
        denominator: &[u32],
    ) -> Option<(Vec<u32>, Vec<u32>)> {
        if denominator.is_empty() {
            return None;
        }
        if Self::cmp_abs(numerator, denominator) == Ordering::Less {
            return Some((Vec::new(), numerator.to_vec()));
        }
        let last = numerator.len().checked_sub(1)?;
        let top = *numerator.get(last)?;
        let bits = last
            .checked_mul(32)?
            .checked_add(usize::try_from(32 - top.leading_zeros()).ok()?)?;
        let quotient_len = bits.checked_add(31)?.checked_div(32)?;
        let mut quotient = Vec::new();
        quotient.try_reserve_exact(quotient_len).ok()?;
        quotient.resize(quotient_len, 0);
        let mut remainder = Vec::new();
        remainder
            .try_reserve_exact(denominator.len().checked_add(1)?)
            .ok()?;
        for bit in (0..bits).rev() {
            Self::shift_left_one(&mut remainder)?;
            if Self::bit(numerator, bit) {
                if remainder.is_empty() {
                    remainder.push(1);
                } else {
                    *remainder.get_mut(0)? |= 1;
                }
            }
            if Self::cmp_abs(&remainder, denominator) != Ordering::Less {
                remainder = Self::sub_abs(&remainder, denominator)?;
                let limb = quotient.get_mut(bit / 32)?;
                *limb |= 1_u32.checked_shl(u32::try_from(bit % 32).ok()?)?;
            }
        }
        Self::normalize(&mut quotient);
        Self::normalize(&mut remainder);
        Some((quotient, remainder))
    }

    fn bit(limbs: &[u32], index: usize) -> bool {
        limbs
            .get(index / 32)
            .is_some_and(|limb| limb & (1_u32 << (index % 32)) != 0)
    }

    fn shift_left_one(limbs: &mut Vec<u32>) -> Option<()> {
        let mut carry = 0_u32;
        for limb in limbs.iter_mut() {
            let next = *limb >> 31;
            *limb = (*limb << 1) | carry;
            carry = next;
        }
        if carry != 0 {
            limbs.try_reserve(1).ok()?;
            limbs.push(carry);
        }
        Some(())
    }

    pub(super) fn shift_left(&self, shift: usize) -> Option<Self> {
        if self.limbs.is_empty() {
            return Some(Self {
                negative: false,
                limbs: Vec::new(),
            });
        }
        let words = shift / 32;
        let bits = u32::try_from(shift % 32).ok()?;
        let len = self.limbs.len().checked_add(words)?.checked_add(1)?;
        let mut limbs = Vec::new();
        limbs.try_reserve_exact(len).ok()?;
        limbs.resize(words, 0);
        let mut carry = 0_u64;
        for limb in &self.limbs {
            let value = (u64::from(*limb) << bits) | carry;
            limbs.push(u32::try_from(value & u64::from(u32::MAX)).ok()?);
            carry = value >> 32;
        }
        if carry != 0 {
            limbs.push(u32::try_from(carry).ok()?);
        }
        Some(Self {
            negative: self.negative,
            limbs,
        })
    }

    pub(super) fn shift_right_arithmetic(&self, shift: usize) -> Option<Self> {
        if self.limbs.is_empty() {
            return Some(Self {
                negative: false,
                limbs: Vec::new(),
            });
        }
        let words = shift / 32;
        let bits = u32::try_from(shift % 32).ok()?;
        let mut discarded = self.limbs.iter().take(words).any(|limb| *limb != 0);
        if bits != 0
            && let Some(limb) = self.limbs.get(words)
        {
            discarded |= limb & ((1_u32 << bits) - 1) != 0;
        }
        let mut limbs = if words >= self.limbs.len() {
            Vec::new()
        } else {
            let mut result = Vec::new();
            result.try_reserve_exact(self.limbs.len() - words).ok()?;
            result.extend(self.limbs.iter().skip(words).copied());
            result
        };
        if bits != 0 {
            let mut carry = 0_u32;
            for limb in limbs.iter_mut().rev() {
                let next = *limb << (32 - bits);
                *limb = (*limb >> bits) | carry;
                carry = next;
            }
        }
        if self.negative && discarded {
            Self::add_one(&mut limbs)?;
        }
        Self::normalize(&mut limbs);
        Some(Self {
            negative: self.negative && !limbs.is_empty(),
            limbs,
        })
    }

    fn add_one(limbs: &mut Vec<u32>) -> Option<()> {
        let mut carry = true;
        for limb in limbs.iter_mut() {
            if !carry {
                break;
            }
            let (next, overflow) = limb.overflowing_add(1);
            *limb = next;
            carry = overflow;
        }
        if carry {
            limbs.try_reserve(1).ok()?;
            limbs.push(1);
        }
        Some(())
    }
}
