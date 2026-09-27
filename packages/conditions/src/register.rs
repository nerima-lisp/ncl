//! Registration of the owned symbols and the standard condition hierarchy.

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, Instance, LambdaList, ObjectError, Package, Runtime,
    ThreadContext, Word, instance_class, set_symbol_special, slot_ref,
};

use crate::class::{HIERARCHY, class_named, install_class, wire_superclass};
use crate::symbols::{SymbolKind, SymbolRow, symbols};

/// Register every owned symbol and the standard condition hierarchy.
///
/// This is the per-crate registration entry point that `ncl-stdlib` calls in
/// dependency order. It creates an internal context, so callers pass only the
/// shared runtime.
///
/// # Errors
/// Returns an object-layer error when allocation, interning, or registration
/// fails.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for row in symbols() {
        register_symbol(runtime, &mut ctx, row)?;
    }
    install_hierarchy(runtime, &mut ctx)?;
    register_accessors(runtime, &mut ctx)?;
    runtime.register_lisp_error_converter(crate::condition_from_lisp_error);
    Ok(())
}

const CELL_ERROR_PARAMETER: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("CONDITION"),
    ty: ncl_object::ParameterType::Any,
};

fn cell_error_name(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let condition = args.required(0)?;
    let class = instance_class(ctx, Instance::from_word(condition))?;
    if !class_named(ctx, class, "CELL-ERROR")? {
        return Err(ObjectError::TypeError);
    }
    slot_ref(ctx, Instance::from_word(condition), 0)
}

fn register_accessors(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    runtime
        .register_builtin(
            ctx,
            BuiltinIdentifier::new(
                BuiltinPackage::CommonLisp,
                BuiltinName::new("CELL-ERROR-NAME"),
            ),
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::fixed(&[CELL_ERROR_PARAMETER]),
                    convention: BuiltinConvention::Direct(Arity::exact(1)),
                },
                cell_error_name,
            ),
        )
        .map(|_| ())
}

fn register_symbol(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    row: &SymbolRow,
) -> Result<(), ObjectError> {
    let package = runtime.ensure_package(ctx, row.package)?;
    let (symbol, _status) = Package::from_word(package).intern(ctx, runtime, row.name)?;
    match row.kind {
        SymbolKind::Function | SymbolKind::ClassAndFunction => {
            runtime.define_function(ctx, row.package, row.name, Word::UNBOUND)?;
        }
        SymbolKind::Variable => set_symbol_special(ctx, symbol, true)?,
        SymbolKind::Class
        | SymbolKind::Other
        | SymbolKind::Constant
        | SymbolKind::Type
        | SymbolKind::Macro
        | SymbolKind::MacroAndClass
        | SymbolKind::SpecialOperatorAndClass
        | SymbolKind::VariableAndFunction => {}
    }
    Ok(())
}

fn install_hierarchy(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    for row in symbols() {
        match row.kind {
            SymbolKind::Class | SymbolKind::ClassAndFunction => {
                install_class(ctx, runtime, row.name)?;
            }
            SymbolKind::Function
            | SymbolKind::Variable
            | SymbolKind::Other
            | SymbolKind::Constant
            | SymbolKind::Type
            | SymbolKind::Macro
            | SymbolKind::MacroAndClass
            | SymbolKind::SpecialOperatorAndClass
            | SymbolKind::VariableAndFunction => {}
        }
    }
    for row in HIERARCHY {
        if let Some(parent) = row.superclass {
            wire_superclass(ctx, runtime, row.name, parent)?;
        }
    }
    Ok(())
}
