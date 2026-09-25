//! Registration for array and vector builtins.

use super::builtins_impl::*;
use super::*;

/// Register the array/vector/bit builtins implemented in this module.
pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    const MAKE_ARRAY_KEYS: &[Parameter] = &[
        Parameter {
            name: BuiltinName::new("ELEMENT-TYPE"),
            ty: ParameterType::Any,
        },
        Parameter {
            name: BuiltinName::new("INITIAL-ELEMENT"),
            ty: ParameterType::Any,
        },
        Parameter {
            name: BuiltinName::new("ADJUSTABLE"),
            ty: ParameterType::Any,
        },
        Parameter {
            name: BuiltinName::new("FILL-POINTER"),
            ty: ParameterType::Any,
        },
        Parameter {
            name: BuiltinName::new("DISPLACED-TO"),
            ty: ParameterType::Any,
        },
        Parameter {
            name: BuiltinName::new("DISPLACED-INDEX-OFFSET"),
            ty: ParameterType::Any,
        },
    ];
    const MAKE_ARRAY_REQUIRED: &[Parameter] = &[Parameter {
        name: BuiltinName::new("DIMENSIONS"),
        ty: ParameterType::Any,
    }];
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("MAKE-ARRAY")),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::with_keys(MAKE_ARRAY_REQUIRED, MAKE_ARRAY_KEYS, false),
                convention: BuiltinConvention::Adapted,
            },
            make_array_builtin,
        ),
    )?;
    const ADJUST_ARRAY_KEYS: &[Parameter] = &[Parameter {
        name: BuiltinName::new("ELEMENT-TYPE"),
        ty: ParameterType::Any,
    }];
    const ADJUST_ARRAY_REQUIRED: &[Parameter] = &[
        Parameter {
            name: BuiltinName::new("ARRAY"),
            ty: ParameterType::Any,
        },
        Parameter {
            name: BuiltinName::new("DIMENSIONS"),
            ty: ParameterType::Any,
        },
    ];
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("ADJUST-ARRAY")),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::with_keys(ADJUST_ARRAY_REQUIRED, ADJUST_ARRAY_KEYS, false),
                convention: BuiltinConvention::Adapted,
            },
            adjust_array_builtin,
        ),
    )?;
    register_aref(ctx, runtime)?;
    for (name, arity, function) in [
        (
            "FILL-POINTER",
            1,
            fill_pointer_builtin as ncl_object::RustBuiltin,
        ),
        (
            "ARRAY-HAS-FILL-POINTER-P",
            1,
            array_has_fill_pointer_p_builtin,
        ),
        ("VECTOR-PUSH", 2, vector_push_builtin),
        ("VECTOR-POP", 1, vector_pop_builtin),
        ("ARRAY-DISPLACEMENT", 1, array_displacement_builtin),
    ] {
        register_one(ctx, runtime, name, arity, function)?;
    }
    for (name, arity, function) in [
        (
            "ARRAY-RANK",
            1,
            array_rank_builtin as ncl_object::RustBuiltin,
        ),
        ("ARRAY-DIMENSIONS", 1, array_dimensions_builtin),
        ("ARRAY-TOTAL-SIZE", 1, array_total_size_builtin),
        ("ROW-MAJOR-AREF", 2, row_major_aref_builtin),
        ("SVREF", 2, svref_builtin),
        ("BIT", 2, bit_builtin),
        ("SBIT", 3, sbit_builtin),
        ("BIT-AND", 2, bit_and),
        ("BIT-IOR", 2, bit_ior),
        ("BIT-XOR", 2, bit_xor),
        ("VECTOR-PUSH-EXTEND", 3, vector_push_extend_builtin),
    ] {
        register_one(ctx, runtime, name, arity, function)?;
    }
    Ok(())
}
