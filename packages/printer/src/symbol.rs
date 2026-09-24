//! Symbol printing.

use ncl_object::{Package, Word, symbol_package};

use crate::error::PrintError;
use crate::options::PrintCase;
use crate::print::Printer;

impl Printer<'_> {
    /// Print a symbol with its package prefix and escaping.
    pub fn print_symbol(&mut self, symbol: Word) -> Result<(), PrintError> {
        let name = self.symbol_text(symbol)?;
        let package = symbol_package(self.ctx, symbol)?;
        if package == Word::NIL {
            if self.options.gensym {
                self.write_str("#:")?;
            }
            let rendered = render_name(&name, self.options.escape, self.options.case);
            return self.write_str(&rendered);
        }
        let package_name = {
            let name = Package::from(package).name(&*self.ctx)?;
            self.string_text(name)?
        };
        if package_name == "KEYWORD" {
            self.write_str(":")?;
        } else if package_name != "COMMON-LISP" {
            self.write_str(&package_name)?;
            self.write_str(":")?;
        }
        let rendered = render_name(&name, self.options.escape, self.options.case);
        self.write_str(&rendered)
    }
}

/// Render a symbol name with `|...|` escaping or case conversion.
fn render_name(name: &str, escape: bool, case: PrintCase) -> String {
    if escape && needs_escape(name, case) {
        let mut text = String::with_capacity(name.len() + 2);
        text.push('|');
        for character in name.chars() {
            if character == '|' || character == '\\' {
                text.push('\\');
            }
            text.push(character);
        }
        text.push('|');
        text
    } else {
        apply_case(name, case)
    }
}

fn apply_case(name: &str, case: PrintCase) -> String {
    match case {
        PrintCase::Upcase => name.to_uppercase(),
        PrintCase::Downcase => name.to_lowercase(),
        PrintCase::Capitalize => capitalize(name),
    }
}

fn capitalize(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    let mut at_word_start = true;
    for character in name.chars() {
        if character.is_alphanumeric() {
            if at_word_start {
                result.extend(character.to_uppercase());
            } else {
                result.extend(character.to_lowercase());
            }
            at_word_start = false;
        } else {
            result.push(character);
            at_word_start = true;
        }
    }
    result
}

/// Whether a name must be wrapped in `|...|` to read back unchanged.
fn needs_escape(name: &str, case: PrintCase) -> bool {
    if name.is_empty() || name == "." {
        return true;
    }
    if apply_case(name, case) != name {
        return true;
    }
    if name.chars().any(|character| {
        character.is_whitespace()
            || matches!(
                character,
                '|' | '(' | ')' | '\'' | '"' | '`' | ',' | ';' | '#' | '\\'
            )
    }) {
        return true;
    }
    name.chars().any(|character| character.is_ascii_digit())
        && name
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, '+' | '-' | '.'))
}
