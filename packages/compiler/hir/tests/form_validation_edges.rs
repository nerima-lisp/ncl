//! Form construction rejects invalid cross-references without losing valid data.

#![deny(
    unreachable_pub,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::as_conversions
)]

use std::error::Error;

use ncl_hir::binding::BindingEnvironment;
use ncl_hir::cl_type::BuiltinType;
use ncl_hir::{
    BindingId, CompilationUnitId, FormError, FormId, HirDeclaration, HirFormBuilder,
    LambdaSignature, Literal, NodeId, OptimizationPolicy, ScopeMetadata, SourceFiles,
    SourceOrigins, SymbolId, SymbolTable, TypeSpecConstId, VerifyError,
};

fn builder(form: FormId) -> Result<HirFormBuilder, Box<dyn Error>> {
    let mut files = SourceFiles::new(form.compilation_unit());
    let file = files.add("form-validation-edges.lisp", "x")?;
    let span = files.span(file, 0, 1)?;
    let mut origins = SourceOrigins::new(form);
    let origin = origins.add_source(&files, file, span)?;
    Ok(HirFormBuilder::new(form, origins, origin)?)
}

const fn empty_signature() -> LambdaSignature {
    LambdaSignature {
        required: Vec::new(),
        optional: Vec::new(),
        rest: None,
        keys: Vec::new(),
        allow_other_keys: false,
        aux: Vec::new(),
    }
}

#[test]
fn lambda_signature_setter_rejects_foreign_and_absent_lambda_nodes() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(60), 0);
    let foreign_form = FormId::new(CompilationUnitId::new(61), 0);
    let mut builder = builder(form)?;
    let foreign_node = NodeId::try_from_index(foreign_form, 0)?;
    let missing_node = NodeId::try_from_index(form, 0)?;

    assert_eq!(
        builder.set_lambda_signature_for_lambda(foreign_node, empty_signature()),
        Err(FormError::ForeignNode(foreign_node))
    );
    assert_eq!(
        builder.set_lambda_signature_for_lambda(missing_node, empty_signature()),
        Err(FormError::InvalidNode(missing_node))
    );
    Ok(())
}

#[test]
fn local_function_signatures_reject_foreign_missing_and_nonfunction_bindings()
-> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(62), 0);
    let foreign_form = FormId::new(CompilationUnitId::new(63), 0);
    let mut builder = builder(form)?;
    let foreign_binding = BindingId::try_from_index(foreign_form, 0)?;
    let missing_binding = BindingId::try_from_index(form, 0)?;

    assert_eq!(
        builder.set_lambda_signature_for_local_function(foreign_binding, empty_signature()),
        Err(FormError::ForeignBinding(foreign_binding))
    );
    assert_eq!(
        builder.set_lambda_signature_for_local_function(missing_binding, empty_signature()),
        Err(FormError::InvalidFunctionSignature)
    );

    let root = builder.root_scope();
    let variable_name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    let variable = builder.bindings_mut().bind_variable(root, variable_name)?;
    assert_eq!(
        builder.set_lambda_signature_for_local_function(variable, empty_signature()),
        Err(FormError::InvalidFunctionSignature)
    );

    let orphan_name = SymbolId::try_from_index(form.compilation_unit(), 1)?;
    let orphan_function = builder.bindings_mut().bind_function(root, orphan_name)?;
    assert_eq!(
        builder.set_lambda_signature_for_local_function(orphan_function, empty_signature()),
        Err(FormError::InvalidFunctionSignature)
    );
    Ok(())
}

#[test]
fn nested_foreign_type_literal_is_rejected_without_consuming_a_constant_id()
-> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(64), 0);
    let foreign_form = FormId::new(CompilationUnitId::new(65), 0);
    let mut builder = builder(form)?;
    let foreign_symbol = SymbolId::try_from_index(foreign_form.compilation_unit(), 0)?;
    let nested_literal = Literal::Vector(vec![Literal::Cons(
        Box::new(Literal::Symbol(foreign_symbol)),
        Box::new(Literal::True),
    )]);

    assert_eq!(
        builder.add_type_spec_constant(nested_literal),
        Err(FormError::ForeignSymbol(foreign_symbol))
    );
    let constant = builder.add_type_spec_constant(Literal::True)?;
    assert_eq!(constant, TypeSpecConstId::try_from_index(form, 0)?);

    let hir = builder.finish()?;
    let foreign_constant = TypeSpecConstId::try_from_index(foreign_form, 0)?;
    assert_eq!(hir.type_spec_constant(constant), Some(&Literal::True));
    assert_eq!(hir.type_spec_constant(foreign_constant), None);
    assert_eq!(hir.type_spec_constant_ids().len(), 1);
    assert_eq!(
        hir.verify(&SymbolTable::new(form.compilation_unit())),
        Ok(())
    );
    Ok(())
}

#[test]
fn unknown_declaration_verification_rejects_an_uninterned_nested_symbol()
-> Result<(), Box<dyn Error>> {
    let unit = CompilationUnitId::new(66);
    let form = FormId::new(unit, 0);
    let declaration_name = SymbolId::try_from_index(unit, 0)?;
    let hidden_operand = SymbolId::try_from_index(unit, 1)?;
    let mut builder = builder(form)?;
    let root = builder.root_scope();
    builder.set_scope_metadata(
        root,
        ScopeMetadata {
            declarations: vec![HirDeclaration::Unknown {
                name: declaration_name,
                arguments: vec![Literal::Vector(vec![Literal::Cons(
                    Box::new(Literal::Nil),
                    Box::new(Literal::Symbol(hidden_operand)),
                )])],
            }],
            policy: OptimizationPolicy::default(),
        },
    )?;
    let hir = builder.finish()?;
    let mut symbols = SymbolTable::new(unit);
    assert_eq!(
        symbols.intern(form, Some("CL"), "EXTENSION", None),
        Ok(declaration_name)
    );

    assert_eq!(
        hir.verify(&symbols),
        Err(VerifyError::InvalidSymbol(hidden_operand))
    );
    Ok(())
}

#[test]
fn rejected_scope_metadata_update_preserves_the_previous_declarations() -> Result<(), Box<dyn Error>>
{
    let unit = CompilationUnitId::new(67);
    let form = FormId::new(unit, 0);
    let foreign_form = FormId::new(CompilationUnitId::new(68), 0);
    let local_name = SymbolId::try_from_index(unit, 0)?;
    let foreign_name = SymbolId::try_from_index(foreign_form.compilation_unit(), 0)?;
    let policy = OptimizationPolicy::default();
    let mut builder = builder(form)?;
    let root = builder.root_scope();
    let stored_declarations = vec![HirDeclaration::Special {
        names: vec![local_name],
    }];
    builder.set_scope_metadata(
        root,
        ScopeMetadata {
            declarations: stored_declarations.clone(),
            policy,
        },
    )?;
    let mut other_bindings = BindingEnvironment::new(form);
    let other_root = other_bindings.root_scope();
    let unallocated_scope = other_bindings.push_scope(other_root)?;
    assert_eq!(
        builder.set_scope_metadata(
            unallocated_scope,
            ScopeMetadata {
                declarations: stored_declarations.clone(),
                policy,
            }
        ),
        Err(FormError::InvalidScope(unallocated_scope))
    );
    assert_eq!(
        builder.set_scope_metadata(
            root,
            ScopeMetadata {
                declarations: vec![HirDeclaration::Special {
                    names: vec![local_name, foreign_name],
                }],
                policy,
            }
        ),
        Err(FormError::ForeignSymbol(foreign_name))
    );

    let hir = builder.finish()?;
    let metadata = hir.scope_metadata().get(&root);
    assert_eq!(
        metadata.map(|metadata| metadata.declarations.as_slice()),
        Some(stored_declarations.as_slice())
    );
    let mut symbols = SymbolTable::new(unit);
    assert_eq!(
        symbols.intern(form, Some("CL"), "LOCAL", None),
        Ok(local_name)
    );
    assert_eq!(hir.verify(&symbols), Ok(()));
    Ok(())
}

#[test]
fn ftype_metadata_verification_requires_its_function_name_in_the_symbol_table()
-> Result<(), Box<dyn Error>> {
    let unit = CompilationUnitId::new(69);
    let form = FormId::new(unit, 0);
    let type_name = SymbolId::try_from_index(unit, 0)?;
    let function_name = SymbolId::try_from_index(unit, 1)?;
    let mut builder = builder(form)?;
    let root = builder.root_scope();
    let source_specifier = builder.add_type_spec_constant(Literal::Symbol(type_name))?;
    let function_type = builder.types_mut().named(BuiltinType::Function)?;
    let declaration = HirDeclaration::Ftype {
        type_id: function_type,
        source_specifier,
        name: function_name,
    };
    builder.set_scope_metadata(
        root,
        ScopeMetadata {
            declarations: vec![declaration.clone()],
            policy: OptimizationPolicy::default(),
        },
    )?;
    let hir = builder.finish()?;
    let mut symbols = SymbolTable::new(unit);
    assert_eq!(
        symbols.intern(form, Some("CL"), "FUNCTION-TYPE", None),
        Ok(type_name)
    );
    assert_eq!(
        hir.verify(&symbols),
        Err(VerifyError::InvalidSymbol(function_name))
    );
    assert_eq!(
        symbols.intern(form, Some("CL"), "FUNCTION-NAME", None),
        Ok(function_name)
    );
    assert_eq!(hir.verify(&symbols), Ok(()));
    assert_eq!(
        hir.scope_metadata()
            .get(&root)
            .map(|metadata| metadata.declarations.as_slice()),
        Some([declaration].as_slice())
    );
    Ok(())
}
