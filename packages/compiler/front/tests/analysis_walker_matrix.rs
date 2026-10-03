//! Table-driven coverage for the recursive non-local-control walkers.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ncl_compiler_front::{
    Expr, FunctionDesignator, LambdaExpr, LambdaList, LetBinding, Literal, LocalFunction,
    Operator, SymbolRef, TagbodyItem, lower_toplevel,
};
use ncl_ir::{Function, HandlerKind, Terminator, verify};

fn symbol(name: &str) -> SymbolRef {
    SymbolRef::interned("COMMON-LISP-USER", name)
}

const fn lambda(body: Vec<Expr>) -> LambdaExpr {
    LambdaExpr {
        lambda_list: LambdaList::new(),
        declarations: Vec::new(),
        docstring: None,
        body,
    }
}

fn return_from(name: &SymbolRef) -> Expr {
    Expr::ReturnFrom {
        name: name.clone(),
        value: Some(Box::new(Expr::Constant(Literal::fixnum(7)))),
    }
}

fn handler_kinds(function: &Function) -> Vec<HandlerKind> {
    function
        .handler_regions
        .iter()
        .map(|region| region.kind)
        .collect()
}

fn assert_verifies(function: &Function, label: &str) {
    assert_eq!(verify(function), Ok(()), "{label} ({}) must verify", function.name);
}

#[allow(clippy::too_many_lines)]
fn go_wrapper_cases(tag: &SymbolRef) -> Vec<(&'static str, Expr)> {
    let go = || Expr::Go { tag: tag.clone() };
    let local = |name: &str| LocalFunction {
        name: symbol(name),
        lambda: lambda(vec![go()]),
    };
    vec![
        ("lambda", Expr::Lambda(Box::new(lambda(vec![go()])))),
        (
            "function-lambda",
            Expr::Function(FunctionDesignator::Lambda(Box::new(lambda(vec![go()])))),
        ),
        (
            "call-lambda-operator",
            Expr::Call {
                operator: Operator::Lambda(Box::new(lambda(vec![go()]))),
                arguments: Vec::new(),
            },
        ),
        ("progn", Expr::Progn(vec![go()])),
        (
            "locally",
            Expr::Locally {
                declarations: Vec::new(),
                body: vec![go()],
            },
        ),
        (
            "macrolet",
            Expr::Macrolet {
                definitions: Vec::new(),
                declarations: Vec::new(),
                body: vec![go()],
            },
        ),
        (
            "symbol-macrolet",
            Expr::SymbolMacrolet {
                definitions: Vec::new(),
                declarations: Vec::new(),
                body: vec![go()],
            },
        ),
        (
            "let",
            Expr::Let {
                sequential: false,
                bindings: vec![LetBinding {
                    name: symbol("VALUE"),
                    value: Some(go()),
                }],
                declarations: Vec::new(),
                body: vec![Expr::Constant(Literal::Nil)],
            },
        ),
        ("setq", Expr::Setq(vec![(symbol("VALUE"), go())])),
        (
            "if",
            Expr::If {
                test: Box::new(go()),
                then: Box::new(Expr::Constant(Literal::Nil)),
                otherwise: Some(Box::new(go())),
            },
        ),
        (
            "block",
            Expr::Block {
                name: symbol("INNER"),
                body: vec![go()],
            },
        ),
        (
            "tagbody",
            Expr::Tagbody(vec![
                TagbodyItem::Tag(symbol("INNER")),
                TagbodyItem::Form(go()),
            ]),
        ),
        (
            "catch",
            Expr::Catch {
                tag: Box::new(go()),
                body: vec![Expr::Constant(Literal::Nil)],
            },
        ),
        (
            "unwind-protect",
            Expr::UnwindProtect {
                protected: Box::new(go()),
                cleanup: vec![Expr::Constant(Literal::Nil)],
            },
        ),
        (
            "progv",
            Expr::Progv {
                symbols: Box::new(go()),
                values: Box::new(Expr::Constant(Literal::Nil)),
                body: vec![Expr::Constant(Literal::Nil)],
            },
        ),
        (
            "flet-definition",
            Expr::Flet {
                definitions: vec![local("LOCAL")],
                declarations: Vec::new(),
                body: vec![Expr::Constant(Literal::Nil)],
            },
        ),
        (
            "labels-definition",
            Expr::Labels {
                definitions: vec![local("RECURSIVE")],
                declarations: Vec::new(),
                body: vec![Expr::Constant(Literal::Nil)],
            },
        ),
    ]
}

#[allow(clippy::too_many_lines)]
fn return_unwind_wrapper_cases(name: &SymbolRef) -> Vec<(&'static str, Expr)> {
    let payload = || Expr::UnwindProtect {
        protected: Box::new(return_from(name)),
        cleanup: vec![Expr::Constant(Literal::T)],
    };
    let local = |function_name: &str| LocalFunction {
        name: symbol(function_name),
        lambda: lambda(vec![payload()]),
    };
    vec![
        ("progn", payload()),
        (
            "locally",
            Expr::Locally {
                declarations: Vec::new(),
                body: vec![payload()],
            },
        ),
        (
            "macrolet",
            Expr::Macrolet {
                definitions: Vec::new(),
                declarations: Vec::new(),
                body: vec![payload()],
            },
        ),
        (
            "symbol-macrolet",
            Expr::SymbolMacrolet {
                definitions: Vec::new(),
                declarations: Vec::new(),
                body: vec![payload()],
            },
        ),
        (
            "let",
            Expr::Let {
                sequential: false,
                bindings: vec![LetBinding {
                    name: symbol("VALUE"),
                    value: Some(payload()),
                }],
                declarations: Vec::new(),
                body: vec![Expr::Constant(Literal::Nil)],
            },
        ),
        ("setq", Expr::Setq(vec![(symbol("VALUE"), payload())])),
        (
            "if",
            Expr::If {
                test: Box::new(Expr::Constant(Literal::T)),
                then: Box::new(Expr::Constant(Literal::Nil)),
                otherwise: Some(Box::new(payload())),
            },
        ),
        (
            "block",
            Expr::Block {
                name: symbol("INNER"),
                body: vec![payload()],
            },
        ),
        (
            "tagbody",
            Expr::Tagbody(vec![
                TagbodyItem::Tag(symbol("INNER")),
                TagbodyItem::Form(payload()),
            ]),
        ),
        (
            "catch",
            Expr::Catch {
                tag: Box::new(payload()),
                body: vec![Expr::Constant(Literal::Nil)],
            },
        ),
        (
            "unwind-protect",
            Expr::UnwindProtect {
                protected: Box::new(payload()),
                cleanup: vec![Expr::Constant(Literal::Nil)],
            },
        ),
        (
            "progv",
            Expr::Progv {
                symbols: Box::new(payload()),
                values: Box::new(Expr::Constant(Literal::Nil)),
                body: vec![Expr::Constant(Literal::Nil)],
            },
        ),
        (
            "call-argument",
            Expr::Call {
                operator: Operator::Name(symbol("LIST")),
                arguments: vec![payload()],
            },
        ),
        (
            "flet-body",
            Expr::Flet {
                definitions: vec![local("LOCAL")],
                declarations: Vec::new(),
                body: vec![payload()],
            },
        ),
        (
            "labels-body",
            Expr::Labels {
                definitions: vec![local("RECURSIVE")],
                declarations: Vec::new(),
                body: vec![payload()],
            },
        ),
    ]
}

#[test]
fn contains_go_walks_the_ast_wrapper_matrix() {
    let tag = symbol("RETRY");
    for (label, wrapper) in go_wrapper_cases(&tag) {
        let lowered = lower_toplevel(&Expr::Tagbody(vec![
            TagbodyItem::Form(Expr::Lambda(Box::new(lambda(vec![wrapper])))),
            TagbodyItem::Tag(tag.clone()),
            TagbodyItem::Form(Expr::Constant(Literal::Nil)),
        ]))
        .unwrap_or_else(|error| panic!("{label}: {error}"));
        assert_verifies(&lowered.entry, label);
        assert_eq!(handler_kinds(&lowered.entry), vec![HandlerKind::Catch], "{label}");
    }
}

#[test]
fn return_and_unwind_walkers_preserve_their_handler_values() {
    let name = symbol("EXIT");
    for (label, wrapper) in return_unwind_wrapper_cases(&name) {
        let lowered = lower_toplevel(&Expr::Block {
            name: name.clone(),
            body: vec![wrapper],
        })
        .unwrap_or_else(|error| panic!("{label}: {error}"));
        assert_verifies(&lowered.entry, label);
        let kinds = handler_kinds(&lowered.entry);
        assert!(kinds.contains(&HandlerKind::UnwindProtect), "{label}");
        assert!(kinds.contains(&HandlerKind::Catch), "{label}");
        assert!(lowered.entry.blocks.iter().any(|block| {
            matches!(block.terminator, Terminator::Throw { .. })
        }), "{label} must throw the escaping return");
    }
}
