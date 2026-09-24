//! String and character printing.

use ncl_object::Word;

use crate::error::PrintError;
use crate::print::Printer;

impl Printer<'_> {
    /// Print a string, escaped when `*print-escape*` is true.
    pub fn print_string(&mut self, string: Word) -> Result<(), PrintError> {
        let text = self.string_text(string)?;
        if !self.options.escape {
            return self.write_str(&text);
        }
        self.write_char('"')?;
        for character in text.chars() {
            match character {
                '"' => self.write_str("\\\"")?,
                '\\' => self.write_str("\\\\")?,
                other => self.write_char(other)?,
            }
        }
        self.write_char('"')
    }

    /// Print a character as `#\name`, or bare when escaping is off.
    pub fn print_character(&mut self, code: u32) -> Result<(), PrintError> {
        let Some(character) = char::from_u32(code) else {
            return self.write_str("#\\?");
        };
        if !self.options.escape {
            return self.write_char(character);
        }
        self.write_str("#\\")?;
        match character {
            ' ' => self.write_str("Space"),
            '\n' => self.write_str("Newline"),
            '\t' => self.write_str("Tab"),
            '\r' => self.write_str("Return"),
            '\u{8}' => self.write_str("Backspace"),
            '\u{c}' => self.write_str("Page"),
            '\u{7f}' => self.write_str("Rubout"),
            '\0' => self.write_str("Null"),
            other => self.write_char(other),
        }
    }
}
