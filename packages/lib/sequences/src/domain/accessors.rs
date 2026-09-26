use super::{list_word, object_car, object_cdr, proper_list};
use ncl_object::{Fixnum, LispError, List, Runtime, ThreadContext, Word};

fn nth_word(ctx: &mut ThreadContext, index: i64, mut cursor: Word) -> Result<Word, LispError> {
    if index < 0 {
        return Err(LispError::TypeError {
            datum: Word::fixnum(index),
            expected: ncl_object::ObjectType::Fixnum,
        });
    }
    for _ in 0..index {
        if cursor == Word::NIL {
            return Ok(Word::NIL);
        }
        cursor = object_cdr(ctx, cursor)?;
    }
    if cursor == Word::NIL {
        Ok(Word::NIL)
    } else {
        Ok(object_car(ctx, cursor)?)
    }
}

pub fn nth(
    ctx: &mut ThreadContext,
    _: &Runtime,
    index: Fixnum,
    value: List,
) -> Result<Word, LispError> {
    let cursor = list_word(value);
    proper_list(ctx, cursor)?;
    nth_word(ctx, index.value(), cursor)
}

pub fn nthcdr(
    ctx: &mut ThreadContext,
    _: &Runtime,
    index: Fixnum,
    value: List,
) -> Result<Word, LispError> {
    if index.value() < 0 {
        return Err(LispError::TypeError {
            datum: index.as_word(),
            expected: ncl_object::ObjectType::Fixnum,
        });
    }
    let mut cursor = list_word(value);
    for _ in 0..index.value() {
        if cursor == Word::NIL {
            return Ok(Word::NIL);
        }
        cursor = object_cdr(ctx, cursor)?;
    }
    proper_list(ctx, cursor)?;
    Ok(cursor)
}

pub fn list_length(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: List,
) -> Result<Word, LispError> {
    super::length(ctx, runtime, super::Sequence::List(value))
}

macro_rules! list_nth { ($($name:ident => $index:literal),+ $(,)?) => { $(pub fn $name(ctx: &mut ThreadContext, _: &Runtime, value: List) -> Result<Word, LispError> { let cursor = list_word(value); proper_list(ctx, cursor)?; nth_word(ctx, $index, cursor) })+ }; }
list_nth!(first => 0, second => 1, third => 2, fourth => 3, fifth => 4, sixth => 5, seventh => 6, eighth => 7, ninth => 8, tenth => 9);
