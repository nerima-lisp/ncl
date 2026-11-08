//! Behavior checks for value and non-local control flow during HIR lowering.

use std::error::Error;

use ncl_hir::{
    CompilationUnitId, FormId, HirFormBuilder, Literal, NodeId, NodeKind, ScopeId, TagbodyTag,
    Terminator as HirTerminator,
};
use ncl_ir::{Constant, ConstantIndex, Function, HandlerKind, OpKind, Terminator, ValueId};

#[path = "lowering_control_paths/handlers.rs"]
mod handlers;

type TestResult<T> = Result<T, Box<dyn Error>>;

fn context(
    form_index: u32,
    source: &str,
) -> TestResult<(FormId, HirFormBuilder, ncl_hir::SymbolTable)> {
    let unit = CompilationUnitId::new(47);
    let form = FormId::new(unit, form_index);
    let mut files = ncl_hir::SourceFiles::new(unit);
    let file = files.add("lowering-control-paths.lisp", source)?;
    let span = files.span(file, 0, source.len())?;
    let mut origins = ncl_hir::SourceOrigins::new(form);
    let origin = origins.add_source(&files, file, span)?;
    let builder = HirFormBuilder::new(form, origins, origin)?;
    Ok((form, builder, ncl_hir::SymbolTable::new(unit)))
}

fn literal(builder: &mut HirFormBuilder, scope: ScopeId, value: Literal) -> TestResult<NodeId> {
    Ok(builder.alloc_node(scope, NodeKind::Literal(value))?)
}

fn returned_value(function: &Function) -> Option<ValueId> {
    function
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            Terminator::Return { values } if values.len() == 1 => values.first().copied(),
            _ => None,
        })
}

fn constant_at(function: &Function, index: ConstantIndex) -> Option<&Constant> {
    function.constants.get(usize::try_from(index.0).ok()?)
}

fn constant_for_value(function: &Function, value: ValueId) -> Option<&Constant> {
    let operation = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find(|operation| operation.results.iter().any(|(result, _)| *result == value))?;
    match &operation.kind {
        OpKind::Const { result } => constant_at(function, *result),
        OpKind::Convert { value, .. } | OpKind::Move { value } => {
            constant_for_value(function, *value)
        }
        _ => None,
    }
}

fn named_call_result(function: &Function, package: &str, name: &str) -> Option<ValueId> {
    named_call_result_with_mode(function, package, name, true)
}

fn named_call_result_with_mode(
    function: &Function,
    package: &str,
    name: &str,
    named_symbol: bool,
) -> Option<ValueId> {
    let arguments = named_call_arguments(function, package, name, named_symbol)?;
    let symbol = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| {
            let OpKind::Const { result } = &operation.kind else {
                return None;
            };
            matches!(constant_at(function, *result), Some(Constant::Symbol {
                package: actual_package,
                name: actual_name,
            }) if actual_package == package && actual_name == name)
            .then(|| operation.results.first().map(|(value, _)| *value))
            .flatten()
        })?;
    let closure = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| {
            matches!(&operation.kind, OpKind::LoadField { object, field: 1 } if *object == symbol)
                .then(|| operation.results.first().map(|(value, _)| *value))
                .flatten()
        })?;
    function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| match &operation.kind {
            OpKind::CallClosure {
                closure: actual_closure,
                args: actual_arguments,
                named_symbol: actual_symbol,
            } if *actual_closure == closure
                && actual_arguments.as_slice() == arguments
                && match (named_symbol, actual_symbol) {
                    (true, Some(actual_symbol)) => *actual_symbol == symbol,
                    (false, None) => true,
                    _ => false,
                } =>
            {
                operation.results.first().map(|(value, _)| *value)
            }
            _ => None,
        })
}

fn named_call_arguments<'a>(
    function: &'a Function,
    package: &str,
    name: &str,
    named_symbol: bool,
) -> Option<&'a [ValueId]> {
    let symbol = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| {
            let OpKind::Const { result } = &operation.kind else {
                return None;
            };
            matches!(constant_at(function, *result), Some(Constant::Symbol {
                package: actual_package,
                name: actual_name,
            }) if actual_package == package && actual_name == name)
            .then(|| operation.results.first().map(|(value, _)| *value))
            .flatten()
        })?;
    let closure = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| {
            matches!(&operation.kind, OpKind::LoadField { object, field: 1 } if *object == symbol)
                .then(|| operation.results.first().map(|(value, _)| *value))
                .flatten()
        })?;
    function.blocks.iter().find_map(|block| {
        block
            .ops
            .iter()
            .find_map(|operation| match &operation.kind {
                OpKind::CallClosure {
                    closure: actual_closure,
                    args,
                    named_symbol: actual_symbol,
                } if *actual_closure == closure
                    && match (named_symbol, actual_symbol) {
                        (true, Some(actual_symbol)) => *actual_symbol == symbol,
                        (false, None) => true,
                        _ => false,
                    } =>
                {
                    Some(args.as_slice())
                }
                _ => None,
            })
    })
}

#[test]
fn omitted_if_else_branch_merges_nil_with_the_then_value() -> TestResult<()> {
    let (_, mut builder, symbols) = context(0, "(if t 31)")?;
    let scope = builder.root_scope();
    let test = literal(&mut builder, scope, Literal::True)?;
    let then_value = literal(
        &mut builder,
        scope,
        Literal::Number(ncl_hir::NumberLiteral::Fixnum(31)),
    )?;
    let conditional = builder.alloc_node(
        scope,
        NodeKind::If {
            test,
            then_branch: then_value,
            else_branch: None,
        },
    )?;
    builder.add_root(conditional)?;

    let form = builder.finish()?;
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;
    let entry = lowered
        .entry
        .blocks
        .first()
        .ok_or_else(|| std::io::Error::other("entry block is missing"))?;
    let (then_target, else_target) = match &entry.terminator {
        Terminator::Branch {
            then_target,
            else_target,
            ..
        } => (*then_target, *else_target),
        _ => return Err(std::io::Error::other("IF branch is missing").into()),
    };
    let then_block = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == then_target)
        .ok_or_else(|| std::io::Error::other("then block is missing"))?;
    let else_block = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == else_target)
        .ok_or_else(|| std::io::Error::other("else block is missing"))?;
    let (then_value, then_merge) = match &then_block.terminator {
        Terminator::Jump { target, args } => {
            (*args.first().ok_or("then value is missing")?, *target)
        }
        _ => return Err(std::io::Error::other("then branch does not reach its merge").into()),
    };
    let (else_value, else_merge) = match &else_block.terminator {
        Terminator::Jump { target, args } => {
            (*args.first().ok_or("else value is missing")?, *target)
        }
        _ => return Err(std::io::Error::other("else branch does not reach its merge").into()),
    };
    assert_eq!(then_merge, else_merge);
    assert_eq!(
        constant_for_value(&lowered.entry, then_value),
        Some(&Constant::Fixnum(31))
    );
    assert_eq!(
        constant_for_value(&lowered.entry, else_value),
        Some(&Constant::Nil)
    );
    let returned = returned_value(&lowered.entry)
        .ok_or_else(|| std::io::Error::other("IF result is not returned"))?;
    let merge_value = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == then_merge)
        .and_then(|block| block.params.first())
        .map(|parameter| parameter.value);
    assert_eq!(merge_value, Some(returned));
    // CLHS IF yields NIL when its false branch is omitted.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_if.htm
    Ok(())
}

#[test]
fn block_return_without_a_value_restores_nil_as_the_function_result() -> TestResult<()> {
    let (form_id, mut builder, mut symbols) = context(2, "(block done (return-from done))")?;
    let scope = builder.root_scope();
    let name = symbols.intern(form_id, Some("COMMON-LISP-USER"), "DONE", None)?;
    builder.register_symbol(name)?;
    let target_index = builder
        .nodes_mut()
        .len()
        .checked_add(1)
        .ok_or_else(|| std::io::Error::other("HIR node index overflow"))?;
    let target = NodeId::try_from_index(form_id, target_index)?;
    let exit = builder.alloc_node(
        scope,
        NodeKind::ReturnFrom {
            target,
            value: None,
        },
    )?;
    let block = builder.alloc_node(
        scope,
        NodeKind::Block {
            name,
            body: vec![exit],
        },
    )?;
    assert_eq!(block, target);
    builder.add_root(block)?;

    let form = builder.finish()?;
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;
    let returned = returned_value(&lowered.entry)
        .ok_or_else(|| std::io::Error::other("block result is not returned"))?;
    let values_list_result = named_call_result(&lowered.entry, "COMMON-LISP", "VALUES-LIST")
        .ok_or_else(|| std::io::Error::other("multiple-value result restoration is missing"))?;
    assert_eq!(returned, values_list_result);
    let capture_args =
        named_call_arguments(&lowered.entry, "NCL-EXT", "CAPTURE-MULTIPLE-VALUES", false)
            .ok_or_else(|| std::io::Error::other("block result capture call is missing"))?;
    assert_eq!(
        capture_args
            .get(1)
            .and_then(|value| constant_for_value(&lowered.entry, *value)),
        Some(&Constant::Nil)
    );
    // CLHS RETURN-FROM without a form returns no values, whose primary value is NIL.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_block.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/05_b.htm
    Ok(())
}

#[test]
fn go_reaches_its_tagbody_block_and_tagbody_returns_nil() -> TestResult<()> {
    let (form_id, mut builder, mut symbols) = context(3, "(tagbody (go target) target)")?;
    let scope = builder.root_scope();
    let tag_name = symbols.intern(form_id, Some("COMMON-LISP-USER"), "TARGET", None)?;
    builder.register_symbol(tag_name)?;
    let tagbody = builder.alloc_node(
        scope,
        NodeKind::Tagbody {
            tags: Vec::new(),
            forms: Vec::new(),
        },
    )?;
    let flow = builder.control_flow_mut();
    flow.begin_function(tagbody)?;
    let entry = flow.create_block(tagbody)?;
    let target = flow.create_block(tagbody)?;
    let go = builder.alloc_node(scope, NodeKind::Go { target })?;
    let tagbody_node = builder
        .nodes_mut()
        .get_mut(tagbody)
        .ok_or_else(|| std::io::Error::other("TAGBODY node is missing"))?;
    let NodeKind::Tagbody { tags, forms } = &mut tagbody_node.kind else {
        return Err(std::io::Error::other("TAGBODY node changed kind").into());
    };
    tags.push(TagbodyTag {
        name: tag_name,
        block: target,
    });
    forms.push(go);
    let flow = builder.control_flow_mut();
    flow.append_operation(entry, go)?;
    flow.set_terminator(entry, HirTerminator::Jump(target))?;
    flow.set_terminator(target, HirTerminator::Return { values: Vec::new() })?;
    builder.add_root(tagbody)?;

    let form = builder.finish()?;
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;
    let go_block = lowered
        .entry
        .blocks
        .iter()
        .find(|block| {
            block.ops.iter().any(|operation| {
                operation.results.first().is_some_and(|(value, _)| {
                    constant_for_value(&lowered.entry, *value) == Some(&Constant::Nil)
                })
            }) && matches!(&block.terminator, Terminator::Jump { .. })
        })
        .ok_or_else(|| std::io::Error::other("GO transfer block is missing"))?;
    let go_target = match &go_block.terminator {
        Terminator::Jump { target, .. } => *target,
        _ => return Err(std::io::Error::other("GO does not jump to its tag").into()),
    };
    let target_block = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == go_target)
        .ok_or_else(|| std::io::Error::other("GO tag block is missing"))?;
    assert!(matches!(&target_block.terminator, Terminator::Jump { .. }));
    let returned = returned_value(&lowered.entry)
        .ok_or_else(|| std::io::Error::other("TAGBODY result is not returned"))?;
    assert_eq!(
        constant_for_value(&lowered.entry, returned),
        Some(&Constant::Nil)
    );
    // CLHS GO transfers control to its tag; TAGBODY returns NIL when it completes.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_go.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_tagbod.htm
    Ok(())
}

#[test]
fn labels_recursion_calls_through_its_captured_function_cell() -> TestResult<()> {
    let (form_id, mut builder, mut symbols) = context(5, "(labels ((self () (self))) (self))")?;
    let scope = builder.root_scope();
    let name = symbols.intern(form_id, Some("COMMON-LISP-USER"), "SELF", None)?;
    builder.register_symbol(name)?;
    let group = builder.bindings_mut().bind_local_functions(
        scope,
        ncl_hir::binding::LocalFunctionKind::Labels,
        &[name],
    )?;
    let definition = group
        .definitions
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("LABELS definition is missing"))?;
    let recursive_call = builder.alloc_node(
        group.body_scope,
        NodeKind::Call {
            target: ncl_hir::CallTarget::Local(definition.binding),
            arguments: Vec::new(),
        },
    )?;
    builder.bindings_mut().record_use(
        definition.binding,
        recursive_call,
        ncl_hir::binding::BindingUseKind::Read,
    )?;
    let body_call = builder.alloc_node(
        group.body_scope,
        NodeKind::Call {
            target: ncl_hir::CallTarget::Local(definition.binding),
            arguments: Vec::new(),
        },
    )?;
    builder.bindings_mut().record_use(
        definition.binding,
        body_call,
        ncl_hir::binding::BindingUseKind::Read,
    )?;
    let local_functions = builder.alloc_node(
        scope,
        NodeKind::LocalFunctions {
            kind: ncl_hir::binding::LocalFunctionKind::Labels,
            body_scope: group.body_scope,
            definitions: vec![ncl_hir::LocalFunctionDefinition {
                binding: definition.binding,
                definition_scope: definition.definition_scope,
                parameters: Vec::new(),
                body_scope: group.body_scope,
                body: vec![recursive_call],
            }],
            body: vec![body_call],
        },
    )?;
    builder.add_root(local_functions)?;

    let form = builder.finish()?;
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;
    let nested = lowered
        .nested
        .first()
        .ok_or_else(|| std::io::Error::other("LABELS function was not lowered"))?;
    let captured_cell = nested
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| match &operation.kind {
            OpKind::LoadCapture { index: 0 } => operation.results.first().map(|(value, _)| *value),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("recursive function cell was not captured"))?;
    // Internal invariant: a LABELS recursive capture loads the function from value-cell field zero.
    let captured_function = nested
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| match &operation.kind {
            OpKind::LoadField { object, field: 0 } if *object == captured_cell => {
                operation.results.first().map(|(value, _)| *value)
            }
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("recursive function was not loaded from its cell"))?;
    assert!(nested.blocks.iter().flat_map(|block| &block.ops).any(|operation| {
        matches!(&operation.kind, OpKind::CallClosure { closure, .. } if *closure == captured_function)
    }));
    // CLHS LABELS definitions can refer to themselves and to each other.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_flet_.htm
    Ok(())
}
