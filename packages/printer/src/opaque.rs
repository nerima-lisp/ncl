//! Opaque `#<...>` printing for objects without a reader syntax.

use ncl_object::Word;

use crate::error::PrintError;
use crate::print::Printer;

impl Printer<'_> {
    /// Print an unreadable object as `#<LABEL 0xADDRESS>`.
    ///
    /// # Errors
    ///
    /// Returns [`PrintError::NotReadable`] when `*print-readably*` is true,
    /// matching the standard's requirement to signal for unreadable objects.
    pub fn print_opaque(&mut self, label: &str, word: Word) -> Result<(), PrintError> {
        if self.options.readably() {
            return Err(PrintError::NotReadable);
        }
        self.write_str("#<")?;
        self.write_str(label)?;
        self.write_char(' ')?;
        let address = format!("{:#x}", word.address());
        self.write_str(&address)?;
        self.write_char('>')
    }
}
