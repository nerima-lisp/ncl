use ncl_object::{
    Arity, Builtin, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, ObjectError, Parameter, ParameterType, Runtime, ThreadContext,
};

use super::adapters::{
    close_adapter, file_length_adapter, file_position_adapter, file_string_length_adapter,
    finish_output_adapter, fresh_line_adapter, get_output_stream_string_adapter,
    input_stream_p_adapter, interactive_stream_p_adapter, make_string_input_adapter,
    make_string_output_adapter, open_adapter, open_stream_p_adapter, output_stream_p_adapter,
    pass_arguments, peek_char_adapter, read_byte_adapter, read_char_adapter, read_line_adapter,
    stream_element_type_adapter, stream_external_format_adapter, streamp_adapter, terpri_adapter,
    unread_char_adapter, write_byte_adapter, write_char_adapter, write_line_adapter,
    write_string_adapter,
};
use super::{
    ARGUMENTS_PARAMETER, CHARACTER_AND_STREAM, CHARACTER_REQUIRED, OPEN_PARAMETERS,
    STREAM_PARAMETERS, STRING_PARAMETER, STRING_PARAMETERS,
};

const BYTE_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("byte"),
    ty: ParameterType::Integer,
};

const PEEK_CHAR_PARAMETERS: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("peek-type"),
        ty: ParameterType::Any,
    },
    Parameter {
        name: BuiltinName::new("input-stream"),
        ty: ParameterType::Any,
    },
    Parameter {
        name: BuiltinName::new("eof-error-p"),
        ty: ParameterType::Any,
    },
    Parameter {
        name: BuiltinName::new("eof-value"),
        ty: ParameterType::Any,
    },
    Parameter {
        name: BuiltinName::new("recursive-p"),
        ty: ParameterType::Any,
    },
];

const OPEN_NAME: &str = "OPEN";
const FILE_POSITION_NAME: &str = "FILE-POSITION";
const FILE_LENGTH_NAME: &str = "FILE-LENGTH";
const CLOSE_NAME: &str = "CLOSE";
const FILE_STRING_LENGTH_NAME: &str = "FILE-STRING-LENGTH";
const READ_BYTE_NAME: &str = "READ-BYTE";
const STREAMP_NAME: &str = "STREAMP";
const INPUT_STREAM_P_NAME: &str = "INPUT-STREAM-P";
const OUTPUT_STREAM_P_NAME: &str = "OUTPUT-STREAM-P";
const OPEN_STREAM_P_NAME: &str = "OPEN-STREAM-P";
const INTERACTIVE_STREAM_P_NAME: &str = "INTERACTIVE-STREAM-P";
const STREAM_ELEMENT_TYPE_NAME: &str = "STREAM-ELEMENT-TYPE";
const STREAM_EXTERNAL_FORMAT_NAME: &str = "STREAM-EXTERNAL-FORMAT";
const READ_CHAR_NAME: &str = "READ-CHAR";
const READ_CHAR_NO_HANG_NAME: &str = "READ-CHAR-NO-HANG";
const UNREAD_CHAR_NAME: &str = "UNREAD-CHAR";
const PEEK_CHAR_NAME: &str = "PEEK-CHAR";
const READ_LINE_NAME: &str = "READ-LINE";
const WRITE_CHAR_NAME: &str = "WRITE-CHAR";
const WRITE_BYTE_NAME: &str = "WRITE-BYTE";
const WRITE_STRING_NAME: &str = "WRITE-STRING";
const WRITE_LINE_NAME: &str = "WRITE-LINE";
const TERPRI_NAME: &str = "TERPRI";
const FRESH_LINE_NAME: &str = "FRESH-LINE";
const FINISH_OUTPUT_NAME: &str = "FINISH-OUTPUT";
const MAKE_STRING_INPUT_STREAM_NAME: &str = "MAKE-STRING-INPUT-STREAM";
const MAKE_STRING_OUTPUT_STREAM_NAME: &str = "MAKE-STRING-OUTPUT-STREAM";
const GET_OUTPUT_STREAM_STRING_NAME: &str = "GET-OUTPUT-STREAM-STRING";

/// Names registered by this module, in registration order.
pub const REGISTERED_BUILTINS: &[&str] = &[
    OPEN_NAME,
    FILE_POSITION_NAME,
    FILE_LENGTH_NAME,
    CLOSE_NAME,
    FILE_STRING_LENGTH_NAME,
    READ_BYTE_NAME,
    STREAMP_NAME,
    INPUT_STREAM_P_NAME,
    OUTPUT_STREAM_P_NAME,
    OPEN_STREAM_P_NAME,
    INTERACTIVE_STREAM_P_NAME,
    STREAM_ELEMENT_TYPE_NAME,
    STREAM_EXTERNAL_FORMAT_NAME,
    READ_CHAR_NAME,
    READ_CHAR_NO_HANG_NAME,
    UNREAD_CHAR_NAME,
    PEEK_CHAR_NAME,
    READ_LINE_NAME,
    WRITE_CHAR_NAME,
    WRITE_BYTE_NAME,
    WRITE_STRING_NAME,
    WRITE_LINE_NAME,
    TERPRI_NAME,
    FRESH_LINE_NAME,
    FINISH_OUTPUT_NAME,
    MAKE_STRING_INPUT_STREAM_NAME,
    MAKE_STRING_OUTPUT_STREAM_NAME,
    GET_OUTPUT_STREAM_STRING_NAME,
];

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
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(OPEN_NAME)),
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
            BuiltinName::new(FILE_POSITION_NAME),
        ),
        position,
    )?;
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new(FILE_LENGTH_NAME),
        ),
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
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(CLOSE_NAME)),
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
            BuiltinName::new(FILE_STRING_LENGTH_NAME),
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
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(READ_BYTE_NAME)),
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
        (STREAMP_NAME, streamp_adapter as _),
        (INPUT_STREAM_P_NAME, input_stream_p_adapter as _),
        (OUTPUT_STREAM_P_NAME, output_stream_p_adapter as _),
        (OPEN_STREAM_P_NAME, open_stream_p_adapter as _),
        (INTERACTIVE_STREAM_P_NAME, interactive_stream_p_adapter as _),
        (STREAM_ELEMENT_TYPE_NAME, stream_element_type_adapter as _),
        (
            STREAM_EXTERNAL_FORMAT_NAME,
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
    super::standard::register(runtime, &mut ctx)?;
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn register_character_builtins(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    let registrations: &[(&str, Builtin, ncl_object::RustBuiltin)] = &[
        (
            READ_CHAR_NAME,
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
            READ_CHAR_NO_HANG_NAME,
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
            UNREAD_CHAR_NAME,
            Builtin {
                lambda_list: LambdaList::fixed(CHARACTER_AND_STREAM),
                convention: BuiltinConvention::Direct(Arity::exact(2)),
            },
            unread_char_adapter,
        ),
        (
            PEEK_CHAR_NAME,
            Builtin {
                lambda_list: LambdaList::with_optional(&[], PEEK_CHAR_PARAMETERS),
                convention: BuiltinConvention::Adapted,
            },
            peek_char_adapter,
        ),
        (
            READ_LINE_NAME,
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
            WRITE_CHAR_NAME,
            Builtin {
                lambda_list: LambdaList::with_optional(CHARACTER_REQUIRED, STREAM_PARAMETERS),
                convention: BuiltinConvention::Adapted,
            },
            write_char_adapter,
        ),
        (
            WRITE_BYTE_NAME,
            Builtin {
                lambda_list: LambdaList::with_optional(&[BYTE_PARAMETER], STREAM_PARAMETERS),
                convention: BuiltinConvention::Adapted,
            },
            write_byte_adapter,
        ),
        (
            WRITE_STRING_NAME,
            Builtin {
                lambda_list: LambdaList::with_rest(&[STRING_PARAMETER], ARGUMENTS_PARAMETER),
                convention: BuiltinConvention::Adapted,
            },
            write_string_adapter,
        ),
        (
            WRITE_LINE_NAME,
            Builtin {
                lambda_list: LambdaList::with_rest(&[STRING_PARAMETER], ARGUMENTS_PARAMETER),
                convention: BuiltinConvention::Adapted,
            },
            write_line_adapter,
        ),
        (
            TERPRI_NAME,
            Builtin {
                lambda_list: LambdaList::with_optional(&[], STREAM_PARAMETERS),
                convention: BuiltinConvention::Adapted,
            },
            terpri_adapter,
        ),
        (
            FRESH_LINE_NAME,
            Builtin {
                lambda_list: LambdaList::with_optional(&[], STREAM_PARAMETERS),
                convention: BuiltinConvention::Adapted,
            },
            fresh_line_adapter,
        ),
        (
            FINISH_OUTPUT_NAME,
            Builtin {
                lambda_list: LambdaList::with_optional(&[], STREAM_PARAMETERS),
                convention: BuiltinConvention::Adapted,
            },
            finish_output_adapter,
        ),
        (
            MAKE_STRING_INPUT_STREAM_NAME,
            Builtin {
                lambda_list: LambdaList::with_rest(&[STRING_PARAMETER], ARGUMENTS_PARAMETER),
                convention: BuiltinConvention::Adapted,
            },
            make_string_input_adapter,
        ),
        (
            MAKE_STRING_OUTPUT_STREAM_NAME,
            Builtin {
                lambda_list: LambdaList::fixed(&[]),
                convention: BuiltinConvention::Direct(Arity::exact(0)),
            },
            make_string_output_adapter,
        ),
        (
            GET_OUTPUT_STREAM_STRING_NAME,
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
