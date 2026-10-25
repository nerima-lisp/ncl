use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError, Package, Parameter,
    Runtime, SetfExpansion, ThreadContext, Word, make_cons,
};

use crate::FfiError;
use crate::memory::{SapAccessType, sap_ref_typed, sap_set_typed};

use super::{OFFSET, SAP, offset_value, sap_value};

const VALUE: Parameter = Parameter {
    name: BuiltinName::new("VALUE"),
    ty: ncl_object::ParameterType::Any,
};

macro_rules! sap_accessors {
    ($(($read:ident, $write:ident, $place:ident, $accessor:literal, $setter:literal, $ty:expr)),* $(,)?) => {
        $(
            fn $read(
                ctx: &mut ThreadContext,
                runtime: &Runtime,
                args: &BuiltinArgs<'_>,
                _: &mut MultipleValues,
            ) -> Result<Word, ObjectError> {
                read(ctx, runtime, args, $ty)
            }

            fn $write(
                ctx: &mut ThreadContext,
                runtime: &Runtime,
                args: &BuiltinArgs<'_>,
                _: &mut MultipleValues,
            ) -> Result<Word, ObjectError> {
                write(ctx, runtime, args, $ty)
            }

            fn $place(
                ctx: &mut ThreadContext,
                runtime: &Runtime,
                args: &[Word],
            ) -> Result<SetfExpansion, ObjectError> {
                place_expander(ctx, runtime, args, $accessor, $setter)
            }
        )*
        const ACCESSORS: &[(&str, &str, ncl_object::RustBuiltin, ncl_object::RustBuiltin, ncl_object::PlaceExpander)] =
            &[$(($accessor, $setter, $read, $write, $place)),*];
    };
}

sap_accessors! {
        (read_u8, write_u8, expand_place_u8, "SAP-REF-8", "%SAP-SET-U8", SapAccessType::Unsigned8),
        (read_i8, write_i8, expand_place_i8, "SIGNED-SAP-REF-8", "%SAP-SET-I8", SapAccessType::Signed8),
        (read_u16, write_u16, expand_place_u16, "SAP-REF-16", "%SAP-SET-U16", SapAccessType::Unsigned16),
        (read_i16, write_i16, expand_place_i16, "SIGNED-SAP-REF-16", "%SAP-SET-I16", SapAccessType::Signed16),
        (read_u32, write_u32, expand_place_u32, "SAP-REF-32", "%SAP-SET-U32", SapAccessType::Unsigned32),
        (read_i32, write_i32, expand_place_i32, "SIGNED-SAP-REF-32", "%SAP-SET-I32", SapAccessType::Signed32),
        (read_u64, write_u64, expand_place_u64, "SAP-REF-64", "%SAP-SET-U64", SapAccessType::Unsigned64),
        (read_i64, write_i64, expand_place_i64, "SIGNED-SAP-REF-64", "%SAP-SET-I64", SapAccessType::Signed64),
        (read_word, write_word, expand_place_word, "SAP-REF-WORD", "%SAP-SET-U64", SapAccessType::Unsigned64),
        (read_signed_word, write_signed_word, expand_place_signed_word, "SIGNED-SAP-REF-WORD", "%SAP-SET-I64", SapAccessType::Signed64),
        (read_sap, write_sap, expand_place_sap, "SAP-REF-SAP", "%SAP-SET-SAP", SapAccessType::Pointer),
        (read_single, write_single, expand_place_single, "SAP-REF-SINGLE", "%SAP-SET-SINGLE", SapAccessType::SingleFloat),
        (read_double, write_double, expand_place_double, "SAP-REF-DOUBLE", "%SAP-SET-DOUBLE", SapAccessType::DoubleFloat),
}

pub(super) fn install(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    for (name, setter, getter, write, place_expander) in ACCESSORS {
        register(runtime, ctx, name, 2, &[SAP, OFFSET], *getter)?;
        register(runtime, ctx, setter, 3, &[SAP, OFFSET, VALUE], *write)?;
        let mut symbol = intern(ctx, runtime, "NCL-FFI", name)?;
        ncl_object::with_root(ctx, &mut symbol, |ctx, symbol| {
            runtime.register_place_expander(ctx, *symbol, *place_expander)
        })?;
    }
    Ok(())
}

fn register(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &'static str,
    arity: u8,
    parameters: &'static [Parameter],
    function: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::NclFfi, BuiltinName::new(name)),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::fixed(parameters),
                convention: BuiltinConvention::Direct(Arity::exact(arity)),
            },
            function,
        ),
    )?;
    Ok(())
}

fn read(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    ty: SapAccessType,
) -> Result<Word, ObjectError> {
    let base = args.required(0)?;
    let offset = args.required(1)?;
    ncl_object::with_roots(ctx, &[base, offset], |ctx, roots| {
        let sap = sap_value(ctx, **roots.first().ok_or(ObjectError::TypeError)?)?;
        let offset = offset_value(**roots.get(1).ok_or(ObjectError::TypeError)?)?;
        sap_ref_typed(ctx, runtime, ty, sap, offset).map_err(FfiError::into_object_error)
    })
}

fn write(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    ty: SapAccessType,
) -> Result<Word, ObjectError> {
    let base = args.required(0)?;
    let offset = args.required(1)?;
    let value = args.required(2)?;
    ncl_object::with_roots(ctx, &[base, offset, value], |ctx, roots| {
        let value = **roots.get(2).ok_or(ObjectError::TypeError)?;
        let sap = sap_value(ctx, **roots.first().ok_or(ObjectError::TypeError)?)?;
        let offset = offset_value(**roots.get(1).ok_or(ObjectError::TypeError)?)?;
        sap_set_typed(ctx, runtime, ty, sap, offset, value).map_err(FfiError::into_object_error)?;
        Ok(value)
    })
}

fn place_expander(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    accessor_name: &'static str,
    setter_name: &'static str,
) -> Result<SetfExpansion, ObjectError> {
    if args.len() != 2 {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let mut package = runtime.ensure_package(ctx, "NCL")?;
        ncl_object::with_root(ctx, &mut package, |ctx, package| {
            let variables = [Word::NIL; 3];
            ncl_object::with_rooted_slice(ctx, &variables, |ctx, variables| {
                for variable in variables.iter_mut() {
                    *variable = ncl_object::Package::from_word(*package).gensym(ctx, runtime)?;
                }
                let base_variable = *variables.first().ok_or(ObjectError::TypeError)?;
                let offset_variable = *variables.get(1).ok_or(ObjectError::TypeError)?;
                let value_variable = *variables.get(2).ok_or(ObjectError::TypeError)?;
                let mut accessor = intern(ctx, runtime, "NCL-FFI", accessor_name)?;
                ncl_object::with_root(ctx, &mut accessor, |ctx, accessor| {
                    let mut setter = intern(ctx, runtime, "NCL-FFI", setter_name)?;
                    ncl_object::with_root(ctx, &mut setter, |ctx, setter| {
                        let mut store_form = make_list(
                            ctx,
                            runtime,
                            &[*setter, base_variable, offset_variable, value_variable],
                        )?;
                        ncl_object::with_root(ctx, &mut store_form, |ctx, store_form| {
                            let access_form = make_list(
                                ctx,
                                runtime,
                                &[*accessor, base_variable, offset_variable],
                            )?;
                            Ok(SetfExpansion {
                                temporary_variables: variables
                                    .get(..2)
                                    .ok_or(ObjectError::TypeError)?
                                    .to_vec(),
                                value_forms: roots.iter().map(|value| **value).collect(),
                                store_variables: vec![value_variable],
                                store_form: *store_form,
                                access_form,
                            })
                        })
                    })
                })
            })
        })
    })
}

fn intern(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    package: &str,
    name: &str,
) -> Result<Word, ObjectError> {
    let package = runtime.ensure_package(ctx, package)?;
    Ok(Package::from_word(package).intern(ctx, runtime, name)?.0)
}

fn make_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    words: &[Word],
) -> Result<Word, ObjectError> {
    ncl_object::with_rooted_slice(ctx, words, |ctx, words| {
        let mut list = Word::NIL;
        for word in words.iter().rev() {
            list = ncl_object::with_root(ctx, &mut list, |ctx, list| {
                make_cons(ctx, runtime, *word, *list)
            })?;
        }
        Ok(list)
    })
}
