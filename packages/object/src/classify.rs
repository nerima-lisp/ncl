use crate::layout::widetag;
use crate::ThreadContext;
use ncl_sys::{LowTag, Word};

/// Classification of a tagged value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ObjectRef {
    Fixnum(i64),
    Character(u32),
    Cons(Word),
    Symbol(Word),
    HashTable(Word),
    String(Word),
    SimpleVector(Word),
    SpecializedArray(Word),
    Array(Word),
    Function(Word),
    Closure(Word),
    Instance(Word),
    Structure(Word),
    Bignum(Word),
    Ratio(Word),
    DoubleFloat(Word),
    Complex(Word),
    Package(Word),
    Readtable(Word),
    Stream(Word),
    Code(Word),
    Other { word: Word, widetag: u8 },
    Immediate(Word),
}

/// Classify a tagged value using lowtag information.
#[must_use]
pub fn classify(word: Word) -> ObjectRef {
    if let Some(value) = word.as_fixnum() {
        return ObjectRef::Fixnum(value);
    }
    if word.is_unbound() {
        return ObjectRef::Immediate(word);
    }
    if let Some(value) = word.as_character() {
        return ObjectRef::Character(value);
    }
    match word.lowtag() {
        x if x == LowTag::List as u8 => {
            if word == Word::NIL {
                ObjectRef::Symbol(word)
            } else {
                ObjectRef::Cons(word)
            }
        }
        x if x == LowTag::Function as u8 => ObjectRef::Function(word),
        x if x == LowTag::Instance as u8 => ObjectRef::Instance(word),
        _ => ObjectRef::Other { word, widetag: 0 },
    }
}

/// Classify a heap object by registered widetag.
#[must_use]
pub fn classify_object(ctx: &ThreadContext, word: Word) -> ObjectRef {
    match ncl_sys::object_widetag(&ctx.thread, word) {
        Some(widetag::SYMBOL) => ObjectRef::Symbol(word),
        Some(widetag::STRING) => ObjectRef::String(word),
        Some(widetag::SIMPLE_VECTOR) => ObjectRef::SimpleVector(word),
        Some(widetag::SPECIALIZED_ARRAY) => ObjectRef::SpecializedArray(word),
        Some(widetag::NON_SIMPLE_ARRAY) => ObjectRef::Array(word),
        Some(widetag::HASH_TABLE) => ObjectRef::HashTable(word),
        Some(widetag::STRUCTURE) => ObjectRef::Structure(word),
        Some(widetag::INSTANCE) => ObjectRef::Instance(word),
        Some(widetag::SIMPLE_FUN) => ObjectRef::Function(word),
        Some(widetag::CLOSURE) => ObjectRef::Closure(word),
        Some(widetag::BIGNUM) => ObjectRef::Bignum(word),
        Some(widetag::RATIO) => ObjectRef::Ratio(word),
        Some(widetag::DOUBLE_FLOAT) => ObjectRef::DoubleFloat(word),
        Some(widetag::COMPLEX) => ObjectRef::Complex(word),
        Some(widetag::PACKAGE) => ObjectRef::Package(word),
        Some(widetag::READTABLE) => ObjectRef::Readtable(word),
        Some(widetag::STREAM) => ObjectRef::Stream(word),
        Some(widetag::CODE) => ObjectRef::Code(word),
        Some(tag) => ObjectRef::Other { word, widetag: tag },
        None => classify(word),
    }
}
