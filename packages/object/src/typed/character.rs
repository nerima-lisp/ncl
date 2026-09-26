//! Character typed view over the tagged-word ABI.

use ncl_sys::Word;

use super::error::{ObjectType, TypeError};

/// A character view validated at the builtin boundary.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Character(u32);
impl Character {
    /// Validate and wrap a tagged character.
    ///
    /// # Errors
    ///
    /// Returns a type error when the word is not a character.
    pub fn try_from_word(word: Word) -> Result<Self, TypeError> {
        word.as_character().map(Self).ok_or(TypeError {
            datum: word,
            expected: ObjectType::Character,
        })
    }
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
    #[must_use]
    pub const fn as_word(self) -> Word {
        Word::character(self.0)
    }
}
