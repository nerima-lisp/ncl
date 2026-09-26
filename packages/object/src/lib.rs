//! Typed, safe values and runtime state for NCL.
#![allow(missing_docs)]
pub use ncl_sys::Word;
pub mod array;
mod builtin;
mod classify;
mod code;
pub mod cons;
mod context;
mod control_extensions;
mod function;
mod function_call;
pub(crate) mod gc;
pub mod hash_table;
mod instance;
mod layout;
mod number;
mod object_access;
mod object_error;
pub mod package;
mod primitives;
mod readtable;
mod registry_extensions;
mod roots;
mod runtime;
mod specialized_array;
mod stream;
mod structure;
mod symbol_extensions;
pub mod typed;
pub use array::{
    ArrayElementType, ArrayOptions, array_dimensions, array_row_major_ref, array_row_major_set,
    make_array, make_simple_vector, make_string, simple_vector_length, simple_vector_ref,
    simple_vector_set, string_length, string_ref, string_set,
};
pub use builtin::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, KeywordAdapter, LambdaList, LispErrorConverter,
    MultipleValues, NclStatus, Parameter, ParameterType, RegisterFn, RustBuiltin,
};
pub use classify::{ObjectRef, classify, classify_object};
pub use code::code_slot;
pub use code::{
    CodeObject, code_constants, code_debug, code_entry, code_size, code_stack_map, make_code_object,
};
pub use cons::{rplaca, rplacd};
pub use context::ThreadContext;
pub use function::{
    Function, closure_ref, function_entry, function_name, make_closure, make_simple_fun,
};
pub use function::{function_code, function_lambda_list};
pub use function_call::{BuiltinFunctionCaller, FunctionArguments, FunctionCaller};
pub use gc::register;
pub use instance::{Instance, instance_class, make_instance, slot_ref, slot_set};
pub use layout::{
    array_offset, code_offset, function_offset, instance_offset, number_offset, readtable_offset,
    simple_vector_offset, specialized_array_offset, stream_offset, string_offset, structure_offset,
    symbol_flag, symbol_offset, widetag,
};
pub use ncl_sys::{ThreadLayout, thread_layout};
pub use number::{
    Bignum, Complex, DoubleFloat, Ratio, bignum_limbs, double_value, make_bignum_from_i128,
    make_complex, make_double, make_ratio,
};
pub use number::{bignum_sign, complex_imag, complex_real, ratio_denominator, ratio_numerator};
pub use object_error::ObjectError;
pub use package::{FindStatus, Package};
pub use primitives::{allocate, car, cdr, make_cons, make_symbol};
pub use readtable::readtable_slot;
pub use readtable::{
    Readtable, make_readtable, readtable_case, readtable_dispatch, readtable_syntax,
};
pub(crate) use roots::{finish_root, with_root, with_roots};
pub use roots::{pop_root, push_root, try_pop_root, try_push_root};
pub use runtime::Runtime;
pub use specialized_array::{
    make_specialized_array, specialized_array_element_type, specialized_array_ref,
    specialized_array_set,
};
pub use stream::stream_slot;
pub use stream::{
    Stream, make_stream, stream_direction, stream_element_type, stream_external_format,
    stream_implementation, stream_state,
};
pub use structure::{
    StructureLayout, make_structure, structure_layout, structure_ref, structure_set,
};
pub use symbol_extensions::{
    set_symbol_constant, set_symbol_macro, set_symbol_package_locked, set_symbol_special,
    set_symbol_value, symbol_flags, symbol_function, symbol_is_constant, symbol_is_macro,
    symbol_is_package_locked, symbol_is_special, symbol_name, symbol_package, symbol_plist,
    symbol_value,
};
pub use typed::{
    ArithmeticError, Array, CellError, Character, Closure, Cons, ControlError, FileError, Fixnum,
    FromLispArg, FunctionDesignator, Integer, LispError, LispString, List, Number, ObjectErrorKind,
    ObjectType, PackageDesignator, PackageError, Pathname, ProgramError, Rational, Real, Sequence,
    SimpleVector, SpecializedArray, StreamError, StringDesignator, StringObject, StructureObject,
    Symbol, TypeError, TypedRustBuiltin, WordView,
};
