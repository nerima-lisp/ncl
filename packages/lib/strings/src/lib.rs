//! Characters, strings, and the NCL-UNICODE data boundary.

mod unicode_data;

mod builtins {
    use crate::unicode_data;
    use ncl_object::{
        Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
        BuiltinName, BuiltinPackage, Character, LambdaList, LispError, MultipleValues, ObjectError,
        Package, Parameter, ParameterType, Runtime, ThreadContext, Word, make_simple_vector,
        make_string, set_symbol_constant, set_symbol_value, simple_vector_length,
        simple_vector_ref, string_length, string_ref, string_set,
    };

    include!("builtins/character.rs");
    include!("builtins/unicode.rs");
    include!("builtins/string_helpers.rs");
    include!("builtins/string_ops.rs");
    include!("builtins/string_compare.rs");
    include!("builtins/register.rs");
}

pub use builtins::{general_category, register};

#[cfg(test)]
#[path = "tests/units.rs"]
mod tests;
