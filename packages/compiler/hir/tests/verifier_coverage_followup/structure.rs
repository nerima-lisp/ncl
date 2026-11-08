use super::*;

#[test]
fn multiple_value_variants_accept_well_scoped_operands() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(85), 0);
    let mut builder = form_builder(form)?;
    let root = builder.root_scope();
    let body_scope = builder.bindings_mut().push_scope(root)?;
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let binding = builder.bindings_mut().bind_variable(body_scope, name)?;
    let value = builder.alloc_node(root, NodeKind::Literal(Literal::True))?;
    builder.alloc_node(
        root,
        NodeKind::MultipleValues(MultipleValueOperation::Produce {
            values: vec![value],
        }),
    )?;
    builder.alloc_node(
        root,
        NodeKind::MultipleValues(MultipleValueOperation::Return {
            values: vec![value],
        }),
    )?;
    builder.alloc_node(
        root,
        NodeKind::MultipleValues(MultipleValueOperation::Prog1 {
            first: value,
            rest: vec![value],
        }),
    )?;
    builder.alloc_node(
        root,
        NodeKind::MultipleValues(MultipleValueOperation::Bind {
            value,
            bindings: vec![binding],
            body_scope,
            body: Vec::new(),
        }),
    )?;
    builder.alloc_node(
        root,
        NodeKind::MultipleValues(MultipleValueOperation::Call {
            function: value,
            arguments: vec![value],
        }),
    )?;

    let hir = builder.finish()?;
    assert_eq!(hir.verify(&symbol_table(form, &["VALUE"])?), Ok(()));
    Ok(())
}

#[test]
fn multiple_value_bind_requires_a_distinct_body_scope_for_bindings() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(75), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let binding = builder.bindings_mut().bind_variable(scope, name)?;
    let value = builder.alloc_node(scope, NodeKind::Literal(Literal::Nil))?;
    let operation = builder.alloc_node(
        scope,
        NodeKind::MultipleValues(MultipleValueOperation::Bind {
            value,
            bindings: vec![binding],
            body_scope: scope,
            body: Vec::new(),
        }),
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["VALUE"])?),
        Err(VerifyError::InvalidBindingScopes(operation))
    );
    Ok(())
}

#[test]
fn parallel_let_bindings_must_be_lexical_variables() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(76), 0);
    let mut builder = form_builder(form)?;
    let parent = builder.root_scope();
    let body_scope = builder.bindings_mut().push_scope(parent)?;
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let binding = builder.bindings_mut().bind_function(body_scope, name)?;
    builder.alloc_node(
        parent,
        NodeKind::Let {
            kind: LetKind::Parallel,
            bindings: vec![LetBinding {
                binding,
                initializer: None,
            }],
            initializer_scopes: vec![parent],
            binding_scopes: vec![body_scope],
            body_scope,
            body: Vec::new(),
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["FUNCTION"])?),
        Err(VerifyError::InvalidBinding(binding))
    );
    Ok(())
}

#[test]
fn lambda_parameters_require_a_distinct_body_scope() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(90), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let parameter = builder.bindings_mut().bind_variable(scope, name)?;
    let lambda = builder.alloc_node(
        scope,
        NodeKind::Lambda {
            parameters: vec![parameter],
            body_scope: scope,
            body: Vec::new(),
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["PARAMETER"])?),
        Err(VerifyError::InvalidBindingScopes(lambda))
    );
    Ok(())
}

#[test]
fn let_initializer_node_must_match_its_declared_scope() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(91), 0);
    let mut builder = form_builder(form)?;
    let parent = builder.root_scope();
    let body_scope = builder.bindings_mut().push_scope(parent)?;
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let binding = builder.bindings_mut().bind_variable(body_scope, name)?;
    let initializer = builder.alloc_node(body_scope, NodeKind::Literal(Literal::True))?;
    let let_node = builder.alloc_node(
        parent,
        NodeKind::Let {
            kind: LetKind::Parallel,
            bindings: vec![LetBinding {
                binding,
                initializer: Some(initializer),
            }],
            initializer_scopes: vec![parent],
            binding_scopes: vec![body_scope],
            body_scope,
            body: Vec::new(),
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["VARIABLE"])?),
        Err(VerifyError::InvalidBindingScopes(let_node))
    );
    Ok(())
}

#[test]
fn local_function_definitions_must_preserve_their_binding_scopes() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(92), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let group =
        builder
            .bindings_mut()
            .bind_local_functions(scope, LocalFunctionKind::Flet, &[name])?;
    let definition = group
        .definitions
        .first()
        .copied()
        .ok_or("FLET definition was not created")?;
    let collapsed = builder.alloc_node(
        scope,
        NodeKind::LocalFunctions {
            kind: LocalFunctionKind::Flet,
            body_scope: scope,
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
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["LOCAL-FUNCTION"])?),
        Err(VerifyError::InvalidBindingScopes(collapsed))
    );

    let form = FormId::new(CompilationUnitId::new(92), 1);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let group =
        builder
            .bindings_mut()
            .bind_local_functions(scope, LocalFunctionKind::Flet, &[name])?;
    let definition = group
        .definitions
        .first()
        .copied()
        .ok_or("FLET definition was not created")?;
    let wrong_definition_scope = builder.alloc_node(
        scope,
        NodeKind::LocalFunctions {
            kind: LocalFunctionKind::Flet,
            body_scope: group.body_scope,
            definitions: vec![LocalFunctionDefinition {
                binding: definition.binding,
                definition_scope: group.body_scope,
                parameters: Vec::new(),
                body_scope: group.body_scope,
                body: Vec::new(),
            }],
            body: Vec::new(),
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["LOCAL-FUNCTION"])?),
        Err(VerifyError::InvalidBindingScopes(wrong_definition_scope))
    );
    Ok(())
}
