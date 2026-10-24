use ncl_object::{
    BuiltinArgs, HeapRootedWord, MultipleValues, ObjectError, Runtime, ThreadContext, Word,
};

type Result<T> = std::result::Result<T, ObjectError>;

fn hold(ctx: &ThreadContext, held: &mut Vec<HeapRootedWord>, value: Word) -> Result<usize> {
    held.push(HeapRootedWord::new(ctx, value)?);
    Ok(held.len() - 1)
}

fn hold_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<HeapRootedWord>,
    name: &str,
) -> Result<usize> {
    let value = super::symbol(ctx, runtime, name)?;
    hold(ctx, held, value)
}

fn hold_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<HeapRootedWord>,
    indexes: &[usize],
) -> Result<usize> {
    let values = indexes
        .iter()
        .map(|index| {
            held.get(*index)
                .map(HeapRootedWord::get)
                .ok_or(ObjectError::TypeError)
        })
        .collect::<Result<Vec<_>>>()?;
    let value = super::list(ctx, runtime, &values)?;
    hold(ctx, held, value)
}

pub fn declaim_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word> {
    let form = args.get(0).ok_or(ObjectError::TypeError)?;
    let declarations = super::macro_arguments(ctx, form)?;
    ncl_object::with_roots(ctx, &declarations, |ctx, roots| {
        let mut held = roots
            .iter()
            .map(|root| HeapRootedWord::new(ctx, **root))
            .collect::<Result<Vec<_>>>()?;
        let eval_when = hold_symbol(ctx, runtime, &mut held, "EVAL-WHEN")?;
        let compile_toplevel = hold_symbol(ctx, runtime, &mut held, "KEYWORD::COMPILE-TOPLEVEL")?;
        let load_toplevel = hold_symbol(ctx, runtime, &mut held, "KEYWORD::LOAD-TOPLEVEL")?;
        let execute = hold_symbol(ctx, runtime, &mut held, "KEYWORD::EXECUTE")?;
        let situations = hold_list(
            ctx,
            runtime,
            &mut held,
            &[compile_toplevel, load_toplevel, execute],
        )?;
        let proclaim = hold_symbol(ctx, runtime, &mut held, "PROCLAIM")?;
        let quote = hold_symbol(ctx, runtime, &mut held, "QUOTE")?;
        let mut body = Vec::with_capacity(roots.len());
        for index in 0..roots.len() {
            let quoted = hold_list(ctx, runtime, &mut held, &[quote, index])?;
            body.push(hold_list(ctx, runtime, &mut held, &[proclaim, quoted])?);
        }
        let mut expansion = Vec::with_capacity(body.len() + 2);
        expansion.push(eval_when);
        expansion.push(situations);
        expansion.extend(body);
        let values = expansion
            .iter()
            .map(|index| {
                held.get(*index)
                    .map(HeapRootedWord::get)
                    .ok_or(ObjectError::TypeError)
            })
            .collect::<Result<Vec<_>>>()?;
        super::list(ctx, runtime, &values)
    })
}
