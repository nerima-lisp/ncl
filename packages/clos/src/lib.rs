//! CLOS class descriptors and the NCL-MOP registration boundary.

/// Typed CLOS domain aggregates and dispatch metadata.
pub mod domain;
pub mod initialization;
pub mod mop;

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinFunctionCaller, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, CellError, Fixnum, FunctionObject, Instance, LambdaList,
    LispError, Local, MultipleValues, ObjectError, ObjectRef, ObjectType, Package, Runtime, Scope,
    ThreadContext, Word, car, cdr, classify_object, instance_class, make_cons,
    make_instance as allocate_instance, set_symbol_plist, simple_vector_length, simple_vector_ref,
    slot_ref, slot_set, string_length, string_ref, symbol_function, symbol_name, symbol_plist,
};

const COMMON_LISP: &str = "COMMON-LISP";
const NCL_MOP: &str = "NCL-MOP";
const CLASS_NAME: usize = 0;
const CLASS_DIRECT_SUPERCLASS: usize = 1;
const CLASS_SLOTS: usize = 2;
const CLASS_EFFECTIVE_SLOTS: usize = 4;
const METHOD_QUALIFIER_PRIMARY: i64 = 0;
const METHOD_QUALIFIER_BEFORE: i64 = 1;
const METHOD_QUALIFIER_AFTER: i64 = 2;
const METHOD_QUALIFIER_AROUND: i64 = 3;
const CONTINUATION_AROUND: i64 = 0;
const CONTINUATION_PRIMARY: i64 = 1;
const CONTINUATION_CORE: i64 = 2;
const METHOD_REGISTRY_KEY_NAME: &str = "%CLOS-METHODS";
const ARGUMENT: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("ARG"),
    ty: ncl_object::ParameterType::Any,
};
const ARGS_1: &[ncl_object::Parameter] = &[ARGUMENT];
const ARGS_2: &[ncl_object::Parameter] = &[ARGUMENT, ARGUMENT];
const ARGS_3: &[ncl_object::Parameter] = &[ARGUMENT, ARGUMENT, ARGUMENT];

include!("lib_helpers.rs");
include!("lib_dispatch.rs");
include!("lib_macros.rs");
include!("lib_core.rs");
include!("lib_registration.rs");
