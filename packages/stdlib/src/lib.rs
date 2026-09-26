//! Standard-library registration ordering.

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError, Parameter, ParameterType,
    Runtime, ThreadContext, Word, car, make_cons,
};

/// Frozen order for standard-library and extension registration.
pub const REGISTRATION_ORDER: &[&str] = &[
    "ncl-types",
    "ncl-reader",
    "ncl-printer",
    "ncl-conditions",
    "ncl-clos",
    "ncl-lib-numbers",
    "ncl-lib-sequences",
    "ncl-lib-strings",
    "ncl-lib-hash-arrays",
    "ncl-lib-streams",
    "ncl-lib-pathnames",
    "ncl-lib-packages",
    "ncl-lib-format",
    "ncl-lib-macros",
    "ncl-threads",
    "ncl-ffi",
    "ncl-image",
    "ncl-os",
    "ncl-uiop",
    "ncl-asdf",
    "ncl-profiler",
    "ncl-coverage",
    "ncl-debug",
    "ncl-disasm",
];

/// Register every standard-library crate that currently exposes a registration
/// entry point, in [`REGISTRATION_ORDER`].
///
/// Crates without a registration function are intentionally skipped until
/// their implementation lane lands.
///
/// # Errors
///
/// Returns the first [`ObjectError`] reported by a crate registration.
pub fn register_all(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    ncl_types::register(runtime)?;
    ncl_reader::register(runtime)?;
    ncl_printer::register(ctx, runtime)?;
    ncl_conditions::register(runtime)?;
    ncl_clos::register(runtime)?;
    ncl_lib_numbers::register(runtime)?;
    register_core(runtime, ctx)?;
    ncl_lib_streams::register(runtime)?;
    ncl_threads::register(runtime)?;
    ncl_ffi::register(runtime)?;
    ncl_image::register(runtime)?;
    Ok(())
}

fn register_core(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    const ONE: &[Parameter] = &[Parameter {
        name: BuiltinName::new("OBJECT"),
        ty: ParameterType::Any,
    }];
    const TWO: &[Parameter] = &[
        Parameter {
            name: BuiltinName::new("CAR"),
            ty: ParameterType::Any,
        },
        Parameter {
            name: BuiltinName::new("CDR"),
            ty: ParameterType::Any,
        },
    ];
    let car_descriptor = Builtin {
        lambda_list: LambdaList::fixed(ONE),
        convention: BuiltinConvention::Direct(Arity::exact(1)),
    };
    let cons_descriptor = Builtin {
        lambda_list: LambdaList::fixed(TWO),
        convention: BuiltinConvention::Direct(Arity::exact(2)),
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("CAR")),
        BuiltinImplementation::direct(car_descriptor, car_builtin)
            .with_entry(ncl_sys::native_car as *const () as usize),
    )?;
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("CONS")),
        BuiltinImplementation::direct(cons_descriptor, cons_builtin)
            .with_entry(ncl_sys::native_cons as *const () as usize),
    )?;
    Ok(())
}

fn car_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    car(ctx, args.required(0)?)
}

fn cons_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    make_cons(ctx, runtime, args.required(0)?, args.required(1)?)
}
