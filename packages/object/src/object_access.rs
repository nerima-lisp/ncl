use crate::{ObjectError, ThreadContext};
use ncl_sys::Word;

#[macro_export]
macro_rules! word_newtype {
    ($name:ident) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $name(Word);
        impl From<Word> for $name {
            fn from(value: Word) -> Self {
                Self(value)
            }
        }
        impl From<$name> for Word {
            fn from(value: $name) -> Self {
                value.0
            }
        }
        impl $name {
            #[must_use]
            pub const fn as_word(self) -> Word {
                self.0
            }
        }
    };
}

pub fn put(
    ctx: &mut ThreadContext,
    object: Word,
    slot: usize,
    value: Word,
) -> Result<(), ObjectError> {
    ctx.write_object_slot(object, slot, value)
}

pub fn get(ctx: &ThreadContext, object: Word, tag: u8, slot: usize) -> Result<Word, ObjectError> {
    if ncl_sys::object_widetag(&ctx.thread, object) != Some(tag) {
        return Err(ObjectError::TypeError);
    }
    ncl_sys::read_object_word(&ctx.thread, object, slot).ok_or(ObjectError::Storage(
        ncl_sys::StorageCondition::ThreadNotRegistered,
    ))
}

pub fn fix(value: usize) -> Result<Word, ObjectError> {
    Ok(Word::fixnum(
        i64::try_from(value).map_err(|_| ObjectError::Layout)?,
    ))
}

pub fn get_function(ctx: &ThreadContext, object: Word, slot: usize) -> Result<Word, ObjectError> {
    match ncl_sys::object_widetag(&ctx.thread, object) {
        Some(crate::widetag::SIMPLE_FUN | crate::widetag::CLOSURE) => {
            ncl_sys::read_object_word(&ctx.thread, object, slot).ok_or(ObjectError::Layout)
        }
        _ => Err(ObjectError::TypeError),
    }
}
