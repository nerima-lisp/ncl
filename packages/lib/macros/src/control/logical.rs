use super::{Result, bindings, form};
use crate::fresh_symbol;
use ncl_object::{HeapRootedWord, Runtime, ThreadContext, Word};

pub(super) fn and(ctx: &mut ThreadContext, runtime: &Runtime, values: &[HeapRootedWord]) -> Result {
    match values {
        [] => Ok(Word::TRUE),
        [value] => Ok(value.get()),
        [value, rest @ ..] => {
            let next = and(ctx, runtime, rest)?;
            form(ctx, runtime, "IF", &[value.get(), next, Word::NIL])
        }
    }
}

pub(super) fn or(ctx: &mut ThreadContext, runtime: &Runtime, values: &[HeapRootedWord]) -> Result {
    match values {
        [] => Ok(Word::NIL),
        [value] => Ok(value.get()),
        [value, rest @ ..] => {
            let temporary_word = fresh_symbol(ctx, runtime)?;
            let temporary = HeapRootedWord::new(ctx, temporary_word)?;
            let let_bindings = bindings(ctx, runtime, &[(temporary.get(), value.get())])?;
            let let_bindings = HeapRootedWord::new(ctx, let_bindings)?;
            let next = or(ctx, runtime, rest)?;
            let test = form(
                ctx,
                runtime,
                "IF",
                &[temporary.get(), temporary.get(), next],
            )?;
            form(ctx, runtime, "LET", &[let_bindings.get(), test])
        }
    }
}
