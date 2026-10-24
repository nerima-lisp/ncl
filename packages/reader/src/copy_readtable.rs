use ncl_object::{
    ObjectError, Readtable as ObjectReadtable, Runtime, ThreadContext, Word, make_readtable,
    make_simple_vector, readtable_case as object_readtable_case, readtable_dispatch,
    readtable_offset, readtable_syntax, simple_vector_ref, with_roots,
};

use crate::{ReadError, Readtable};

const TABLE_SIZE: usize = 256;

/// Return an independent copy of `readtable`.
///
/// # Errors
/// Returns an object-layer failure when allocation fails.
pub fn copy_readtable(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    readtable: Readtable,
) -> Result<Readtable, ReadError> {
    copy_readtable_to(ctx, runtime, readtable, None).map_err(ReadError::from)
}

/// Copy `source` into `destination`, or return a new readtable if none is given.
///
/// # Errors
/// Returns an object-layer failure when allocation or slot mutation fails.
pub fn copy_readtable_to(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: Readtable,
    destination: Option<Readtable>,
) -> Result<Readtable, ObjectError> {
    let destination_word = destination.map_or(Word::NIL, |table| table.object().as_word());
    let has_destination = destination.is_some();
    with_roots(
        ctx,
        &[source.object().as_word(), destination_word],
        |ctx, tables| {
            let source_word = tables.first().copied().ok_or(ObjectError::Layout)?.get();
            let source = ObjectReadtable::from_word(source_word);
            let case = object_readtable_case(ctx, source)?;
            with_roots(ctx, &[case], |ctx, cases| {
                let source_word = tables.first().copied().ok_or(ObjectError::Layout)?.get();
                let source = ObjectReadtable::from_word(source_word);
                let syntax = readtable_syntax(ctx, source)?;
                let syntax_copy = copy_vector(ctx, runtime, syntax)?;
                with_roots(ctx, &[syntax_copy], |ctx, syntax_copies| {
                    let source_word = tables.first().copied().ok_or(ObjectError::Layout)?.get();
                    let source = ObjectReadtable::from_word(source_word);
                    let dispatch = readtable_dispatch(ctx, source)?;
                    let dispatch_copy = copy_vector(ctx, runtime, dispatch)?;
                    with_roots(
                        ctx,
                        &[
                            syntax_copies
                                .first()
                                .copied()
                                .ok_or(ObjectError::Layout)?
                                .get(),
                            dispatch_copy,
                            cases.first().copied().ok_or(ObjectError::Layout)?.get(),
                        ],
                        |ctx, copies| {
                            let syntax = copies.first().copied().ok_or(ObjectError::Layout)?.get();
                            let dispatch = copies.get(1).copied().ok_or(ObjectError::Layout)?.get();
                            let case = copies.get(2).copied().ok_or(ObjectError::Layout)?.get();
                            if has_destination {
                                let destination =
                                    tables.get(1).copied().ok_or(ObjectError::Layout)?.get();
                                ctx.write_object_slot(
                                    destination,
                                    readtable_offset::SYNTAX,
                                    syntax,
                                )?;
                                ctx.write_object_slot(
                                    destination,
                                    readtable_offset::DISPATCH,
                                    dispatch,
                                )?;
                                ctx.write_object_slot(destination, readtable_offset::CASE, case)?;
                                Ok(Readtable::from_object(ObjectReadtable::from_word(
                                    destination,
                                )))
                            } else {
                                make_readtable(ctx, runtime, syntax, dispatch, case)
                                    .map(Readtable::from_object)
                            }
                        },
                    )
                })
            })
        },
    )
}

fn copy_vector(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: Word,
) -> Result<Word, ObjectError> {
    let mut entries = [Word::NIL; TABLE_SIZE];
    for (index, slot) in entries.iter_mut().enumerate() {
        *slot = simple_vector_ref(ctx, source, index)?;
    }
    make_simple_vector(ctx, runtime, &entries)
}

#[cfg(test)]
#[allow(
    clippy::panic,
    clippy::unwrap_used,
    reason = "test assertions fail on reader errors"
)]
mod tests {
    use ncl_object::{
        ObjectError, Readtable as ObjectReadtable, Runtime, ThreadContext, Word,
        readtable_dispatch, readtable_offset, readtable_syntax, simple_vector_ref,
        simple_vector_set, with_heap_rooted_slice,
    };

    use super::copy_readtable_to;
    use crate::{ReadError, Readtable, ReadtableCase, standard_readtable};

    #[test]
    fn destination_copy_preserves_case_and_table_entries() {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("context registration: {error}"));
        ctx.set_gc_stress(true);
        ctx.set_strict_forwarding(true);

        let result = with_heap_rooted_slice(&mut ctx, &[Word::NIL; 3], |ctx, tables| {
            let source =
                standard_readtable(ctx, &runtime).map_err(|error| read_error_to_object(&error))?;
            tables
                .first()
                .ok_or(ObjectError::Layout)?
                .set(source.object().as_word());
            let destination =
                standard_readtable(ctx, &runtime).map_err(|error| read_error_to_object(&error))?;
            tables
                .get(1)
                .ok_or(ObjectError::Layout)?
                .set(destination.object().as_word());

            let source_word = tables.first().ok_or(ObjectError::Layout)?.get();
            let source = Readtable::from_object(ObjectReadtable::from_word(source_word));
            let source_syntax = source
                .syntax_table(ctx)
                .map_err(|error| read_error_to_object(&error))?;
            let source_dispatch = source
                .dispatch_table(ctx)
                .map_err(|error| read_error_to_object(&error))?;
            simple_vector_set(ctx, source_syntax, usize::from(b'z'), Word::fixnum(1))?;
            simple_vector_set(ctx, source_dispatch, usize::from(b'~'), Word::fixnum(19))?;
            ctx.write_object_slot(source_word, readtable_offset::CASE, Word::fixnum(1))?;

            let destination_word = tables.get(1).ok_or(ObjectError::Layout)?.get();
            let destination = Readtable::from_object(ObjectReadtable::from_word(destination_word));
            let copied = copy_readtable_to(ctx, &runtime, source, Some(destination))?;
            let current_destination = tables.get(1).ok_or(ObjectError::Layout)?.get();
            let returned_destination = copied.object().as_word() == current_destination;

            let source_word = tables.first().ok_or(ObjectError::Layout)?.get();
            let destination_word = tables.get(1).ok_or(ObjectError::Layout)?.get();
            let source = Readtable::from_object(ObjectReadtable::from_word(source_word));
            let destination = Readtable::from_object(ObjectReadtable::from_word(destination_word));
            let source_syntax = readtable_syntax(ctx, source.object())?;
            let source_dispatch = readtable_dispatch(ctx, source.object())?;
            let destination_syntax = readtable_syntax(ctx, destination.object())?;
            let destination_dispatch = readtable_dispatch(ctx, destination.object())?;
            let destination_contents_match = crate::readtable_case(ctx, destination)
                .map_err(|error| read_error_to_object(&error))?
                == ReadtableCase::Downcase
                && simple_vector_ref(ctx, destination_syntax, usize::from(b'z'))?
                    == simple_vector_ref(ctx, source_syntax, usize::from(b'z'))?
                && simple_vector_ref(ctx, destination_dispatch, usize::from(b'~'))?
                    == simple_vector_ref(ctx, source_dispatch, usize::from(b'~'))?;
            let destination_tables_are_copies =
                destination_syntax != source_syntax && destination_dispatch != source_dispatch;

            let source = Readtable::from_object(ObjectReadtable::from_word(source_word));
            let fresh = copy_readtable_to(ctx, &runtime, source, None)?;
            tables
                .get(2)
                .ok_or(ObjectError::Layout)?
                .set(fresh.object().as_word());
            let source_word = tables.first().ok_or(ObjectError::Layout)?.get();
            let fresh_word = tables.get(2).ok_or(ObjectError::Layout)?.get();
            let source = Readtable::from_object(ObjectReadtable::from_word(source_word));
            let fresh = Readtable::from_object(ObjectReadtable::from_word(fresh_word));
            let fresh_syntax = readtable_syntax(ctx, fresh.object())?;
            let fresh_dispatch = readtable_dispatch(ctx, fresh.object())?;
            let fresh_copy_is_independent = fresh_word != source_word
                && fresh_syntax != readtable_syntax(ctx, source.object())?
                && fresh_dispatch != readtable_dispatch(ctx, source.object())?;

            Ok((
                returned_destination,
                destination_contents_match,
                destination_tables_are_copies,
                fresh_copy_is_independent,
            ))
        })
        .unwrap_or_else(|error| panic!("copy readtable: {error}"));

        assert!(result.0);
        assert!(result.1);
        assert!(result.2);
        assert!(result.3);
    }

    const fn read_error_to_object(error: &ReadError) -> ObjectError {
        match error {
            ReadError::Object(error) => *error,
            ReadError::UnexpectedEof
            | ReadError::UnmatchedRightParen
            | ReadError::DotWithoutCdr
            | ReadError::InvalidSymbolToken(_)
            | ReadError::PackageNotFound(_)
            | ReadError::InvalidBase(_)
            | ReadError::InvalidDigit(_)
            | ReadError::InvalidNumber(_)
            | ReadError::NumberOutOfRange
            | ReadError::FloatFormatUnavailable(_)
            | ReadError::ReadEvalDisabled
            | ReadError::ReadEvalUnavailable
            | ReadError::InvalidCharacter
            | ReadError::UnknownCharacterName(_)
            | ReadError::UndefinedDispatchMacro(_)
            | ReadError::UninvocableMacroFunction(_)
            | ReadError::InvalidFeatureExpression
            | ReadError::ArraySyntax
            | ReadError::StructureSyntax
            | ReadError::PathnameSyntax
            | ReadError::NotDispatchMacro(_) => ObjectError::TypeError,
        }
    }
}
