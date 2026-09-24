//! List printing.

use ncl_object::{ObjectRef, Word, car, cdr, classify_object};

use crate::error::PrintError;
use crate::print::Printer;

impl Printer<'_> {
    /// Print a cons as a list, a dotted pair, or a quote abbreviation.
    pub fn print_cons(&mut self, list: Word) -> Result<(), PrintError> {
        if let Some(abbreviation) = self.abbreviation(list)? {
            self.write_str(abbreviation)?;
            let tail = cdr(self.ctx, list)?;
            let quoted = car(self.ctx, tail)?;
            return self.print(quoted);
        }
        self.write_char('(')?;
        let indent = self.column;
        let mut cursor = list;
        let mut count = 0usize;
        let mut first = true;
        loop {
            if let Some(limit) = self.options.length
                && count == limit
            {
                if count > 0 {
                    self.write_char(' ')?;
                }
                self.write_str("...")?;
                break;
            }
            if !first {
                self.separator(indent)?;
            }
            first = false;
            let head = car(self.ctx, cursor)?;
            self.print(head)?;
            count += 1;
            let tail = cdr(self.ctx, cursor)?;
            if tail == Word::NIL {
                break;
            }
            if tail.is_cons() && !self.tail_is_shared(tail) {
                cursor = tail;
            } else {
                self.write_str(" . ")?;
                self.print(tail)?;
                break;
            }
        }
        self.write_char(')')
    }

    /// Detect the `'` and `#'` reader abbreviations.
    fn abbreviation(&mut self, list: Word) -> Result<Option<&'static str>, PrintError> {
        let head = car(self.ctx, list)?;
        if !matches!(classify_object(self.ctx, head), ObjectRef::Symbol(_)) {
            return Ok(None);
        }
        let tail = cdr(self.ctx, list)?;
        if tail == Word::NIL || !tail.is_cons() || cdr(self.ctx, tail)? != Word::NIL {
            return Ok(None);
        }
        match self.symbol_text(head)?.as_str() {
            "QUOTE" => Ok(Some("'")),
            "FUNCTION" => Ok(Some("#'")),
            _ => Ok(None),
        }
    }
}
