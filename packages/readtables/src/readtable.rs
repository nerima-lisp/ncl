use ncl_object::{
    ObjectError, Runtime, ThreadContext, Word, allocate, get, put, readtable_offset, widetag,
    with_roots,
};

/// A typed handle to a Lisp readtable object.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Readtable(Word);

impl Readtable {
    /// Construct a readtable handle from its tagged word.
    #[must_use]
    pub const fn from_word(value: Word) -> Self {
        Self(value)
    }

    /// Return the tagged word represented by this handle.
    #[must_use]
    pub const fn as_word(self) -> Word {
        self.0
    }
}

impl From<Readtable> for Word {
    fn from(value: Readtable) -> Self {
        value.0
    }
}

/// Allocate a readtable descriptor.
///
/// # Errors
/// Returns an error when allocation fails.
pub fn make_readtable(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    syntax: Word,
    dispatch: Word,
    case_mode: Word,
) -> Result<Readtable, ObjectError> {
    with_roots(ctx, &[syntax, dispatch, case_mode], |ctx, values| {
        let object = allocate(ctx, runtime, widetag::READTABLE, 3)?;
        for (i, v) in values.iter().copied().enumerate() {
            put(ctx, object, i, *v)?;
        }
        Ok(Readtable::from_word(object))
    })
}
/// Read a raw readtable slot.
///
/// # Errors
/// Returns an error when the object or slot is invalid.
pub fn readtable_slot(
    ctx: &ThreadContext,
    object: Readtable,
    slot: usize,
) -> Result<Word, ObjectError> {
    get(ctx, object.into(), widetag::READTABLE, slot)
}

macro_rules! readtable_accessor {
    ($name:ident, $slot:expr, $doc:literal) => {
        #[doc = $doc]
        ///
        /// # Errors
        /// Returns an error when the object is not a readtable.
        pub fn $name(ctx: &ThreadContext, object: Readtable) -> Result<Word, ObjectError> {
            readtable_slot(ctx, object, $slot)
        }
    };
}
readtable_accessor!(
    readtable_syntax,
    readtable_offset::SYNTAX,
    "Read a readtable syntax table."
);
readtable_accessor!(
    readtable_dispatch,
    readtable_offset::DISPATCH,
    "Read a readtable dispatch table."
);
readtable_accessor!(
    readtable_case,
    readtable_offset::CASE,
    "Read a readtable case mode."
);
