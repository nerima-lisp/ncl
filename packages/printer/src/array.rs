//! Array and vector printing.

use ncl_object::{
    Word, array_dimensions, array_row_major_ref, simple_vector_length, simple_vector_ref,
    specialized_array_ref,
};

use crate::circle::specialized_length;
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
        let indent = self.column;
        self.print_elements(indent, length, |printer, index| {
            simple_vector_ref(&*printer.ctx, vector, index).map_err(PrintError::from)
        })?;
        self.write_char(')')
    }

    /// Print a specialized array as `#(element ...)`.
    ///
    /// `ncl-object` exposes no length accessor, so the length comes from the
    /// probe in [`crate::circle::specialized_length`].
    pub fn print_specialized_array(&mut self, array: Word) -> Result<(), PrintError> {
        if !self.options.array {
            return self.print_opaque("ARRAY", array);
        }
        let length = specialized_length(&*self.ctx, array);
        self.write_str("#(")?;
        let indent = self.column;
        self.print_elements(indent, length, |printer, index| {
            specialized_array_ref(&*printer.ctx, array, index).map_err(PrintError::from)
        })?;
        self.write_char(')')
    }

    /// Print a non-simple array as `#(element ...)` or `#nA(element ...)`.
    pub fn print_array(&mut self, array: Word) -> Result<(), PrintError> {
        if !self.options.array {
            return self.print_opaque("ARRAY", array);
        }
        let dimensions = array_dimensions(&*self.ctx, array)?;
        let rank = dimensions.len();
        if rank == 1 {
            self.write_str("#(")?;
        } else {
            self.write_str(&format!("#{rank}A("))?;
        }
        let indent = self.column;
        let total: usize = dimensions.iter().product();
        self.print_elements(indent, total, |printer, index| {
            array_row_major_ref(&*printer.ctx, array, index).map_err(PrintError::from)
        })?;
        self.write_char(')')
    }

    /// Print `length` elements separated by spaces or pretty line breaks.
    ///
    /// `element` reads the element at an index. `*print-length*` and
    /// `SB-EXT:*PRINT-VECTOR-LENGTH*` print `...` once their limit is reached.
    pub fn print_elements(
        &mut self,
        indent: usize,
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
                self.separator(indent)?;
            }
            let value = element(self, index)?;
            self.print(value)?;
        }
        Ok(())
    }
}
