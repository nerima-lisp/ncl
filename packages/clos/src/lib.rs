//! CLOS class descriptors and the NCL-MOP registration boundary.

/// Typed CLOS domain aggregates and dispatch metadata.
pub mod domain;
pub mod initialization;
pub mod mop;

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinFunctionCaller, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, Fixnum, FunctionObject, Instance, LambdaList, Local,
    MultipleValues, ObjectError, ObjectRef, ObjectType, Package, Runtime, Scope, Stream,
    ThreadContext, Word, car, cdr, classify_object, instance_class,
    make_instance as allocate_instance, simple_vector_length, simple_vector_ref, slot_ref,
    slot_set, symbol_function,
};

const COMMON_LISP: &str = "COMMON-LISP";
const NCL_MOP: &str = "NCL-MOP";
const CLASS_NAME: usize = 0;
const CLASS_DIRECT_SUPERCLASS: usize = 1;
const CLASS_SLOTS: usize = 2;
const CLASS_KIND: usize = 3;
const CLASS_EFFECTIVE_SLOTS: usize = 4;
const METHOD_REGISTRY_KEY_NAME: &str = "%CLOS-METHODS";
const METHOD_COMBINATION_KEY_NAME: &str = "%CLOS-METHOD-COMBINATION";

const ARGUMENT: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("ARG"),
    ty: ncl_object::ParameterType::Any,
};
const ARGS_1: &[ncl_object::Parameter] = &[ARGUMENT];
const ARGS_2: &[ncl_object::Parameter] = &[ARGUMENT, ARGUMENT];
const ARGS_3: &[ncl_object::Parameter] = &[ARGUMENT, ARGUMENT, ARGUMENT];

include!("lib_helpers.rs");
mod lib_dispatch_effective_methods;
mod lib_dispatch_selection;
include!("lib_dispatch.rs");
include!("lib_generic_registration.rs");
include!("lib_find_method.rs");
include!("lib_macros.rs");
include!("lib_defmethod_macro.rs");
include!("lib_defstruct.rs");
include!("lib_structures.rs");
include!("lib_core.rs");
include!("lib_core_typep.rs");
include!("lib_registration.rs");

#[cfg(test)]
#[path = "defmethod_macro_tests.rs"]
mod defmethod_macro_tests;

#[cfg(test)]
#[path = "symbol_plist_tests.rs"]
mod symbol_plist_tests;
