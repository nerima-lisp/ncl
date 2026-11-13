use ncl_object::{
    BuiltinFunctionCaller, FindStatus, FunctionArguments, FunctionCaller, FunctionDesignator,
    FunctionObject, MultipleValues, ObjectError, Package, Runtime, ThreadContext, Word,
    make_bignum_from_i128, make_string, pop_root, push_root, string_length, string_ref, with_root,
    with_rooted_slice,
};

use crate::executor::{ExecutionState, FormatOutput, ResolvedParameter};
use crate::model::Directive;
use crate::{FormatError, FormatErrorKind};

pub(super) fn execute(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    let name = directive
        .function_name
        .as_deref()
        .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidDirectiveUse))?;
    let (package_name, symbol_name, external_only) = parse_name(name)
        .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidDirectiveUse))?;
    let package = state
        .runtime
        .find_package(state.ctx, package_name)
        .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidDirectiveUse))?;
    let mut symbol_name = make_string(
        state.ctx,
        state.runtime,
        &symbol_name.chars().collect::<Vec<_>>(),
    )
    .map_err(|_| FormatError::at(directive, FormatErrorKind::InvalidDirectiveUse))?;
    let runtime = state.runtime;
    let found = with_root(state.ctx, &mut symbol_name, |ctx, name| {
        Package::from_word(package).find_symbol(ctx, *name)
    })
    .map_err(|_| FormatError::at(directive, FormatErrorKind::InvalidDirectiveUse))?;
    let Some((mut symbol, status)) = found else {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidDirectiveUse,
        ));
    };
    if external_only && status != FindStatus::External {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidDirectiveUse,
        ));
    }
    let parameters = state.parameters(directive)?;
    let value = state.take_argument(directive)?;
    let mut failure = None;
    let rooted = with_root(state.ctx, &mut symbol, |ctx, symbol| {
        let mut value = value;
        with_root(ctx, &mut value, |ctx, value| {
            match with_parameter_words(ctx, runtime, &parameters, directive, |ctx, words| {
                write_named_result(directive, ctx, runtime, *symbol, *value, words, output)
            }) {
                Ok(()) => Ok(()),
                Err(error) => {
                    failure = Some(error);
                    Err(ObjectError::TypeError)
                }
            }
        })
    });
    if let Some(error) = failure {
        return Err(error);
    }
    rooted.map_err(|_| FormatError::at(directive, FormatErrorKind::InvalidDirectiveUse))
}

fn write_named_result(
    directive: &Directive,
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    symbol: Word,
    value: Word,
    parameters: &[Word],
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    let stream = call_stream_builtin(ctx, runtime, "MAKE-STRING-OUTPUT-STREAM", &[])
        .map_err(|_| FormatError::at(directive, FormatErrorKind::InvalidDirectiveUse))?;
    let mut stream = stream;
    let mut output_failure = None;
    let result = with_root(ctx, &mut stream, |ctx, stream| {
        let designator = FunctionDesignator::try_from_word(ctx, symbol)?;
        let mut arguments = Vec::with_capacity(parameters.len() + 4);
        arguments.extend_from_slice(&[
            *stream,
            value,
            if directive.colon {
                Word::TRUE
            } else {
                Word::NIL
            },
            if directive.at_sign {
                Word::TRUE
            } else {
                Word::NIL
            },
        ]);
        arguments.extend_from_slice(parameters);
        let mut caller = BuiltinFunctionCaller;
        let mut values = MultipleValues::new();
        caller.call_function(
            ctx,
            runtime,
            designator,
            FunctionArguments::new(&arguments),
            &mut values,
        )?;
        let string = call_stream_builtin(ctx, runtime, "GET-OUTPUT-STREAM-STRING", &[*stream])?;
        let length = string_length(ctx, string)?;
        let text = (0..length)
            .map(|index| string_ref(ctx, string, index))
            .collect::<Result<String, _>>()?;
        if let Err(error) = output.write_str(&text) {
            output_failure = Some(error);
            return Err(ObjectError::TypeError);
        }
        Ok(())
    });
    if let Some(error) = output_failure {
        return Err(error);
    }
    result.map_err(|_| FormatError::at(directive, FormatErrorKind::InvalidDirectiveUse))
}

fn parse_name(name: &str) -> Option<(&str, &str, bool)> {
    if let Some((package, symbol)) = name.split_once("::") {
        (!package.is_empty() && !symbol.is_empty()).then_some((package, symbol, false))
    } else if let Some((package, symbol)) = name.split_once(':') {
        (!package.is_empty() && !symbol.is_empty()).then_some((package, symbol, true))
    } else {
        (!name.is_empty()).then_some(("COMMON-LISP-USER", name, false))
    }
}

fn with_parameter_words<T>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    parameters: &[ResolvedParameter],
    directive: &Directive,
    execute: impl FnOnce(&mut ThreadContext, &[Word]) -> Result<T, FormatError>,
) -> Result<T, FormatError> {
    let mut words = Vec::with_capacity(parameters.len());
    let mut roots = Vec::new();
    let build = (|| {
        for parameter in parameters {
            match parameter {
                ResolvedParameter::Integer(value) if is_fixnum(*value) => {
                    words.push(Word::fixnum(*value));
                }
                ResolvedParameter::Integer(value) => {
                    let number = make_bignum_from_i128(ctx, runtime, i128::from(*value))
                        .map_err(|_| FormatError::at(directive, FormatErrorKind::InvalidParameter))?
                        .as_word();
                    let mut number = Box::new(number);
                    let token = push_root(ctx, &mut number);
                    words.push(*number);
                    roots.push((words.len() - 1, number, token));
                }
                ResolvedParameter::Character(character) => {
                    words.push(Word::character(u32::from(*character)));
                }
                ResolvedParameter::Unsupplied => words.push(Word::NIL),
            }
        }
        Ok(())
    })();
    let result = build.and_then(|()| {
        // Each root index is captured immediately after its word is appended.
        for (index, number, _) in &roots {
            // check-added-lines: allow(index) index is captured after push; words is not shortened.
            words[*index] = **number;
        }
        let mut failure = None;
        let rooted = with_rooted_slice(ctx, &words, |ctx, rooted| match execute(ctx, rooted) {
            Ok(value) => Ok(value),
            Err(error) => {
                failure = Some(error);
                Err(ObjectError::TypeError)
            }
        });
        failure.map_or_else(
            || rooted.map_err(|_| FormatError::at(directive, FormatErrorKind::InvalidDirectiveUse)),
            Err,
        )
    });
    let mut root_failure = false;
    for (_, _, token) in roots.into_iter().rev() {
        root_failure |= !pop_root(ctx, token);
    }
    if root_failure {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidDirectiveUse,
        ));
    }
    result
}

fn is_fixnum(value: i64) -> bool {
    (-(1_i64 << 62)..(1_i64 << 62)).contains(&value)
}

fn call_stream_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .ok_or(ObjectError::UndefinedFunction)
        .and_then(FunctionObject::try_from)?;
    runtime.call_builtin(ctx, function, arguments)
}
