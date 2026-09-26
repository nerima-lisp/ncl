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

pub fn register(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ncl_object::ObjectError> {
    let mut package = runtime.ensure_package(ctx, "COMMON-LISP")?;
    with_root(ctx, &mut package, |ctx, package| {
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
                        Word::NIL,
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
