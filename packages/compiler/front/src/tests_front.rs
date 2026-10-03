#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::{
    Declaration, Expr, FrontError, KeyParam, LambdaExpr, LambdaList, LambdaListKind, LetBinding,
    Literal, NumberLiteral, Operator, OptionalParam, ParamName, Quality, SymbolRef, TypeSpecifier,
    lower_toplevel,
};
use ncl_ir::{Constant, Function, OpKind, Terminator, verify};
use ncl_object::{Package, Runtime, ThreadContext, Word, make_cons};

struct Fixture {
    ctx: ThreadContext,
    runtime: Runtime,
}

impl Fixture {
    fn new() -> Self {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("context");
        Self { ctx, runtime }
    }

    fn intern(&mut self, package: &str, name: &str) -> Word {
        let package = self
            .runtime
            .ensure_package(&mut self.ctx, package)
            .expect("package");
        Package::from_word(package)
            .intern(&mut self.ctx, &self.runtime, name)
            .expect("symbol")
            .0
    }

    fn cl(&mut self, name: &str) -> Word {
        self.intern("COMMON-LISP", name)
    }

    fn user(&mut self, name: &str) -> Word {
        self.intern("COMMON-LISP-USER", name)
    }

    fn keyword(&mut self, name: &str) -> Word {
        self.intern("KEYWORD", name)
    }

    fn list(&mut self, words: &[Word]) -> Word {
        words.iter().rev().fold(Word::NIL, |tail, word| {
            make_cons(&mut self.ctx, &self.runtime, *word, tail).expect("cons")
        })
    }

    fn lambda_list(
        &mut self,
        words: &[Word],
        kind: LambdaListKind,
    ) -> Result<LambdaList, FrontError> {
        let registry = super::MacroRegistry::new();
        let form = self.list(words);
        let mut expander = super::FormExpander::new(&mut self.ctx, &self.runtime, &registry);
        expander.expand_lambda_list(form, kind)
    }
}

fn symbol(name: &str) -> SymbolRef {
    SymbolRef::interned("COMMON-LISP-USER", name)
}

fn verifies(function: &Function) {
    assert!(verify(function).is_ok(), "invalid IR: {function}");
}

fn has_op(function: &Function, predicate: impl Fn(&OpKind) -> bool) -> bool {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .any(|op| predicate(&op.kind))
}

#[test]
fn lambda_lists_cover_all_parameter_sections_and_errors() {
    let mut fixture = Fixture::new();
    let required = fixture.user("REQUIRED");
    let optional = fixture.cl("&OPTIONAL");
    let optional_name = fixture.user("OPTIONAL");
    let supplied = fixture.user("P");
    let optional_spec = fixture.list(&[optional_name, Word::fixnum(1), supplied]);
    let rest = fixture.cl("&REST");
    let rest_name = fixture.user("REST");
    let key = fixture.cl("&KEY");
    let keyword = fixture.keyword("VALUE");
    let key_name = fixture.user("VALUE");
    let key_spec = fixture.list(&[keyword, key_name]);
    let allow = fixture.cl("&ALLOW-OTHER-KEYS");
    let aux = fixture.cl("&AUX");
    let aux_name = fixture.user("AUX");
    let aux_spec = fixture.list(&[aux_name, Word::fixnum(2)]);
    let parsed = fixture
        .lambda_list(
            &[
                required,
                optional,
                optional_spec,
                rest,
                rest_name,
                key,
                key_spec,
                allow,
                aux,
                aux_spec,
            ],
            LambdaListKind::Ordinary,
        )
        .expect("lambda list");
    assert_eq!(parsed.required.len(), 1);
    assert_eq!(parsed.optional.len(), 1);
    assert_eq!(parsed.keys.len(), 1);
    assert!(parsed.rest.is_some());
    assert!(parsed.allow_other_keys);
    assert_eq!(parsed.aux.len(), 1);
    assert!(parsed.accepts_keywords());
    assert_eq!(parsed.keyword_names()[0].name, "VALUE");

    let body = fixture.cl("&BODY");
    let body_name = fixture.user("BODY");
    let error = fixture.lambda_list(&[body, body_name], LambdaListKind::Ordinary);
    assert!(matches!(
        error,
        Err(FrontError::UnknownLambdaListKeyword { .. })
    ));
    let duplicate = fixture.lambda_list(&[optional, optional], LambdaListKind::Ordinary);
    assert!(matches!(
        duplicate,
        Err(FrontError::DuplicateLambdaListKeyword { .. })
    ));
}

#[test]
fn declarations_parse_values_and_retain_unknown_specifiers() {
    let x = symbol("X");
    let type_name = symbol("TYPE");
    let declaration = Literal::Cons(
        Box::new(Literal::Symbol(symbol("DECLARE"))),
        Box::new(Literal::Cons(
            Box::new(Literal::Cons(
                Box::new(Literal::Symbol(symbol("SPECIAL"))),
                Box::new(Literal::Cons(
                    Box::new(Literal::Symbol(x.clone())),
                    Box::new(Literal::Nil),
                )),
            )),
            Box::new(Literal::Cons(
                Box::new(Literal::Cons(
                    Box::new(Literal::Symbol(symbol("OPTIMIZE"))),
                    Box::new(Literal::Cons(
                        Box::new(Literal::Cons(
                            Box::new(Literal::Symbol(symbol("SPEED"))),
                            Box::new(Literal::Cons(
                                Box::new(Literal::fixnum(3)),
                                Box::new(Literal::Nil),
                            )),
                        )),
                        Box::new(Literal::Nil),
                    )),
                )),
                Box::new(Literal::Cons(
                    Box::new(Literal::Cons(
                        Box::new(Literal::Symbol(symbol("MYSTERY"))),
                        Box::new(Literal::Cons(
                            Box::new(Literal::fixnum(9)),
                            Box::new(Literal::Nil),
                        )),
                    )),
                    Box::new(Literal::Nil),
                )),
            )),
        )),
    );
    let parsed = super::parse_declare_form(&declaration).expect("declarations");
    assert!(matches!(parsed[0], Declaration::Special(ref names) if names == &[x]));
    assert!(
        matches!(parsed[1], Declaration::Optimize(ref qualities) if qualities[0].quality == Quality::Speed && qualities[0].value == 3)
    );
    assert!(matches!(parsed[2], Declaration::Unknown { ref name, .. } if name.name == "MYSTERY"));

    let type_declaration = Literal::Cons(
        Box::new(Literal::Symbol(symbol("TYPE"))),
        Box::new(Literal::Cons(
            Box::new(Literal::Symbol(type_name)),
            Box::new(Literal::Cons(
                Box::new(Literal::Symbol(symbol("Y"))),
                Box::new(Literal::Nil),
            )),
        )),
    );
    let parsed = Declaration::parse(&type_declaration).expect("type declaration");
    assert!(matches!(parsed, Declaration::Type { names, .. } if names[0].name == "Y"));
}

#[test]
fn lowering_constants_and_control_flow_preserves_values() {
    let quoted = Expr::Constant(Literal::Vector(vec![
        Literal::String("λ".chars().collect()),
        Literal::Number(NumberLiteral::Ratio {
            numerator: Box::new(NumberLiteral::Fixnum(2)),
            denominator: Box::new(NumberLiteral::Bignum {
                negative: false,
                limbs: vec![3],
            }),
        }),
    ]));
    let lowered = lower_toplevel(&quoted).expect("vector literal");
    verifies(&lowered.entry);
    assert!(
        lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Structure { .. }))
    );
    assert!(lowered.entry.constants.iter().any(
        |constant| matches!(constant, Constant::StringBytes(bytes) if bytes == &[0xce, 0xbb])
    ));

    let x = symbol("X");
    let control = Expr::Let {
        sequential: true,
        bindings: vec![LetBinding {
            name: x.clone(),
            value: Some(Expr::Constant(Literal::fixnum(1))),
        }],
        declarations: vec![],
        body: vec![Expr::If {
            test: Box::new(Expr::Variable(x.clone())),
            then: Box::new(Expr::Setq(vec![(
                x.clone(),
                Expr::Constant(Literal::fixnum(2)),
            )])),
            otherwise: Some(Box::new(Expr::Constant(Literal::Nil))),
        }],
    };
    let lowered = lower_toplevel(&control).expect("control flow");
    verifies(&lowered.entry);
    assert!(has_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::Compare { .. }
    )));
    assert!(
        lowered
            .entry
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, Terminator::Branch { .. }))
    );
    assert!(
        lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(2)))
    );
}

#[test]
fn lowering_lambda_parameters_and_dynamic_forms_emits_observable_ir() {
    let argument = symbol("ARG");
    let optional = symbol("OPTIONAL");
    let lambda = Expr::Lambda(Box::new(LambdaExpr {
        lambda_list: LambdaList {
            required: vec![ParamName::Symbol(argument.clone())],
            optional: vec![OptionalParam {
                name: ParamName::Symbol(optional.clone()),
                default: Some(Expr::Constant(Literal::fixnum(7))),
                supplied_p: Some(ParamName::Symbol(symbol("SUPPLIED-P"))),
            }],
            keys: vec![KeyParam {
                keyword: SymbolRef::interned("KEYWORD", "VALUE"),
                name: ParamName::Symbol(symbol("VALUE")),
                default: None,
                supplied_p: Some(ParamName::Symbol(symbol("VALUE-P"))),
            }],
            allow_other_keys: true,
            ..LambdaList::new()
        },
        declarations: vec![],
        docstring: Some("doc".to_owned()),
        body: vec![Expr::Progn(vec![
            Expr::Variable(argument),
            Expr::Variable(optional),
        ])],
    }));
    let lowered = lower_toplevel(&Expr::Call {
        operator: Operator::Lambda(Box::new(match lambda {
            Expr::Lambda(value) => *value,
            _ => unreachable!(),
        })),
        arguments: vec![Expr::Constant(Literal::fixnum(3))],
    })
    .expect("lambda");
    verifies(&lowered.entry);
    verifies(&lowered.nested[0]);
    assert!(has_op(
        &lowered.nested[0],
        |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "check-keywords")
    ));
    assert!(has_op(
        &lowered.nested[0],
        |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "keyword-supplied-p")
    ));

    let dynamic = Expr::Catch {
        tag: Box::new(Expr::Constant(Literal::Symbol(symbol("TAG")))),
        body: vec![Expr::UnwindProtect {
            protected: Box::new(Expr::Throw {
                tag: Box::new(Expr::Constant(Literal::Symbol(symbol("TAG")))),
                value: Box::new(Expr::Constant(Literal::fixnum(4))),
            }),
            cleanup: vec![Expr::Constant(Literal::fixnum(5))],
        }],
    };
    let lowered = lower_toplevel(&dynamic).expect("dynamic control");
    verifies(&lowered.entry);
    assert_eq!(lowered.entry.handler_regions.len(), 2);
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == ncl_ir::HandlerKind::Catch)
    );
}

#[test]
fn type_specifier_and_literal_helpers_return_their_values() {
    let type_specifier = TypeSpecifier::new(Literal::Symbol(symbol("INTEGER")));
    assert_eq!(type_specifier.form(), &Literal::Symbol(symbol("INTEGER")));
    assert_eq!(
        type_specifier.into_form(),
        Literal::Symbol(symbol("INTEGER"))
    );
    assert!(Literal::fixnum(1).is_self_evaluating());
    assert!(!Literal::Symbol(symbol("X")).is_self_evaluating());
    assert!(
        Literal::Cons(Box::new(Literal::fixnum(1)), Box::new(Literal::Nil)).is_self_evaluating()
            == false
    );
}
