use ncl_object::{
    Arity, Builtin, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, ObjectError, Parameter, ParameterType, Runtime, ThreadContext,
};

use super::adapters::{
    close_adapter, file_length_adapter, file_position_adapter, file_string_length_adapter,
    fresh_line_adapter, get_output_stream_string_adapter, input_stream_p_adapter,
    interactive_stream_p_adapter, make_string_input_adapter, make_string_output_adapter,
    open_adapter, open_stream_p_adapter, output_stream_p_adapter, pass_arguments,
    peek_char_adapter, read_byte_adapter, read_char_adapter, read_line_adapter,
    stream_element_type_adapter, stream_external_format_adapter, streamp_adapter, terpri_adapter,
    unread_char_adapter, write_char_adapter, write_line_adapter, write_string_adapter,
};
use super::{
    ARGUMENTS_PARAMETER, CHARACTER_AND_STREAM, CHARACTER_REQUIRED, OPEN_PARAMETERS,
    STREAM_PARAMETERS, STRING_PARAMETER, STRING_PARAMETERS,
};

/// Register the implemented file-stream functions.
///
/// # Errors
///
/// Returns an error when a builtin cannot be registered.
#[allow(clippy::too_many_lines)]
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    let open = BuiltinImplementation::adapted(
        Builtin {
            lambda_list: LambdaList::with_rest(
                OPEN_PARAMETERS,
                Parameter {
                    name: BuiltinName::new("options"),
                    ty: ParameterType::Any,
                },
            ),
            convention: BuiltinConvention::Adapted,
        },
        open_adapter,
        pass_arguments,
    );
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("OPEN")),
        open,
    )?;
    let position = BuiltinImplementation::adapted(
        Builtin {
            lambda_list: LambdaList::with_optional(STREAM_PARAMETERS, &[]),
            convention: BuiltinConvention::Adapted,
        },
        file_position_adapter,
        pass_arguments,
    );
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("FILE-POSITION"),
        ),
        position,
    )?;
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("FILE-LENGTH")),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::fixed(STREAM_PARAMETERS),
                convention: BuiltinConvention::Direct(Arity::exact(1)),
            },
            file_length_adapter,
        ),
    )?;
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("CLOSE")),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::with_rest(
                    STREAM_PARAMETERS,
                    Parameter {
                        name: BuiltinName::new("options"),
                        ty: ParameterType::Any,
                    },
                ),
                convention: BuiltinConvention::Adapted,
            },
            close_adapter,
            pass_arguments,
        ),
    )?;
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("FILE-STRING-LENGTH"),
        ),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::fixed(STRING_PARAMETERS),
                convention: BuiltinConvention::Direct(Arity::exact(2)),
            },
            file_string_length_adapter,
        ),
    )?;
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("READ-BYTE")),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::with_rest(
                    STREAM_PARAMETERS,
                    Parameter {
                        name: BuiltinName::new("options"),
                        ty: ParameterType::Any,
                    },
                ),
                convention: BuiltinConvention::Adapted,
            },
            read_byte_adapter,
            pass_arguments,
        ),
    )?;
    for (name, function) in [
        ("STREAMP", streamp_adapter as _),
        ("INPUT-STREAM-P", input_stream_p_adapter as _),
        ("OUTPUT-STREAM-P", output_stream_p_adapter as _),
        ("OPEN-STREAM-P", open_stream_p_adapter as _),
        ("INTERACTIVE-STREAM-P", interactive_stream_p_adapter as _),
        ("STREAM-ELEMENT-TYPE", stream_element_type_adapter as _),
        (
            "STREAM-EXTERNAL-FORMAT",
            stream_external_format_adapter as _,
        ),
    ] {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::fixed(STREAM_PARAMETERS),
                    convention: BuiltinConvention::Direct(Arity::exact(1)),
                },
                function,
            ),
        )?;
    }
    register_character_builtins(runtime, &mut ctx)?;
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn register_character_builtins(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    let registrations: &[(&str, Builtin, ncl_object::RustBuiltin)] = &[
        (
            "READ-CHAR",
            Builtin {
                lambda_list: LambdaList::with_rest(
                    &[],
                    Parameter {
                        name: BuiltinName::new("arguments"),
                        ty: ParameterType::Any,
                    },
                ),
                convention: BuiltinConvention::Adapted,
            },
            read_char_adapter,
        ),
        (
            "READ-CHAR-NO-HANG",
            Builtin {
                lambda_list: LambdaList::with_rest(
                    &[],
                    Parameter {
                        name: BuiltinName::new("arguments"),
                        ty: ParameterType::Any,
                    },
                ),
                convention: BuiltinConvention::Adapted,
            },
            read_char_adapter,
        ),
        (
            "UNREAD-CHAR",
            Builtin {
                lambda_list: LambdaList::fixed(CHARACTER_AND_STREAM),
                convention: BuiltinConvention::Direct(Arity::exact(2)),
            },
            unread_char_adapter,
        ),
        (
            "PEEK-CHAR",
            Builtin {
                lambda_list: LambdaList::with_rest(
                    &[],
                    Parameter {
                        name: BuiltinName::new("arguments"),
                        ty: ParameterType::Any,
                    },
                ),
                convention: BuiltinConvention::Adapted,
            },
            peek_char_adapter,
        ),
        (
            "READ-LINE",
            Builtin {
                lambda_list: LambdaList::with_rest(
                    &[],
                    Parameter {
                        name: BuiltinName::new("arguments"),
                        ty: ParameterType::Any,
                    },
                ),
                convention: BuiltinConvention::Adapted,
            },
            read_line_adapter,
        ),
        (
            "WRITE-CHAR",
            Builtin {
                lambda_list: LambdaList::with_optional(CHARACTER_REQUIRED, STREAM_PARAMETERS),
                convention: BuiltinConvention::Adapted,
            },
            write_char_adapter,
        ),
        (
            "WRITE-STRING",
            Builtin {
                lambda_list: LambdaList::with_rest(&[STRING_PARAMETER], ARGUMENTS_PARAMETER),
                convention: BuiltinConvention::Adapted,
            },
            write_string_adapter,
        ),
        (
            "WRITE-LINE",
            Builtin {
                lambda_list: LambdaList::with_rest(&[STRING_PARAMETER], ARGUMENTS_PARAMETER),
                convention: BuiltinConvention::Adapted,
            },
            write_line_adapter,
        ),
        (
            "TERPRI",
            Builtin {
                lambda_list: LambdaList::with_optional(&[], STREAM_PARAMETERS),
                convention: BuiltinConvention::Adapted,
            },
            terpri_adapter,
        ),
        (
            "FRESH-LINE",
            Builtin {
                lambda_list: LambdaList::with_optional(&[], STREAM_PARAMETERS),
                convention: BuiltinConvention::Adapted,
            },
            fresh_line_adapter,
        ),
        (
            "MAKE-STRING-INPUT-STREAM",
            Builtin {
                lambda_list: LambdaList::with_rest(&[STRING_PARAMETER], ARGUMENTS_PARAMETER),
                convention: BuiltinConvention::Adapted,
            },
            make_string_input_adapter,
        ),
        (
            "MAKE-STRING-OUTPUT-STREAM",
            Builtin {
                lambda_list: LambdaList::fixed(&[]),
                convention: BuiltinConvention::Direct(Arity::exact(0)),
            },
            make_string_output_adapter,
        ),
        (
            "GET-OUTPUT-STREAM-STRING",
            Builtin {
                lambda_list: LambdaList::fixed(STREAM_PARAMETERS),
                convention: BuiltinConvention::Direct(Arity::exact(1)),
            },
            get_output_stream_string_adapter,
        ),
    ];
    for (name, descriptor, function) in registrations {
        runtime.register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::adapted(*descriptor, *function, pass_arguments),
        )?;
    }
    Ok(())
}
