use ncl_object::{
    Package, Runtime, Stream, ThreadContext, Word, make_simple_vector, make_stream,
    set_symbol_special, set_symbol_value, symbol_value, with_root,
};

use super::character::StreamKind;

const STANDARD_STREAMS: [(&str, StreamKind, &str); 7] = [
    ("*STANDARD-OUTPUT*", StreamKind::StandardOutput, "OUTPUT"),
    ("*STANDARD-INPUT*", StreamKind::StandardInput, "INPUT"),
    ("*ERROR-OUTPUT*", StreamKind::StandardError, "OUTPUT"),
    ("*TERMINAL-IO*", StreamKind::StandardTwoWay, "IO"),
    ("*QUERY-IO*", StreamKind::StandardTwoWay, "IO"),
    ("*DEBUG-IO*", StreamKind::StandardTwoWay, "IO"),
    ("*TRACE-OUTPUT*", StreamKind::StandardOutput, "OUTPUT"),
];

fn register_print_control_variables(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ncl_object::ObjectError> {
    let mut common_lisp = runtime.ensure_package(ctx, "COMMON-LISP")?;
    with_root(ctx, &mut common_lisp, |ctx, common_lisp| {
        for (name, value) in [
            ("*PRINT-ARRAY*", Word::TRUE),
            ("*PRINT-BASE*", Word::fixnum(10)),
            ("*PRINT-CIRCLE*", Word::NIL),
            ("*PRINT-ESCAPE*", Word::TRUE),
            ("*PRINT-GENSYM*", Word::TRUE),
            ("*PRINT-LENGTH*", Word::NIL),
            ("*PRINT-LEVEL*", Word::NIL),
            ("*PRINT-LINES*", Word::NIL),
            ("*PRINT-MISER-WIDTH*", Word::NIL),
            ("*PRINT-PRETTY*", Word::NIL),
            ("*PRINT-RADIX*", Word::NIL),
            ("*PRINT-RIGHT-MARGIN*", Word::NIL),
        ] {
            register_print_control_variable(ctx, runtime, *common_lisp, name, value)?;
        }

        let mut keyword = runtime.ensure_package(ctx, "KEYWORD")?;
        with_root(ctx, &mut keyword, |ctx, keyword| {
            let (mut upcase, _) = Package::from_word(*keyword).intern(ctx, runtime, "UPCASE")?;
            with_root(ctx, &mut upcase, |ctx, upcase| {
                register_print_control_variable(ctx, runtime, *common_lisp, "*PRINT-CASE*", *upcase)
            })
        })
    })
}

fn register_print_control_variable(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    package: Word,
    name: &str,
    value: Word,
) -> Result<(), ncl_object::ObjectError> {
    let mut package = package;
    with_root(ctx, &mut package, |ctx, package| {
        let mut value = value;
        with_root(ctx, &mut value, |ctx, value| {
            let (mut symbol, _) = Package::from_word(*package).intern(ctx, runtime, name)?;
            with_root(ctx, &mut symbol, |ctx, symbol| {
                set_symbol_special(ctx, *symbol, true)?;
                set_symbol_value(ctx, *symbol, *value)
            })
        })
    })
}

pub fn register(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ncl_object::ObjectError> {
    let mut package = runtime.ensure_package(ctx, "COMMON-LISP")?;
    with_root(ctx, &mut package, |ctx, package| {
        let (mut element_type, _) =
            Package::from_word(*package).intern(ctx, runtime, "CHARACTER")?;
        with_root(ctx, &mut element_type, |ctx, element_type| {
            for (name, kind, direction_name) in STANDARD_STREAMS {
                let (mut symbol, _) = Package::from_word(*package).intern(ctx, runtime, name)?;
                with_root(ctx, &mut symbol, |ctx, symbol| {
                    let (mut direction, _) =
                        Package::from_word(*package).intern(ctx, runtime, direction_name)?;
                    with_root(ctx, &mut direction, |ctx, direction| {
                        let state = make_simple_vector(
                            ctx,
                            runtime,
                            &[Word::fixnum(kind.code()), Word::fixnum(0)],
                        )?;
                        let stream = make_stream(
                            ctx,
                            runtime,
                            *direction,
                            *element_type,
                            Word::NIL,
                            state,
                            Word::TRUE,
                        )?;
                        set_symbol_special(ctx, *symbol, true)?;
                        set_symbol_value(ctx, *symbol, stream.into())
                    })
                })?;
            }
            Ok(())
        })
    })?;
    register_print_control_variables(runtime, ctx)
}

pub fn lookup(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Stream, ncl_object::ObjectError> {
    let mut package = runtime.ensure_package(ctx, "COMMON-LISP")?;
    with_root(ctx, &mut package, |ctx, package| {
        let (mut symbol, _) = Package::from_word(*package).intern(ctx, runtime, name)?;
        with_root(ctx, &mut symbol, |ctx, symbol| {
            Ok(Stream::from_word(symbol_value(ctx, *symbol)?))
        })
    })
}
