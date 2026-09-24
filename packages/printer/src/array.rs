//! Array and vector printing.

use ncl_object::{Word, simple_vector_length, simple_vector_ref};

use crate::error::PrintError;
use crate::print::Printer;

impl Printer<'_> {
    /// Print a simple vector as `#(element ...)`.
    pub fn print_simple_vector(&mut self, vector: Word) -> Result<(), PrintError> {
        if !self.options.array {
            return self.print_opaque("VECTOR", vector);
        }
        let length = simple_vector_length(self.ctx, vector)?;
        self.write_str("#(")?;
        self.print_elements(length, |printer, index| {
            simple_vector_ref(printer.ctx, vector, index).map_err(PrintError::from)
        })?;
        self.write_char(')')
    }

    /// Print `length` elements separated by spaces, honoring the length limits.
    ///
    /// `element` reads the element at an index. The `*print-length*` and
    /// `SB-EXT:*PRINT-VECTOR-LENGTH*` limits apply, printing `...` once the
    /// limit is reached.
    pub fn print_elements(
        &mut self,
        length: usize,
        mut element: impl FnMut(&mut Self, usize) -> Result<Word, PrintError>,
    ) -> Result<(), PrintError> {
        let limit = self.options.vector_length.or(self.options.length);
        for index in 0..length {
            if let Some(limit) = limit
                && index == limit
            {
                if index > 0 {
                    self.write_char(' ')?;
                }
                return self.write_str("...");
            }
            if index > 0 {
                self.write_char(' ')?;
            }
            let value = element(self, index)?;
            self.print(value)?;
        }
        Ok(())
    }
}
