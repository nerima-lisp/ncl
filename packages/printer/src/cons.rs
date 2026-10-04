//! List printing.

use ncl_object::{ObjectRef, Word, car, cdr, classify_object, symbol_package};

use crate::error::PrintError;
use crate::print::Printer;

impl Printer<'_> {
    /// Print a cons as a list, a dotted pair, or a quote abbreviation.
    pub fn print_cons(&mut self, list: Word) -> Result<(), PrintError> {
        if self.options.pretty() && self.print_standard_operator_form(list)? {
            return Ok(());
        }
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
            if let Some(limit) = self.options.length().map(crate::options::NonNegative::get)
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

    fn print_standard_operator_form(&mut self, list: Word) -> Result<bool, PrintError> {
        let head = car(self.ctx, list)?;
        if !matches!(classify_object(self.ctx, head), ObjectRef::Symbol(_)) {
            return Ok(false);
        }
        let package = symbol_package(&*self.ctx, head).ok();
        let common_lisp = package.is_some_and(|package| {
            self.runtime.find_package(&*self.ctx, "COMMON-LISP") == Some(package)
        });
        if !common_lisp || !matches!(self.symbol_text(head)?.as_str(), "LET" | "LET*" | "DEFUN") {
            return Ok(false);
        }
        let mut cursor = list;
        while cursor != Word::NIL {
            if !cursor.is_cons() {
                return Ok(false);
            }
            cursor = cdr(self.ctx, cursor)?;
        }

        self.write_char('(')?;
        self.print(head)?;
        cursor = cdr(self.ctx, list)?;
        let mut first = true;
        while cursor != Word::NIL {
            let item = car(self.ctx, cursor)?;
            if first {
                self.write_char(' ')?;
                first = false;
            } else {
                self.separator(2)?;
            }
            self.print(item)?;
            cursor = cdr(self.ctx, cursor)?;
        }
        self.write_char(')')?;
        Ok(true)
    }

    /// Detect the `'` and `#'` reader abbreviations.
    fn abbreviation(&self, list: Word) -> Result<Option<&'static str>, PrintError> {
        let head = car(self.ctx, list)?;
        if !matches!(classify_object(self.ctx, head), ObjectRef::Symbol(_)) {
            return Ok(None);
        }
        let tail = cdr(self.ctx, list)?;
        if tail == Word::NIL || !tail.is_cons() || cdr(self.ctx, tail)? != Word::NIL {
            return Ok(None);
        }
        let name = self.symbol_text(head)?;
        let package = symbol_package(&*self.ctx, head).ok();
        let common_lisp = package.is_some_and(|package| {
            self.runtime.find_package(&*self.ctx, "COMMON-LISP") == Some(package)
        });
        match (common_lisp, name.as_str()) {
            (true, "QUOTE") => Ok(Some("'")),
            (true, "FUNCTION") => Ok(Some("#'")),
            _ => Ok(None),
        }
    }
}
