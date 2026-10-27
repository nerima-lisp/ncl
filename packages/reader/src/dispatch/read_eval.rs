use ncl_object::{HeapRootedWord, Runtime, ThreadContext, Word};

use crate::error::ReadError;
use crate::input::CharSource;
use crate::reader::{ReadEvaluation, ReadOptions, read_form};
use crate::token::intern_common_lisp;
use crate::token::{is_read_suppressed, refresh_dynamic_reader_options};

pub(super) fn read_eval_dispatch(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &HeapRootedWord,
    labels: &HeapRootedWord,
) -> Result<Word, ReadError> {
    if is_read_suppressed(ctx, runtime, opts)? {
        if read_form(ctx, runtime, source, opts, rt, labels)?.is_none() {
            return Err(ReadError::UnexpectedEof);
        }
        return Ok(Word::NIL);
    }
    let read_eval_enabled = match opts.read_evaluation() {
        ReadEvaluation::Enabled => true,
        ReadEvaluation::Disabled => false,
        ReadEvaluation::Dynamic => {
            let symbol = intern_common_lisp(ctx, runtime, "*READ-EVAL*")?;
            let symbol = HeapRootedWord::new(ctx, symbol)?;
            ncl_object::symbol_value(ctx, symbol.get())? != Word::NIL
        }
    };
    if !read_eval_enabled {
        return Err(ReadError::ReadEvalDisabled);
    }
    let Some(form) = read_form(ctx, runtime, source, opts, rt, labels)? else {
        return Err(ReadError::UnexpectedEof);
    };
    if is_read_suppressed(ctx, runtime, opts)? {
        return Ok(Word::NIL);
    }
    if ctx.evaluator_runtime().is_none() {
        return Err(ReadError::ReadEvalUnavailable);
    }
    let form = HeapRootedWord::new(ctx, form)?;
    let eval = intern_common_lisp(ctx, runtime, "EVAL")?;
    let eval = HeapRootedWord::new(ctx, eval)?;
    let arguments = [form.get()];
    let mut caller = ncl_object::BuiltinFunctionCaller;
    let mut values = ncl_object::MultipleValues::default();
    let value = ncl_object::FunctionCaller::call_function(
        &mut caller,
        ctx,
        runtime,
        ncl_object::FunctionDesignator::Symbol(ncl_object::Symbol::from_word(eval.get())),
        ncl_object::FunctionArguments::new(&arguments),
        &mut values,
    )?;
    let value = HeapRootedWord::new(ctx, value)?;
    refresh_dynamic_reader_options(ctx, runtime, opts)?;
    Ok(value.get())
}
