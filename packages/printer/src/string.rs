//! String and character printing.

use ncl_object::Word;

use crate::error::PrintError;
use crate::print::Printer;

impl Printer<'_> {
    /// Print a string, escaped when `*print-escape*` is true.
    pub fn print_string(&mut self, string: Word) -> Result<(), PrintError> {
        let text = self.string_text(string)?;
        if !self.options.escape() {
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
        if !self.options.escape() {
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on setup and output"
)]
mod tests {
    use super::super::{PrintOptions, StringSink, write};
    use crate::print::Printer;
    use ncl_object::{Runtime, ThreadContext, Word, make_string};

    #[test]
    fn escaped_strings_cover_backslash_and_raw_control_paths() {
        let runtime = Runtime::new().unwrap();
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).unwrap();
        let text = make_string(&mut ctx, &runtime, &['\\', '"', 'x']).unwrap();
        let mut sink = StringSink::new();
        write(&mut ctx, &runtime, text, &mut sink, &PrintOptions::new()).unwrap();
        assert_eq!(sink.into_string(), "\"\\\\\\\"x\"");
        let mut sink = StringSink::new();
        write(
            &mut ctx,
            &runtime,
            text,
            &mut sink,
            &PrintOptions::new().with_escape(false),
        )
        .unwrap();
        assert_eq!(sink.into_string(), "\\\"x");
    }

    #[test]
    fn named_and_invalid_characters_have_stable_output() {
        let runtime = Runtime::new().unwrap();
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).unwrap();
        for (code, expected) in [
            (32, "#\\Space"),
            (10, "#\\Newline"),
            (9, "#\\Tab"),
            (0, "#\\Null"),
        ] {
            let mut sink = StringSink::new();
            write(
                &mut ctx,
                &runtime,
                Word::character(code),
                &mut sink,
                &PrintOptions::new(),
            )
            .unwrap();
            assert_eq!(sink.into_string(), expected);
        }
        let mut sink = StringSink::new();
        let mut printer = Printer::new(&mut ctx, &runtime, &mut sink, PrintOptions::new());
        printer.print_character(0x11_0000).unwrap();
        assert_eq!(sink.into_string(), "#\\?");
    }
}
