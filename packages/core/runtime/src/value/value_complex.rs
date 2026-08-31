use super::Value;

/// A Common Lisp complex number.
#[derive(Clone, Debug)]
pub struct Complex {
    pub real: Value,
    pub imag: Value,
}

impl Complex {
    #[must_use]
    pub fn new(real: Value, imag: Value) -> Self {
        Self { real, imag }
    }
}

impl Value {
    /// Constructs a complex value from real-valued components.
    #[must_use]
    pub fn complex(real: Value, imag: Value) -> Self {
        Self::Complex(std::rc::Rc::new(Complex::new(real, imag)))
    }

    /// Returns the complex components when this is a complex value.
    #[must_use]
    pub fn as_complex(&self) -> Option<&Complex> {
        match self {
            Self::Complex(value) => Some(value),
            _ => None,
        }
    }
}
