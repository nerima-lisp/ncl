//! Typed ANSI file-stream builtins.

#![forbid(unsafe_code)]

/// Builtin adapter implementations.
pub mod adapters;
/// Character stream operations.
pub mod character;
/// Typed failures from stream operations.
pub mod error;
/// File stream operations.
pub mod file;
/// Builtin registration.
pub mod registration;
pub(crate) mod standard;

pub use error::StreamError;
/// Typed handle to a stream model object.
pub use ncl_streams::Stream;

const POSITION: usize = 1;
const DATA: usize = 2;
const STRING_INPUT: i64 = -1;
const STRING_OUTPUT: i64 = -2;
const CLOSED: i64 = -3;
const FILE_OUTPUT: i64 = -4;
const STREAM_PARAMETER: ncl_object::Parameter = ncl_object::Parameter {
    name: ncl_object::BuiltinName::new("stream"),
    ty: ncl_object::ParameterType::Any,
};
const CHARACTER_PARAMETER: ncl_object::Parameter = ncl_object::Parameter {
    name: ncl_object::BuiltinName::new("character"),
    ty: ncl_object::ParameterType::Any,
};
const STRING_PARAMETER: ncl_object::Parameter = ncl_object::Parameter {
    name: ncl_object::BuiltinName::new("string"),
    ty: ncl_object::ParameterType::Any,
};
const ARGUMENTS_PARAMETER: ncl_object::Parameter = ncl_object::Parameter {
    name: ncl_object::BuiltinName::new("arguments"),
    ty: ncl_object::ParameterType::Any,
};
const CHARACTER_REQUIRED: &[ncl_object::Parameter] = &[CHARACTER_PARAMETER];
const CHARACTER_AND_STREAM: &[ncl_object::Parameter] = &[CHARACTER_PARAMETER, STREAM_PARAMETER];
const OPEN_PARAMETERS: &[ncl_object::Parameter] = &[ncl_object::Parameter {
    name: ncl_object::BuiltinName::new("namestring"),
    ty: ncl_object::ParameterType::Any,
}];
const STREAM_PARAMETERS: &[ncl_object::Parameter] = &[STREAM_PARAMETER];
const STRING_PARAMETERS: &[ncl_object::Parameter] = &[
    ncl_object::Parameter {
        name: ncl_object::BuiltinName::new("stream"),
        ty: ncl_object::ParameterType::Any,
    },
    ncl_object::Parameter {
        name: ncl_object::BuiltinName::new("string"),
        ty: ncl_object::ParameterType::Any,
    },
];

pub use registration::{register, register_with_context};
