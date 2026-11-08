use super::*;

#[test]
fn lexical_references_reject_bindings_hidden_in_sibling_scopes() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(79), 0);
    let mut builder = form_builder(form)?;
    let root = builder.root_scope();
    let child = builder.bindings_mut().push_scope(root)?;
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let binding = builder.bindings_mut().bind_variable(child, name)?;
    let reference = builder.alloc_node(
        root,
        NodeKind::VariableReference {
            target: ncl_hir::node::VariableTarget::Lexical(binding),
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["VARIABLE"])?),
        Err(VerifyError::InvisibleVariable {
            node: reference,
            binding,
        })
    );
    Ok(())
}

#[test]
fn calls_reject_arguments_outside_the_call_scope() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(80), 0);
    let mut builder = form_builder(form)?;
    let root = builder.root_scope();
    let child_scope = builder.bindings_mut().push_scope(root)?;
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let argument = builder.alloc_node(child_scope, NodeKind::Literal(Literal::True))?;
    builder.alloc_node(
        root,
        NodeKind::Call {
            target: CallTarget::Global(name),
            arguments: vec![argument],
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["FUNCTION"])?),
        Err(VerifyError::InvalidScope(child_scope))
    );
    Ok(())
}

#[test]
fn local_function_references_must_be_visible_at_their_use_site() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(81), 0);
    let mut builder = form_builder(form)?;
    let root = builder.root_scope();
    let child = builder.bindings_mut().push_scope(root)?;
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let group =
        builder
            .bindings_mut()
            .bind_local_functions(child, LocalFunctionKind::Flet, &[name])?;
    let definition = group
        .definitions
        .first()
        .copied()
        .ok_or("FLET definition was not created")?;
    let reference = builder.alloc_node(
        root,
        NodeKind::FunctionReference {
            target: CallTarget::Local(definition.binding),
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["LOCAL-FUNCTION"])?),
        Err(VerifyError::InvisibleFunction {
            node: reference,
            binding: definition.binding,
        })
    );
    Ok(())
}

#[test]
fn semantic_node_references_reject_missing_symbols_nodes_blocks_scopes_and_values()
-> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(84), 0);
    let mut builder = form_builder(form)?;
    let missing_symbol = SymbolId::try_from_index(form.compilation_unit(), 4)?;
    builder.alloc_node(
        builder.root_scope(),
        NodeKind::Literal(Literal::Symbol(missing_symbol)),
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidSymbol(missing_symbol))
    );

    let form = FormId::new(CompilationUnitId::new(84), 1);
    let mut builder = form_builder(form)?;
    let missing_node = ncl_hir::NodeId::try_from_index(form, 9)?;
    let function = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(function)?;
    builder.alloc_node(
        builder.root_scope(),
        NodeKind::Call {
            target: CallTarget::Global(function),
            arguments: vec![missing_node],
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["FUNCTION"])?),
        Err(VerifyError::InvalidNode(missing_node))
    );

    let form = FormId::new(CompilationUnitId::new(84), 2);
    let mut builder = form_builder(form)?;
    let missing_block = ncl_hir::BlockId::try_from_index(form, 9)?;
    builder.alloc_node(
        builder.root_scope(),
        NodeKind::Go {
            target: missing_block,
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidBlock(missing_block))
    );

    let form = FormId::new(CompilationUnitId::new(84), 3);
    let mut builder = form_builder(form)?;
    let parent = builder.root_scope();
    let node_scope = builder.bindings_mut().push_scope(parent)?;
    let body_scope = builder.bindings_mut().push_scope(parent)?;
    builder.alloc_node(
        node_scope,
        NodeKind::Sequence {
            body_scope,
            forms: Vec::new(),
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidScope(body_scope))
    );

    let form = FormId::new(CompilationUnitId::new(84), 4);
    let mut builder = form_builder(form)?;
    let missing_value = ValueId::try_from_index(form, 0)?;
    let accepted_type = builder.types_mut().named(BuiltinType::Integer)?;
    let expected_type = builder.add_type_spec_constant(Literal::True)?;
    let source_origin = SourceOriginId::try_from_index(form, 0)?;
    builder.alloc_node(
        builder.root_scope(),
        NodeKind::CheckType {
            value: missing_value,
            accepted_type,
            expected_type,
            source_origin,
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidValue(missing_value))
    );
    Ok(())
}

#[test]
fn semantic_reference_nodes_accept_visible_bindings_and_same_scope_operands()
-> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(103), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let lexical_name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    let global_name = SymbolId::try_from_index(form.compilation_unit(), 1)?;
    let special_name = SymbolId::try_from_index(form.compilation_unit(), 2)?;
    let global_function = SymbolId::try_from_index(form.compilation_unit(), 3)?;
    let intrinsic_function = SymbolId::try_from_index(form.compilation_unit(), 4)?;
    let local_function = SymbolId::try_from_index(form.compilation_unit(), 5)?;
    for symbol in [
        lexical_name,
        global_name,
        special_name,
        global_function,
        intrinsic_function,
        local_function,
    ] {
        builder.register_symbol(symbol)?;
    }
    let group = builder.bindings_mut().bind_local_functions(
        scope,
        LocalFunctionKind::Flet,
        &[local_function],
    )?;
    let variable = builder.bindings_mut().bind_variable(scope, lexical_name)?;
    let definition = group
        .definitions
        .first()
        .copied()
        .ok_or("FLET definition was not created")?;
    let value = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
    let lambda = builder.alloc_node(
        scope,
        NodeKind::Lambda {
            parameters: Vec::new(),
            body_scope: scope,
            body: Vec::new(),
        },
    )?;
    add_variable_references(
        &mut builder,
        scope,
        variable,
        [special_name, global_name],
        value,
    )?;
    add_function_references(
        &mut builder,
        scope,
        global_function,
        intrinsic_function,
        lambda,
        value,
    )?;
    add_local_function_references(&mut builder, scope, &group, definition)?;
    add_operation_references(
        &mut builder,
        scope,
        global_name,
        global_function,
        intrinsic_function,
        value,
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(
            form,
            &[
                "LEXICAL",
                "GLOBAL-VARIABLE",
                "SPECIAL",
                "GLOBAL-FUNCTION",
                "INTRINSIC",
                "LOCAL-FUNCTION",
            ],
        )?),
        Ok(())
    );
    Ok(())
}

fn add_variable_references(
    builder: &mut HirFormBuilder,
    scope: ncl_hir::ScopeId,
    variable: ncl_hir::BindingId,
    names: [SymbolId; 2],
    value: ncl_hir::NodeId,
) -> Result<(), Box<dyn Error>> {
    let [special_name, global_name] = names;
    builder.alloc_node(
        scope,
        NodeKind::VariableReference {
            target: VariableTarget::Lexical(variable),
        },
    )?;
    builder.alloc_node(
        scope,
        NodeKind::VariableReference {
            target: VariableTarget::Special(special_name),
        },
    )?;
    builder.alloc_node(
        scope,
        NodeKind::VariableReference {
            target: VariableTarget::Global(global_name),
        },
    )?;
    builder.alloc_node(
        scope,
        NodeKind::Assignment {
            target: VariableTarget::Lexical(variable),
            value,
        },
    )?;
    Ok(())
}

fn add_function_references(
    builder: &mut HirFormBuilder,
    scope: ncl_hir::ScopeId,
    global_function: SymbolId,
    intrinsic_function: SymbolId,
    lambda: ncl_hir::NodeId,
    value: ncl_hir::NodeId,
) -> Result<(), Box<dyn Error>> {
    builder.alloc_node(
        scope,
        NodeKind::FunctionReference {
            target: CallTarget::Global(global_function),
        },
    )?;
    builder.alloc_node(
        scope,
        NodeKind::FunctionReference {
            target: CallTarget::Intrinsic(intrinsic_function),
        },
    )?;
    builder.alloc_node(scope, NodeKind::ClosureReference { function: lambda })?;
    builder.alloc_node(
        scope,
        NodeKind::Call {
            target: CallTarget::Value(value),
            arguments: vec![value],
        },
    )?;
    Ok(())
}

fn add_local_function_references(
    builder: &mut HirFormBuilder,
    scope: ncl_hir::ScopeId,
    group: &ncl_hir::binding::LocalFunctionGroup,
    definition: ncl_hir::binding::LocalFunctionBinding,
) -> Result<(), Box<dyn Error>> {
    let body_value = builder.alloc_node(group.body_scope, NodeKind::Literal(Literal::Nil))?;
    builder.alloc_node(
        group.body_scope,
        NodeKind::FunctionReference {
            target: CallTarget::Local(definition.binding),
        },
    )?;
    builder.alloc_node(
        group.body_scope,
        NodeKind::Call {
            target: CallTarget::Local(definition.binding),
            arguments: vec![body_value],
        },
    )?;
    builder.alloc_node(
        scope,
        NodeKind::LocalFunctions {
            kind: LocalFunctionKind::Flet,
            body_scope: group.body_scope,
            definitions: vec![LocalFunctionDefinition {
                binding: definition.binding,
                definition_scope: definition.definition_scope,
                parameters: Vec::new(),
                body_scope: scope,
                body: Vec::new(),
            }],
            body: Vec::new(),
        },
    )?;
    Ok(())
}

fn add_operation_references(
    builder: &mut HirFormBuilder,
    scope: ncl_hir::ScopeId,
    global_name: SymbolId,
    global_function: SymbolId,
    intrinsic_function: SymbolId,
    value: ncl_hir::NodeId,
) -> Result<(), Box<dyn Error>> {
    for (function, arguments) in [
        (global_function, vec![value]),
        (intrinsic_function, vec![value]),
        (global_function, vec![value]),
    ] {
        builder.alloc_node(
            scope,
            NodeKind::KnownFunctionOperation {
                function,
                arguments: arguments.clone(),
            },
        )?;
        builder.alloc_node(
            scope,
            NodeKind::SequenceOperation {
                function,
                arguments: arguments.clone(),
            },
        )?;
        builder.alloc_node(
            scope,
            NodeKind::ArrayOperation {
                function,
                arguments,
            },
        )?;
    }
    builder.alloc_node(
        scope,
        NodeKind::If {
            test: value,
            then_branch: value,
            else_branch: Some(value),
        },
    )?;
    builder.alloc_node(
        scope,
        NodeKind::DynamicBinding {
            symbols: value,
            values: value,
            body: vec![value],
        },
    )?;
    builder.alloc_node(
        scope,
        NodeKind::TopLevelDefinition {
            name: global_name,
            kind: DefinitionKind::Variable,
            value,
        },
    )?;
    builder.alloc_node(
        scope,
        NodeKind::CompileTimeForm {
            situations: vec![CompileTimeSituation::CompileToplevel],
            form: value,
        },
    )?;
    builder.alloc_node(
        scope,
        NodeKind::LoadTimeValue {
            form: value,
            read_only: true,
        },
    )?;
    Ok(())
}
