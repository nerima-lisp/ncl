//! Value-sensitive checks for lexical binding and call-form lowering.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ncl_compiler_front::{Expr, LambdaExpr, LambdaList, LetBinding, Literal, SymbolRef};
use ncl_ir::{BlockId, Constant, Function, HandlerKind, OpKind, Terminator, ValueId, verify};

fn symbol(name: &str) -> SymbolRef {
    SymbolRef::interned("COMMON-LISP-USER", name)
}

fn assert_verifies(function: &Function) {
    if let Err(errors) = verify(function) {
        panic!("{} does not verify: {errors:?}\n{function}", function.name);
    }
}

fn constant_for_value(function: &Function, value: ValueId) -> Option<&Constant> {
    let mut current = value;
    for _ in 0..function
        .blocks
        .iter()
        .map(|block| block.ops.len())
        .sum::<usize>()
    {
        let op = function
            .blocks
            .iter()
            .flat_map(|block| block.ops.iter())
            .find(|op| op.results.iter().any(|(result, _)| *result == current))?;
        match &op.kind {
            OpKind::Const { result } => return function.constants.get(result.0 as usize),
            OpKind::Convert { value, .. } | OpKind::Move { value } => current = *value,
            _ => return None,
        }
    }
    None
}

fn ops(function: &Function) -> impl Iterator<Item = &ncl_ir::Op> {
    function.blocks.iter().flat_map(|block| block.ops.iter())
}

#[test]
fn setq_preserves_lexical_values_and_writes_global_symbol_values() {
    let lexical = symbol("LEXICAL");
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: lexical.clone(),
            value: Some(Expr::Constant(Literal::fixnum(17))),
        }],
        declarations: Vec::new(),
        body: vec![
            Expr::Setq(vec![(lexical, Expr::Constant(Literal::fixnum(23)))]),
            Expr::Setq(vec![(
                symbol("GLOBAL"),
                Expr::Constant(Literal::fixnum(29)),
            )]),
        ],
    };
    let lowered = ncl_compiler_front::lower_toplevel(&expression).expect("setq lowers");

    assert_verifies(&lowered.entry);
    let numeric_constants = ops(&lowered.entry)
        .filter_map(|op| match op.kind {
            OpKind::Const { result } => lowered.entry.constants.get(result.0 as usize),
            _ => None,
        })
        .filter(|constant| matches!(constant, Constant::Fixnum(_)))
        .collect::<Vec<_>>();
    assert_eq!(
        numeric_constants,
        vec![
            &Constant::Fixnum(17),
            &Constant::Fixnum(23),
            &Constant::Fixnum(29)
        ]
    );
    let stores = ops(&lowered.entry)
        .filter_map(|op| match op.kind {
            OpKind::StoreField {
                object,
                field,
                value,
            } => Some((object, field, value)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        stores.len(),
        1,
        "only the global SETQ writes a symbol value cell"
    );
    let store = stores.first().expect("the global SETQ store is present");
    assert_eq!(
        store.1,
        u32::try_from(ncl_object::symbol_offset::VALUE).unwrap()
    );
    assert_eq!(
        constant_for_value(&lowered.entry, store.2),
        Some(&Constant::Fixnum(29))
    );
    assert!(ops(&lowered.entry).all(|op| {
        !matches!(op.kind, OpKind::StoreField { value, .. }
            if constant_for_value(&lowered.entry, value) == Some(&Constant::Fixnum(23)))
    }));
    assert_eq!(
        lowered
            .entry
            .blocks
            .last()
            .expect("the lowered function has a final block")
            .terminator,
        Terminator::Return {
            values: vec![store.2]
        }
    );
}

#[test]
fn progv_restores_on_a_nonlocal_exit_and_returns_nil_from_its_handler() {
    let name = symbol("DONE");
    let expression = Expr::Block {
        name: name.clone(),
        body: vec![Expr::Progv {
            symbols: Box::new(Expr::Constant(Literal::Nil)),
            values: Box::new(Expr::Constant(Literal::Nil)),
            body: vec![Expr::ReturnFrom {
                name,
                value: Some(Box::new(Expr::Constant(Literal::fixnum(31)))),
            }],
        }],
    };
    let lowered = ncl_compiler_front::lower_toplevel(&expression).expect("progv exit lowers");

    assert_verifies(&lowered.entry);
    assert_eq!(lowered.entry.handler_regions.len(), 1);
    let region = lowered
        .entry
        .handler_regions
        .first()
        .expect("the progv handler region is present");
    assert_eq!(region.kind, HandlerKind::Progv);
    assert_eq!(region.protected, vec![BlockId(0), BlockId(1)]);
    assert_eq!(region.handler, BlockId(3));
    assert_eq!(region.cleanup, None);
    assert_eq!(region.catch_tag, None);
    assert_eq!(region.binding_targets.len(), 2);

    let protected_id = region
        .protected
        .first()
        .copied()
        .expect("the progv protected block is present");
    let protected = lowered
        .entry
        .blocks
        .get(protected_id.0 as usize)
        .expect("the progv protected block is addressable");
    let enters = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| block.ops.iter())
        .filter_map(|op| match op.kind {
            OpKind::EnterHandler { region } => Some(region),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(enters, vec![region.id]);
    let leaves = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| block.ops.iter())
        .filter_map(|op| match op.kind {
            OpKind::LeaveHandler { region } => Some(region),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(leaves, vec![region.id, region.id]);
    let Terminator::Jump { args, .. } = &protected.terminator else {
        panic!("progv protected path must jump to the enclosing block");
    };
    assert_eq!(args.len(), 1);
    let jump_value = args.first().expect("the protected jump carries its value");
    assert_eq!(
        constant_for_value(&lowered.entry, *jump_value),
        Some(&Constant::Fixnum(31))
    );

    let handler = lowered
        .entry
        .blocks
        .get(region.handler.0 as usize)
        .expect("the progv handler block is addressable");
    let Terminator::Return { values } = &handler.terminator else {
        panic!("progv handler must return the restored NIL value");
    };
    assert_eq!(values.len(), 1);
    let handler_value = values
        .first()
        .expect("the handler returns its restored value");
    assert_eq!(
        constant_for_value(&lowered.entry, *handler_value),
        Some(&Constant::Nil)
    );
    let symbols_target = region
        .binding_targets
        .first()
        .expect("the symbols binding target is present");
    let values_target = region
        .binding_targets
        .get(1)
        .expect("the values binding target is present");
    assert_eq!(
        constant_for_value(&lowered.entry, *symbols_target),
        Some(&Constant::Nil)
    );
    assert_eq!(
        constant_for_value(&lowered.entry, *values_target),
        Some(&Constant::Nil)
    );
}

#[test]
fn progv_normal_exit_leaves_the_handler_and_returns_the_body_value() {
    let expression = Expr::Progv {
        symbols: Box::new(Expr::Constant(Literal::Nil)),
        values: Box::new(Expr::Constant(Literal::Nil)),
        body: vec![Expr::Constant(Literal::fixnum(37))],
    };
    let lowered = ncl_compiler_front::lower_toplevel(&expression).expect("progv lowers");

    assert_verifies(&lowered.entry);
    let region = lowered
        .entry
        .handler_regions
        .first()
        .expect("the progv handler region is present");
    assert_eq!(region.kind, HandlerKind::Progv);
    assert_eq!(region.protected, vec![BlockId(0)]);
    assert_eq!(region.cleanup, None);
    let leave_count = ops(&lowered.entry)
        .filter(|op| matches!(op.kind, OpKind::LeaveHandler { region: id } if id == region.id))
        .count();
    assert_eq!(leave_count, 2);
    assert!(lowered.entry.blocks.iter().any(|block| {
        matches!(&block.terminator, Terminator::Jump { args, .. }
        if args.first().is_some_and(|value| {
            constant_for_value(&lowered.entry, *value) == Some(&Constant::Fixnum(37))
        }))
    }));
}

#[test]
fn captured_setq_writes_through_the_value_cell() {
    let name = symbol("CAPTURED");
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: name.clone(),
            value: Some(Expr::Constant(Literal::fixnum(5))),
        }],
        declarations: Vec::new(),
        body: vec![Expr::Lambda(Box::new(LambdaExpr {
            lambda_list: LambdaList::new(),
            declarations: Vec::new(),
            docstring: None,
            body: vec![Expr::Setq(vec![(
                name,
                Expr::Constant(Literal::fixnum(41)),
            )])],
        }))],
    };
    let lowered = ncl_compiler_front::lower_toplevel(&expression).expect("captured setq lowers");

    assert_verifies(&lowered.entry);
    let nested = lowered.nested.first().expect("captured lambda is lowered");
    assert_verifies(nested);
    assert!(ops(nested).any(|op| {
        matches!(op.kind, OpKind::StoreField { field: 0, value, .. }
            if constant_for_value(nested, value) == Some(&Constant::Fixnum(41)))
    }));
}
