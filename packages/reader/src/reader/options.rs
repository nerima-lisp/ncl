use ncl_object::{HeapRootedWord, Runtime, ThreadContext, Word, symbol_value};

use crate::error::ReadError;
use crate::token::intern_common_lisp;

use super::{ReadOptions, ReadSuppression};

pub fn is_read_suppressed(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    opts: &ReadOptions,
) -> Result<bool, ReadError> {
    match opts.read_suppression() {
        ReadSuppression::Keep => Ok(false),
        ReadSuppression::Discard => Ok(true),
        ReadSuppression::Dynamic => {
            if let Some(suppressed) = opts.cached_read_suppression() {
                Ok(suppressed)
            } else {
                let suppressed = current_read_suppression(ctx, runtime)?;
                opts.cache_read_suppression(suppressed);
                Ok(suppressed)
            }
        }
    }
}

pub fn current_read_suppression(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> Result<bool, ReadError> {
    let symbol = intern_common_lisp(ctx, runtime, "*READ-SUPPRESS*")?;
    let symbol = HeapRootedWord::new(ctx, symbol)?;
    Ok(symbol_value(ctx, symbol.get())? != Word::NIL)
}
