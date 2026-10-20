#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]

//! Seventh-wave coverage for dispatch tables, sink failures, numbers, and circles.

use ncl_object::{
    ArrayElementType, ArrayOptions, ObjectError, Runtime, ThreadContext, Word, make_array,
    make_bignum_from_i128, make_cons, make_simple_vector, make_specialized_array,
};
use ncl_printer::{
    CharSink, PrintError, PrintOptions, StringSink, copy_pprint_dispatch, pprint_dispatch, write,
};

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();
    (runtime, ctx)
}

fn render(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    object: Word,
    options: PrintOptions,
) -> String {
    let mut sink = StringSink::new();
    write(ctx, runtime, object, &mut sink, &options).unwrap();
    sink.into_string()
}

#[derive(Default)]
struct RejectingSink;

impl CharSink for RejectingSink {
    fn write_char(&mut self, _character: char) -> Result<(), PrintError> {
        Err(PrintError::Sink("rejected".to_string()))
    }
}

#[test]
fn dispatch_tables_handle_empty_and_malformed_entries() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(1), Word::NIL).unwrap(),
        Word::NIL
    );
    assert_eq!(
        copy_pprint_dispatch(&mut ctx, &runtime, Word::NIL).unwrap(),
        Word::NIL
    );
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(1), Word::fixnum(2)),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn rejecting_sink_propagates_a_sink_print_error() {
    let (runtime, mut ctx) = context();
    let mut sink = RejectingSink;
    assert_eq!(
        write(
            &mut ctx,
            &runtime,
            Word::fixnum(7),
            &mut sink,
            &PrintOptions::new(),
        ),
        Err(PrintError::Sink("rejected".to_string()))
    );
}

#[test]
fn radix_numbers_cover_decimal_suffix_bignum_prefix_and_positive_infinity() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            Word::fixnum(42),
            PrintOptions::new().with_radix(true),
        ),
        "42."
    );
    let bignum = make_bignum_from_i128(&mut ctx, &runtime, 42).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            bignum.as_word(),
            PrintOptions::new().with_base(16).with_radix(true),
        ),
        "#x2A"
    );
    let infinity = ncl_object::make_double(&mut ctx, &runtime, f64::INFINITY).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, infinity.as_word(), PrintOptions::new()),
        "#<DOUBLE-FLOAT Infinity>"
    );
}

#[test]
fn array_zero_limit_and_shared_nested_vector_are_printed_meaningfully() {
    let (runtime, mut ctx) = context();
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            vector,
            PrintOptions::new().with_length(Some(0)),
        ),
        "#(...)"
    );

    let nested = make_array(
        &mut ctx,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: vector,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            nested,
            PrintOptions::new().with_circle(true),
        ),
        "#(#1=#(1) #1#)"
    );
}

#[test]
fn specialized_array_length_and_zero_vector_limit_are_stable() {
    let (runtime, mut ctx) = context();
    let array = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Fixnum,
        &[Word::fixnum(3), Word::fixnum(4)],
    )
    .unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            array,
            PrintOptions::new().with_vector_length(Some(0)),
        ),
        "#(...)"
    );
    assert_eq!(
        render(&runtime, &mut ctx, array, PrintOptions::new()),
        "#(3 4)"
    );
}

#[test]
fn circle_scan_labels_a_cycle_reached_through_a_cons_tail() {
    let (runtime, mut ctx) = context();
    let tail = make_cons(&mut ctx, &runtime, Word::fixnum(2), Word::NIL).unwrap();
    let root = make_cons(&mut ctx, &runtime, Word::fixnum(1), tail).unwrap();
    ncl_object::rplacd(&mut ctx, tail, root).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            root,
            PrintOptions::new().with_circle(true),
        ),
        "#1=(1 2 . #1#)"
    );
}
