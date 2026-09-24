//! Character input abstraction for the reader.

/// A source of characters consumed by the reader.
///
/// `read` takes `&mut dyn CharSource`, so a string and (once L9 lands) a Lisp
/// stream can drive the same reader. Implementations must honour the contract
/// that [`CharSource::unread_char`] receives only the most recently
/// [`CharSource::read_char`] character.
pub trait CharSource {
    /// Read and consume the next character, or `None` at end of input.
    fn read_char(&mut self) -> Option<char>;
    /// Peek at the next character without consuming it, or `None` at end of input.
    fn peek_char(&mut self) -> Option<char>;
    /// Push a character back so the next `read_char` returns it.
    fn unread_char(&mut self, ch: char);
}

/// A [`CharSource`] over a borrowed string slice.
///
/// Characters are decoded as Unicode scalar values. `unread_char` stores the
/// character on an internal stack rather than assuming the source can rewind.
#[derive(Clone, Debug)]
pub struct StringSource<'a> {
    iter: std::iter::Peekable<std::str::Chars<'a>>,
    pushed: Vec<char>,
}

impl<'a> StringSource<'a> {
    /// Create a source over `text`.
    #[must_use]
    pub fn new(text: &'a str) -> Self {
        Self {
            iter: text.chars().peekable(),
            pushed: Vec::new(),
        }
    }
}

impl CharSource for StringSource<'_> {
    fn read_char(&mut self) -> Option<char> {
        self.pushed.pop().or_else(|| self.iter.next())
    }

    fn peek_char(&mut self) -> Option<char> {
        if let Some(&ch) = self.pushed.last() {
            return Some(ch);
        }
        self.iter.peek().copied()
    }

    fn unread_char(&mut self, ch: char) {
        self.pushed.push(ch);
    }
}
