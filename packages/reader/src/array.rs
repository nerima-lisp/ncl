//! Reading `#nA` array literals.

use std::cell::Cell;

use ncl_object::{
    ArrayElementType, ArrayOptions, Runtime, ThreadContext, Word, array_row_major_set, car, cdr,
    make_array, with_roots,
};

use crate::error::ReadError;
use crate::input::CharSource;
use crate::reader::{ReadOptions, read_form};

pub fn read_array(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut dyn CharSource,
    opts: &ReadOptions,
    rt: &Cell<Word>,
    labels: &Cell<Word>,
    rank: usize,
) -> Result<Word, ReadError> {
    let form =
        read_form(ctx, runtime, source, opts, rt, labels)?.ok_or(ReadError::UnexpectedEof)?;
    let mut dimensions = Vec::with_capacity(rank);
    let mut elements = Vec::new();
    read_array_contents(ctx, form, rank, 0, &mut dimensions, &mut elements)?;
    if dimensions.len() != rank {
        return Err(ReadError::ArraySyntax);
    }
    with_roots(ctx, &elements, |ctx, rooted_elements| {
        let array = make_array(
            ctx,
            runtime,
            &dimensions,
            ArrayOptions {
                element_type: ArrayElementType::T,
                initial_element: Word::NIL,
                adjustable: false,
                fill_pointer: None,
                displaced_to: None,
                displaced_index_offset: 0,
            },
        )?;
        for (index, element) in rooted_elements.iter().copied().enumerate() {
            array_row_major_set(ctx, array, index, *element)?;
        }
        Ok(array)
    })
    .map_err(ReadError::from)
}

fn read_array_contents(
    ctx: &ThreadContext,
    form: Word,
    rank: usize,
    depth: usize,
    dimensions: &mut Vec<usize>,
    elements: &mut Vec<Word>,
) -> Result<(), ReadError> {
    if depth == rank {
        elements.push(form);
        return Ok(());
    }
    let mut values = Vec::new();
    let mut cursor = form;
    while cursor != Word::NIL {
        if !cursor.is_cons() {
            return Err(ReadError::ArraySyntax);
        }
        values.push(car(ctx, cursor).map_err(ReadError::from)?);
        cursor = cdr(ctx, cursor).map_err(ReadError::from)?;
    }
    if let Some(expected) = dimensions.get(depth) {
        if *expected != values.len() {
            return Err(ReadError::ArraySyntax);
        }
    } else {
        dimensions.push(values.len());
    }
    for value in values {
        read_array_contents(ctx, value, rank, depth + 1, dimensions, elements)?;
    }
    Ok(())
}
