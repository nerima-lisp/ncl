use ncl_object::{
    BuiltinFunctionCaller, FunctionArguments, FunctionCaller, FunctionDesignator, FunctionObject,
    MultipleValues, ObjectError, ObjectRef, StringObject, Word, classify_object, with_root,
    with_rooted_slice,
};

use crate::executor::{ExecutionState, Flow, FormatError, FormatErrorKind, execute_control};
use crate::model::Directive;

use super::{
    FlowResult, Output, State, invalid_use, list_elements, object_error, string_value,
    with_rooted_words,
};

pub(super) fn execute(
    directive: &Directive,
    state: &mut State<'_>,
    output: &mut Output<'_>,
) -> FlowResult {
    if directive.colon || !directive.parameters.is_empty() {
        return Err(invalid_use(directive));
    }
    let control_word = state.take_argument(directive)?;
    let control_type = classify_object(state.ctx, control_word);
    if let ObjectRef::String(word) = control_type {
        execute_string_control(directive, state, output, StringObject::from_word(word))
    } else if !directive.at_sign
        && matches!(
            control_type,
            ObjectRef::Function(_) | ObjectRef::Closure(_) | ObjectRef::Symbol(_)
        )
    {
        execute_function_control(directive, state, output, control_word)
    } else {
        Err(FormatError::at(
            directive,
            FormatErrorKind::WrongArgumentType,
        ))
    }
}

fn execute_string_control(
    directive: &Directive,
    state: &mut State<'_>,
    output: &mut Output<'_>,
    control_word: StringObject,
) -> FlowResult {
    let control =
        string_value(state.ctx, control_word).map_err(|error| object_error(directive, error))?;
    let parsed = crate::parser::parse(&control).map_err(|error| {
        FormatError::new(
            error.offset,
            error.directive,
            FormatErrorKind::Parse(error.kind),
        )
    })?;
    let flow = if directive.at_sign {
        execute_control(state, &parsed, output)?
    } else {
        let list = state.take_argument(directive)?;
        let arguments = list_elements(state.ctx, list, directive)?;
        let previous_is_one = state.previous_is_one;
        let runtime = state.runtime;
        let frames = state.frames.clone();
        let (flow, previous_is_one) =
            with_rooted_words(state.ctx, &arguments, directive, |ctx, rooted| {
                let mut nested = ExecutionState {
                    arguments: rooted,
                    index: 0,
                    previous_is_one,
                    frames,
                    ctx,
                    runtime,
                };
                let flow = execute_control(&mut nested, &parsed, output)?;
                Ok((flow, nested.previous_is_one))
            })?;
        state.previous_is_one = previous_is_one;
        flow
    };
    Ok(if flow == Flow::Escape {
        Flow::Continue
    } else {
        flow
    })
}

fn execute_function_control(
    directive: &Directive,
    state: &mut State<'_>,
    output: &mut Output<'_>,
    control_word: Word,
) -> FlowResult {
    let list = state.take_argument(directive)?;
    let arguments = list_elements(state.ctx, list, directive)?;
    let text = call_formatter(
        directive,
        state.ctx,
        state.runtime,
        control_word,
        &arguments,
    )?;
    output.write_str(&text)?;
    Ok(Flow::Continue)
}

fn call_formatter(
    directive: &Directive,
    ctx: &mut ncl_object::ThreadContext,
    runtime: &ncl_object::Runtime,
    control_word: Word,
    arguments: &[Word],
) -> Result<String, FormatError> {
    let mut control_word = control_word;
    with_root(ctx, &mut control_word, |ctx, control| {
        with_rooted_slice(ctx, arguments, |ctx, rooted_arguments| {
            let mut stream = call_stream_builtin(ctx, runtime, "MAKE-STRING-OUTPUT-STREAM", &[])?;
            with_root(ctx, &mut stream, |ctx, stream| {
                let mut call_arguments = Vec::with_capacity(rooted_arguments.len() + 1);
                call_arguments.push(*stream);
                call_arguments.extend_from_slice(rooted_arguments);
                with_rooted_slice(ctx, &call_arguments, |ctx, rooted_call_arguments| {
                    let designator = FunctionDesignator::try_from_word(ctx, *control)?;
                    let mut caller = BuiltinFunctionCaller;
                    let mut values = MultipleValues::new();
                    caller.call_function(
                        ctx,
                        runtime,
                        designator,
                        FunctionArguments::new(rooted_call_arguments),
                        &mut values,
                    )
                })?;
                let text =
                    call_stream_builtin(ctx, runtime, "GET-OUTPUT-STREAM-STRING", &[*stream])?;
                string_value(ctx, StringObject::from_word(text))
            })
        })
    })
    .map_err(|error| object_error(directive, error))
}

fn call_stream_builtin(
    ctx: &mut ncl_object::ThreadContext,
    runtime: &ncl_object::Runtime,
    name: &str,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .ok_or(ObjectError::UndefinedFunction)
        .and_then(FunctionObject::try_from)?;
    runtime.call_builtin(ctx, function, arguments)
}
