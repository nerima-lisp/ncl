//! Form construction preserves typed ownership and rejects incomplete references.

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

use ncl_hir::arena::Arena;
use ncl_hir::binding::BindingEnvironment;
use ncl_hir::cl_type::BuiltinType;
use ncl_hir::{
    BindingId, BlockId, CleanupId, CompilationUnitId, ControlError, FormError, FormId,
    HirDeclaration, HirFormBuilder, IdError, LambdaKey, LambdaSignature, Literal, NodeId, NodeKind,
    OptimizationPolicy, ScopeMetadata, SourceFiles, SourceOriginId, SourceOrigins, SymbolId,
    SymbolTable, TypeId, TypeSpecConstId, ValueId, VerifyError,
};

fn source_origins(form: FormId) -> Result<(SourceOrigins, SourceOriginId), Box<dyn Error>> {
    let mut files = SourceFiles::new(form.compilation_unit());
    let file = files.add("form-boundaries.lisp", "t")?;
    let span = files.span(file, 0, 1)?;
    let mut origins = SourceOrigins::new(form);
    let origin = origins.add_source(&files, file, span)?;
    Ok((origins, origin))
}

fn builder(form: FormId) -> Result<HirFormBuilder, Box<dyn Error>> {
    let (origins, origin) = source_origins(form)?;
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
fn arena_mutation_and_consumption_preserve_append_order() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(20), 4);
    let foreign_form = FormId::new(CompilationUnitId::new(20), 5);
    let mut values = Arena::<u32, NodeId>::new(form);
    let first = values.allocate(10)?;
    let second = values.allocate(20)?;
    let foreign = NodeId::try_from_index(foreign_form, 0)?;
    let missing = NodeId::try_from_index(form, 2)?;

    if let Some(value) = values.get_mut(first) {
        *value = 11;
    }
    assert_eq!(values.get(second).copied(), Some(20));
    assert_eq!(values.get_mut(foreign), None);
    assert_eq!(values.get_mut(missing), None);
    assert_eq!(
        values
            .iter()
            .map(|(index, value)| (index, *value))
            .collect::<Vec<_>>(),
        vec![(0, 11), (1, 20)]
    );
    assert_eq!(values.into_values(), vec![11, 20]);
    Ok(())
}

#[cfg(target_pointer_width = "64")]
#[test]
fn typed_identity_families_report_u32_index_overflow() -> Result<(), Box<dyn Error>> {
    let unit = CompilationUnitId::new(21);
    let form = FormId::new(unit, 1);
    let too_large = usize::try_from(u64::from(u32::MAX) + 1)?;
    let expected = IdError::IndexOverflow { index: too_large };

    assert_eq!(FormId::try_from_index(unit, too_large), Err(expected));
    assert_eq!(NodeId::try_from_index(form, too_large), Err(expected));
    assert_eq!(BindingId::try_from_index(form, too_large), Err(expected));
    assert_eq!(BlockId::try_from_index(form, too_large), Err(expected));
    assert_eq!(CleanupId::try_from_index(form, too_large), Err(expected));
    assert_eq!(TypeId::try_from_index(form, too_large), Err(expected));
    assert_eq!(ValueId::try_from_index(form, too_large), Err(expected));
    assert_eq!(
        SourceOriginId::try_from_index(form, too_large),
        Err(expected)
    );
    assert_eq!(
        TypeSpecConstId::try_from_index(form, too_large),
        Err(expected)
    );
    assert_eq!(SymbolId::try_from_index(unit, too_large), Err(expected));
    assert_eq!(
        expected.to_string(),
        format!("arena index {too_large} exceeds the u32 range")
    );
    Ok(())
}

#[test]
fn form_builder_requires_a_source_origin_owned_by_the_form() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(22), 0);
    let foreign_form = FormId::new(CompilationUnitId::new(23), 0);
    let (foreign_origins, foreign_default) = source_origins(foreign_form)?;
    assert!(matches!(
        HirFormBuilder::new(form, foreign_origins, foreign_default),
        Err(FormError::ForeignForm(found)) if found == foreign_form
    ));

    let (origins, _) = source_origins(form)?;
    let missing_origin = SourceOriginId::try_from_index(form, 1)?;
    assert!(matches!(
        HirFormBuilder::new(form, origins, missing_origin),
        Err(FormError::InvalidOrigin(found)) if found == missing_origin
    ));
    Ok(())
}

#[test]
fn form_builder_rejects_foreign_and_missing_scope_node_and_origin_ids() -> Result<(), Box<dyn Error>>
{
    let form = FormId::new(CompilationUnitId::new(24), 0);
    let foreign_form = FormId::new(CompilationUnitId::new(25), 0);
    let mut builder = builder(form)?;
    let foreign_scope = BindingEnvironment::new(foreign_form).root_scope();
    assert_eq!(
        builder.alloc_node(foreign_scope, NodeKind::Literal(Literal::True)),
        Err(FormError::ForeignScope(foreign_scope))
    );

    let mut other_bindings = BindingEnvironment::new(form);
    let parent_scope = other_bindings.root_scope();
    let absent_scope = other_bindings.push_scope(parent_scope)?;
    assert_eq!(
        builder.alloc_node(absent_scope, NodeKind::Literal(Literal::True)),
        Err(FormError::InvalidScope(absent_scope))
    );

    let foreign_origin = SourceOriginId::try_from_index(foreign_form, 0)?;
    assert_eq!(
        builder.alloc_node_at(
            builder.root_scope(),
            foreign_origin,
            NodeKind::Literal(Literal::True)
        ),
        Err(FormError::InvalidOrigin(foreign_origin))
    );

    let foreign_node = NodeId::try_from_index(foreign_form, 0)?;
    let missing_node = NodeId::try_from_index(form, 0)?;
    assert_eq!(
        builder.add_root(foreign_node),
        Err(FormError::ForeignNode(foreign_node))
    );
    assert_eq!(
        builder.add_root(missing_node),
        Err(FormError::InvalidNode(missing_node))
    );

    let root = builder.alloc_node(builder.root_scope(), NodeKind::Literal(Literal::True))?;
    builder.add_root(root)?;
    let hir = builder.finish()?;
    assert_eq!(hir.roots(), &[root]);
    assert_eq!(
        hir.verify(&SymbolTable::new(form.compilation_unit())),
        Ok(())
    );
    Ok(())
}

#[test]
fn form_builder_records_one_producer_for_each_ssa_value() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(26), 0);
    let foreign_form = FormId::new(CompilationUnitId::new(27), 0);
    let mut builder = builder(form)?;
    let value = ValueId::try_from_index(form, 0)?;
    let foreign_value = ValueId::try_from_index(foreign_form, 0)?;
    let missing_node = NodeId::try_from_index(form, 0)?;
    let foreign_node = NodeId::try_from_index(foreign_form, 0)?;

    assert_eq!(
        builder.register_value(foreign_value),
        Err(FormError::ForeignValue(foreign_value))
    );
    assert_eq!(
        builder.register_value_producer(value, foreign_node),
        Err(FormError::ForeignNode(foreign_node))
    );
    assert_eq!(
        builder.register_value_producer(value, missing_node),
        Err(FormError::InvalidNode(missing_node))
    );

    let producer = builder.alloc_node(builder.root_scope(), NodeKind::Literal(Literal::True))?;
    builder.register_value_producer(value, producer)?;
    assert_eq!(
        builder.register_value_producer(value, producer),
        Err(FormError::DuplicateValueProducer(value))
    );
    let hir = builder.finish()?;
    assert_eq!(hir.value_producer(value), Some(producer));
    assert!(hir.value_ids().contains(&value));
    Ok(())
}

#[test]
fn scope_metadata_rejects_foreign_and_missing_type_ids() -> Result<(), Box<dyn Error>> {
    let unit = CompilationUnitId::new(28);
    let form = FormId::new(unit, 0);
    let foreign_form = FormId::new(CompilationUnitId::new(29), 0);
    let mut builder = builder(form)?;
    let root = builder.root_scope();
    let foreign_type = TypeId::try_from_index(foreign_form, 0)?;
    let missing_type = TypeId::try_from_index(form, builder.types_mut().len())?;
    let constant = builder.add_type_spec_constant(Literal::True)?;
    let policy = OptimizationPolicy::default();
    let foreign_scope = BindingEnvironment::new(foreign_form).root_scope();
    assert_eq!(
        builder.set_scope_metadata(
            foreign_scope,
            ScopeMetadata {
                declarations: Vec::new(),
                policy,
            }
        ),
        Err(FormError::ForeignScope(foreign_scope))
    );

    assert_eq!(
        builder.set_scope_metadata(
            root,
            ScopeMetadata {
                declarations: vec![HirDeclaration::Type {
                    type_id: foreign_type,
                    source_specifier: constant,
                    names: Vec::new(),
                }],
                policy,
            }
        ),
        Err(FormError::ForeignType(foreign_type))
    );
    assert_eq!(
        builder.set_scope_metadata(
            root,
            ScopeMetadata {
                declarations: vec![HirDeclaration::Type {
                    type_id: missing_type,
                    source_specifier: constant,
                    names: Vec::new(),
                }],
                policy,
            }
        ),
        Err(FormError::InvalidType(missing_type))
    );

    let integer_type = builder.types_mut().named(BuiltinType::Integer)?;
    let foreign_constant = TypeSpecConstId::try_from_index(foreign_form, 0)?;
    let missing_constant = TypeSpecConstId::try_from_index(form, 1)?;
    for source_specifier in [foreign_constant, missing_constant] {
        let expected_error = if source_specifier == foreign_constant {
            FormError::ForeignTypeSpecConstant(source_specifier)
        } else {
            FormError::InvalidTypeSpecConstant(source_specifier)
        };
        assert_eq!(
            builder.set_scope_metadata(
                root,
                ScopeMetadata {
                    declarations: vec![HirDeclaration::Type {
                        type_id: integer_type,
                        source_specifier,
                        names: Vec::new(),
                    }],
                    policy,
                }
            ),
            Err(expected_error)
        );
    }

    Ok(())
}

#[test]
fn scope_metadata_rejects_foreign_symbol_owners() -> Result<(), Box<dyn Error>> {
    let unit = CompilationUnitId::new(34);
    let form = FormId::new(unit, 0);
    let foreign_form = FormId::new(CompilationUnitId::new(35), 0);
    let mut builder = builder(form)?;
    let root = builder.root_scope();
    let policy = OptimizationPolicy::default();
    let foreign_symbol = SymbolId::try_from_index(foreign_form.compilation_unit(), 0)?;

    assert_eq!(
        builder.add_type_spec_constant(Literal::Symbol(foreign_symbol)),
        Err(FormError::ForeignSymbol(foreign_symbol))
    );
    assert_eq!(
        builder.set_scope_metadata(
            root,
            ScopeMetadata {
                declarations: vec![HirDeclaration::Special {
                    names: vec![foreign_symbol],
                }],
                policy,
            }
        ),
        Err(FormError::ForeignSymbol(foreign_symbol))
    );
    Ok(())
}

#[test]
fn scope_metadata_preserves_type_source_and_requires_interned_symbols() -> Result<(), Box<dyn Error>>
{
    let unit = CompilationUnitId::new(36);
    let form = FormId::new(unit, 0);
    let mut builder = builder(form)?;
    let root = builder.root_scope();
    let policy = OptimizationPolicy::default();
    let type_name = SymbolId::try_from_index(unit, 0)?;
    let name = SymbolId::try_from_index(unit, 1)?;
    let constant = builder.add_type_spec_constant(Literal::Symbol(type_name))?;
    let integer_type = builder.types_mut().named(BuiltinType::Integer)?;

    builder.set_scope_metadata(
        root,
        ScopeMetadata {
            declarations: vec![HirDeclaration::Type {
                type_id: integer_type,
                source_specifier: constant,
                names: vec![name],
            }],
            policy,
        },
    )?;
    let hir = builder.finish()?;
    let stored_metadata = hir
        .scope_metadata()
        .get(&root)
        .ok_or("root scope metadata was not retained")?;
    let expected_declarations = [HirDeclaration::Type {
        type_id: integer_type,
        source_specifier: constant,
        names: vec![name],
    }];
    assert_eq!(
        stored_metadata.declarations.as_slice(),
        expected_declarations.as_slice()
    );
    assert_eq!(
        hir.type_spec_constant(constant),
        Some(&Literal::Symbol(type_name))
    );
    assert!(hir.symbol_ids().contains(&name));
    assert!(hir.symbol_ids().contains(&type_name));
    let mut symbols = SymbolTable::new(unit);
    assert_eq!(
        hir.verify(&SymbolTable::new(CompilationUnitId::new(32))),
        Err(VerifyError::WrongForm)
    );
    assert!(matches!(
        hir.verify(&symbols),
        Err(VerifyError::InvalidSymbol(found)) if found == name || found == type_name
    ));
    assert_eq!(
        symbols.intern(form, Some("CL"), "INTEGER", None),
        Ok(type_name)
    );
    assert_eq!(symbols.intern(form, Some("CL"), "X", None), Ok(name));
    assert_eq!(hir.verify(&symbols), Ok(()));
    Ok(())
}

#[test]
fn lambda_signatures_reject_wrong_nodes_and_preserve_valid_parameters() -> Result<(), Box<dyn Error>>
{
    let unit = CompilationUnitId::new(30);
    let form = FormId::new(unit, 0);
    let mut builder = builder(form)?;
    let definition_scope = builder.root_scope();
    let non_lambda = builder.alloc_node(definition_scope, NodeKind::Literal(Literal::True))?;
    assert_eq!(
        builder.set_lambda_signature_for_lambda(non_lambda, empty_signature()),
        Err(FormError::InvalidFunctionSignature)
    );

    let parameter_scope = builder.bindings_mut().push_scope(definition_scope)?;
    let parameter_symbol = SymbolId::try_from_index(unit, 0)?;
    builder.register_symbol(parameter_symbol)?;
    let parameter = builder
        .bindings_mut()
        .bind_variable(parameter_scope, parameter_symbol)?;
    let lambda = builder.alloc_node(
        definition_scope,
        NodeKind::Lambda {
            parameters: vec![parameter],
            body_scope: parameter_scope,
            body: Vec::new(),
        },
    )?;
    let duplicate_signature = LambdaSignature {
        required: vec![parameter, parameter],
        ..empty_signature()
    };
    assert_eq!(
        builder.set_lambda_signature_for_lambda(lambda, duplicate_signature),
        Err(FormError::InvalidFunctionSignature)
    );
    let foreign_keyword = SymbolId::try_from_index(CompilationUnitId::new(33), 0)?;
    let foreign_keyword_signature = LambdaSignature {
        keys: vec![LambdaKey {
            keyword: foreign_keyword,
            binding: parameter,
            default: None,
            initializer_scope: definition_scope,
            supplied_p: None,
        }],
        ..empty_signature()
    };
    assert_eq!(
        builder.set_lambda_signature_for_lambda(lambda, foreign_keyword_signature),
        Err(FormError::ForeignSymbol(foreign_keyword))
    );
    let signature = LambdaSignature {
        required: vec![parameter],
        ..empty_signature()
    };
    builder.set_lambda_signature_for_lambda(lambda, signature)?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.lambda_signature_for_lambda(lambda)
            .map(|signature| signature.required.as_slice()),
        Some([parameter].as_slice())
    );
    let mut symbols = SymbolTable::new(unit);
    assert_eq!(
        symbols.intern(form, Some("CL"), "ARG", None),
        Ok(parameter_symbol)
    );
    assert_eq!(hir.verify(&symbols), Ok(()));
    Ok(())
}

#[test]
fn form_finishing_rejects_missing_cfg_entries_and_terminators() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(31), 0);
    let mut no_entry = builder(form)?;
    let function = no_entry.alloc_node(no_entry.root_scope(), NodeKind::Literal(Literal::True))?;
    no_entry.control_flow_mut().begin_function(function)?;
    assert!(matches!(
        no_entry.finish(),
        Err(FormError::Control(ControlError::MissingEntry(found))) if found == function
    ));

    let mut unterminated = builder(FormId::new(CompilationUnitId::new(31), 1))?;
    let function =
        unterminated.alloc_node(unterminated.root_scope(), NodeKind::Literal(Literal::True))?;
    unterminated.control_flow_mut().begin_function(function)?;
    let entry = unterminated.control_flow_mut().create_block(function)?;
    assert!(matches!(
        unterminated.finish(),
        Err(FormError::Control(ControlError::MissingTerminator(found))) if found == entry
    ));
    Ok(())
}

#[test]
fn form_errors_display_their_context() {
    assert_eq!(
        FormError::InvalidFunctionSignature.to_string(),
        "HIR form error: InvalidFunctionSignature"
    );
}
