//! Number printing.

use ncl_object::{
    Bignum, Complex, DoubleFloat, Ratio, bignum_limbs, bignum_sign, complex_imag, complex_real,
    double_value, ratio_denominator, ratio_numerator,
};

use crate::error::PrintError;
use crate::print::Printer;

impl Printer<'_> {
    /// Print a fixnum in `*print-base*`.
    pub fn print_fixnum(&mut self, value: i64) -> Result<(), PrintError> {
        self.write_str(&self.radix_prefix())?;
        let text = format_integer(value, self.options.base);
        self.write_str(&text)?;
        if self.options.radix && self.options.base == 10 {
            self.write_char('.')?;
        }
        Ok(())
    }

    /// Print a bignum in `*print-base*`.
    pub fn print_bignum(&mut self, number: Bignum) -> Result<(), PrintError> {
        let limbs = bignum_limbs(self.ctx, number)?;
        let negative = bignum_sign(self.ctx, number)?;
        self.write_str(&self.radix_prefix())?;
        self.write_str(&format_limbs(&limbs, negative, self.options.base))?;
        if self.options.radix && self.options.base == 10 {
            self.write_char('.')?;
        }
        Ok(())
    }

    /// Print a ratio as `numerator/denominator`.
    pub fn print_ratio(&mut self, number: Ratio) -> Result<(), PrintError> {
        let numerator = ratio_numerator(self.ctx, number)?;
        let denominator = ratio_denominator(self.ctx, number)?;
        self.print(numerator)?;
        self.write_char('/')?;
        self.print(denominator)
    }

    /// Print a double float in its shortest round-tripping form.
    pub fn print_double(&mut self, number: DoubleFloat) -> Result<(), PrintError> {
        let value = double_value(self.ctx, number)?;
        self.write_str(&format_float(value))
    }

    /// Print a complex as `#C(real imaginary)`.
    pub fn print_complex(&mut self, number: Complex) -> Result<(), PrintError> {
        let real = complex_real(self.ctx, number)?;
        let imaginary = complex_imag(self.ctx, number)?;
        self.write_str("#C(")?;
        self.print(real)?;
        self.write_char(' ')?;
        self.print(imaginary)?;
        self.write_char(')')
    }

    /// The radix prefix for the current `*print-radix*` and `*print-base*`.
    fn radix_prefix(&self) -> String {
        if !self.options.radix {
            return String::new();
        }
        match self.options.base {
            2 => "#b".to_string(),
            8 => "#o".to_string(),
            16 => "#x".to_string(),
            10 => String::new(),
            base => format!("#{base}r"),
        }
    }
}

/// Format an integer in `base`, which is clamped to 2 through 36.
fn format_integer(value: i64, base: u32) -> String {
    let base = base.clamp(2, 36);
    let negative = value < 0;
    let mut magnitude = value.unsigned_abs();
    let divisor = u64::from(base);
    let mut digits = Vec::new();
    if magnitude == 0 {
        digits.push('0');
    }
    while magnitude > 0 {
        digits.push(digit_character(magnitude % divisor));
        magnitude /= divisor;
    }
    digits.reverse();
    let mut text = String::with_capacity(digits.len() + 1);
    if negative {
        text.push('-');
    }
    text.extend(digits);
    text
}

/// Format little-endian limbs in `base`, which is clamped to 2 through 36.
fn format_limbs(limbs: &[u32], negative: bool, base: u32) -> String {
    let base = base.clamp(2, 36);
    let divisor = u64::from(base);
    let mut limbs = limbs.to_vec();
    while limbs.last() == Some(&0) {
        limbs.pop();
    }
    if limbs.is_empty() {
        return "0".to_string();
    }
    let mut digits = Vec::new();
    loop {
        let mut remainder = 0u64;
        let mut all_zero = true;
        for limb in limbs.iter_mut().rev() {
            let value = (remainder << 32) | u64::from(*limb);
            *limb = u32::try_from(value / divisor).unwrap_or(0);
            remainder = value % divisor;
            all_zero &= *limb == 0;
        }
        digits.push(digit_character(remainder));
        if all_zero {
            break;
        }
    }
    digits.reverse();
    let mut text = String::with_capacity(digits.len() + 1);
    if negative {
        text.push('-');
    }
    text.extend(digits);
    text
}

fn digit_character(digit: u64) -> char {
    char::from_digit(u32::try_from(digit).unwrap_or(0), 36).unwrap_or('0')
}

/// Render a double float so it reads back as the same value.
fn format_float(value: f64) -> String {
    if value.is_nan() {
        return "#<DOUBLE-FLOAT NaN>".to_string();
    }
    if value.is_infinite() {
        let sign = if value.is_sign_negative() { "-" } else { "" };
        return format!("#<DOUBLE-FLOAT {sign}Infinity>");
    }
    let text = format!("{value}");
    if text.contains(['.', 'e', 'E']) {
        text
    } else {
        format!("{text}.0")
    }
}
