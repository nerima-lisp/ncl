use super::{Result, elements, held_form, held_forms, held_fresh_symbols, held_list, symbol};
use ncl_object::{ObjectError, Runtime, ThreadContext, Word, symbol_name};

fn held_named_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    name: &str,
) -> Result<usize> {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let value = symbol(ctx, runtime, name)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    held.push(value);
    Ok(held.len() - 1)
}

#[derive(Clone, Copy)]
enum SymbolIterationMode {
    All,
    External,
    Symbols,
}

impl SymbolIterationMode {
    const fn keyword(self) -> &'static str {
        match self {
            Self::All => "KEYWORD::ALL",
            Self::External => "KEYWORD::EXTERNAL",
            Self::Symbols => "KEYWORD::SYMBOLS",
        }
    }
}

fn symbols_loop(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
    helper: &str,
    mode: SymbolIterationMode,
    all_packages: bool,
) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let spec = elements(ctx, **roots.first().ok_or(ObjectError::TypeError)?)?;
        let spec_max = if all_packages { 2 } else { 3 };
        if !(1..=spec_max).contains(&spec.len()) {
            return Err(ObjectError::TypeError);
        }

        let mut held = roots.iter().map(|root| **root).collect::<Vec<_>>();
        let variable = held.len();
        held.push(spec[0]); // check-added-lines: allow(index) spec length was validated above.
        symbol_name(ctx, held[variable])?; // check-added-lines: allow(index) variable was just appended.

        let package = if all_packages {
            held.len()
        } else if let Some(package) = spec.get(1) {
            let index = held.len();
            held.push(*package);
            index
        } else {
            held_named_symbol(ctx, runtime, &mut held, "COMMON-LISP::*PACKAGE*")?
        };
        if all_packages {
            held.push(Word::NIL);
        }

        let result = held.len();
        held.push(if all_packages {
            spec.get(1).copied().unwrap_or(Word::NIL)
        } else {
            spec.get(2).copied().unwrap_or(Word::NIL)
        });
        let kind = held_named_symbol(ctx, runtime, &mut held, mode.keyword())?;
        let symbols = held_form(ctx, runtime, &mut held, helper, &[kind, package])?;
        let fresh = held_fresh_symbols(ctx, runtime, &mut held, 3)?;
        // check-added-lines: allow(index) the helper returns the requested three symbols.
        let (cursor, loop_tag, end_tag) = (fresh[0], fresh[1], fresh[2]);
        let cursor_arg = [cursor];
        let end_tag_arg = [end_tag];
        let forms = held_forms(
            ctx,
            runtime,
            &mut held,
            &[("ENDP", &cursor_arg), ("GO", &end_tag_arg)],
        )?;
        // check-added-lines: allow(index) held_forms returns one result per requested form.
        let (endp, go_end) = (forms[0], forms[1]);
        let test_args: [usize; 2] = (endp, go_end).into();
        let test = held_form(ctx, runtime, &mut held, "IF", &test_args)?;
        let forms = held_forms(
            ctx,
            runtime,
            &mut held,
            &[("CAR", &cursor_arg), ("CDR", &cursor_arg)],
        )?;
        // check-added-lines: allow(index) held_forms returns one result per requested form.
        let (car, cdr) = (forms[0], forms[1]);
        let set_variable_args = [variable, car];
        let advance_args = [cursor, cdr];
        let jump_args = [loop_tag];
        let forms = held_forms(
            ctx,
            runtime,
            &mut held,
            &[
                ("SETQ", &set_variable_args),
                ("SETQ", &advance_args),
                ("GO", &jump_args),
            ],
        )?;
        // check-added-lines: allow(index) held_forms returns one result per requested form.
        let (set_variable, advance, jump) = (forms[0], forms[1], forms[2]);

        let declare = held_named_symbol(ctx, runtime, &mut held, "DECLARE")?;
        let mut body_start = 1;
        let mut declarations = Vec::new();
        while let Some(body_form) = roots.get(body_start).map(|root| **root) {
            let declaration = held[declare]; // check-added-lines: allow(index) declare was just appended.
            if !body_form.is_cons() || ncl_object::car(ctx, body_form)? != declaration {
                break;
            }
            declarations.push(body_start);
            body_start += 1;
        }

        let mut tagbody = vec![loop_tag, test, set_variable];
        tagbody.extend(body_start..roots.len());
        tagbody.extend([advance, jump, end_tag]);
        let tagbody = held_form(ctx, runtime, &mut held, "TAGBODY", &tagbody)?;
        let nil = held.len();
        held.push(Word::NIL);
        let clear = held_form(ctx, runtime, &mut held, "SETQ", &[variable, nil])?;
        let variable_binding = held_list(ctx, runtime, &mut held, &[variable, nil])?;
        let cursor_binding = held_list(ctx, runtime, &mut held, &[cursor, symbols])?;
        let bindings = held_list(ctx, runtime, &mut held, &[variable_binding, cursor_binding])?;
        let mut body = vec![bindings];
        body.extend(declarations);
        body.extend([tagbody, clear, result]);
        let body = held_form(ctx, runtime, &mut held, "LET", &body)?;
        let block_name = held.len();
        held.push(Word::NIL);
        let block = held_form(ctx, runtime, &mut held, "BLOCK", &[block_name, body])?;
        Ok(held[block]) // check-added-lines: allow(index) held_form returns the appended final slot.
    })
}

pub(super) fn expand_do_symbols(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    symbols_loop(
        ctx,
        runtime,
        values,
        "NCL-EXT::%PACKAGE-ITERATION-SYMBOLS",
        SymbolIterationMode::Symbols,
        false,
    )
}

pub(super) fn expand_do_external_symbols(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    symbols_loop(
        ctx,
        runtime,
        values,
        "NCL-EXT::%PACKAGE-ITERATION-SYMBOLS",
        SymbolIterationMode::External,
        false,
    )
}

pub(super) fn expand_do_all_symbols(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    symbols_loop(
        ctx,
        runtime,
        values,
        "NCL-EXT::%PACKAGE-ITERATION-SYMBOLS",
        SymbolIterationMode::All,
        true,
    )
}
