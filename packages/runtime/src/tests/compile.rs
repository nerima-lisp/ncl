use super::{decode_payload, encode_payload, is_fasl, optimization_level, read_forms};
use ncl_compiler_front::ast::{
    Expr, FunctionDesignator, LambdaExpr, LetBinding, LocalFunction, LocalMacro, Operator,
    SymbolMacro, TagbodyItem,
};
use ncl_compiler_front::lambda_list::LambdaList;
use ncl_compiler_front::literal::Literal;
use ncl_compiler_front::{Declaration, OptimizeQuality, Quality, SymbolRef, TypeSpecifier};
use ncl_object::{Runtime as ObjectRuntime, ThreadContext};

fn optimize(quality: Quality, value: u8) -> Declaration {
    Declaration::Optimize(vec![OptimizeQuality { quality, value }])
}

fn probe() -> Expr {
    Expr::Locally {
        declarations: vec![optimize(Quality::Debug, 3)],
        body: Vec::new(),
    }
}

#[test]
fn optimization_level_follows_nested_declarations_and_safe_fallbacks() {
    let speed = Declaration::Optimize(vec![OptimizeQuality {
        quality: Quality::Speed,
        value: 1,
    }]);
    let debug = Declaration::Optimize(vec![OptimizeQuality {
        quality: Quality::Debug,
        value: 2,
    }]);
    let safety = Declaration::Optimize(vec![OptimizeQuality {
        quality: Quality::Safety,
        value: 1,
    }]);
    let unsupported = Declaration::Optimize(vec![OptimizeQuality {
        quality: Quality::Space,
        value: 3,
    }]);

    let nested = Expr::Locally {
        declarations: vec![speed, safety, debug],
        body: Vec::new(),
    };
    assert_eq!(optimization_level(&nested), super::OptimizationLevel::Debug);

    let unsafe_request = Expr::Progn(vec![
        Expr::Locally {
            declarations: vec![unsupported],
            body: vec![Expr::Constant(ncl_compiler_front::literal::Literal::Nil)],
        },
        Expr::Constant(ncl_compiler_front::literal::Literal::Nil),
    ]);
    assert_eq!(
        optimization_level(&unsafe_request),
        super::OptimizationLevel::Safety
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn optimization_level_walks_all_supported_expression_shapes() {
    let lambda = LambdaExpr {
        lambda_list: LambdaList::new(),
        declarations: vec![optimize(Quality::Debug, 3)],
        docstring: None,
        body: vec![probe()],
    };
    let local_function = LocalFunction {
        name: SymbolRef::interned("COMMON-LISP-USER", "LOCAL"),
        lambda: lambda.clone(),
    };
    let local_macro = LocalMacro {
        name: SymbolRef::interned("COMMON-LISP-USER", "MACRO"),
        lambda_list: LambdaList::new(),
        declarations: vec![optimize(Quality::Debug, 3)],
        docstring: None,
        body: vec![probe()],
    };
    let symbol_macro = SymbolMacro {
        name: SymbolRef::interned("COMMON-LISP-USER", "SYMBOL"),
        expansion: probe(),
    };
    let name = SymbolRef::interned("COMMON-LISP-USER", "FUNCTION");
    let forms = vec![
        (
            "call with named operator",
            Expr::Call {
                operator: Operator::Name(name.clone()),
                arguments: vec![probe()],
            },
        ),
        (
            "call with lambda operator",
            Expr::Call {
                operator: Operator::Lambda(Box::new(lambda.clone())),
                arguments: vec![probe()],
            },
        ),
        (
            "named function designator",
            Expr::Function(FunctionDesignator::Name(name)),
        ),
        (
            "lambda function designator",
            Expr::Function(FunctionDesignator::Lambda(Box::new(lambda.clone()))),
        ),
        ("lambda", Expr::Lambda(Box::new(lambda.clone()))),
        (
            "if",
            Expr::If {
                test: Box::new(probe()),
                then: Box::new(probe()),
                otherwise: Some(Box::new(probe())),
            },
        ),
        ("progn", Expr::Progn(vec![probe()])),
        (
            "block",
            Expr::Block {
                name: SymbolRef::interned("COMMON-LISP", "NIL"),
                body: vec![probe()],
            },
        ),
        (
            "return-from with value",
            Expr::ReturnFrom {
                name: SymbolRef::interned("COMMON-LISP", "NIL"),
                value: Some(Box::new(probe())),
            },
        ),
        (
            "tagbody",
            Expr::Tagbody(vec![
                TagbodyItem::Tag(SymbolRef::interned("COMMON-LISP-USER", "TAG")),
                TagbodyItem::Form(probe()),
            ]),
        ),
        (
            "catch",
            Expr::Catch {
                tag: Box::new(probe()),
                body: vec![probe()],
            },
        ),
        (
            "throw",
            Expr::Throw {
                tag: Box::new(probe()),
                value: Box::new(probe()),
            },
        ),
        (
            "unwind-protect",
            Expr::UnwindProtect {
                protected: Box::new(probe()),
                cleanup: vec![probe()],
            },
        ),
        (
            "let",
            Expr::Let {
                sequential: false,
                bindings: vec![
                    LetBinding {
                        name: SymbolRef::interned("COMMON-LISP-USER", "VALUE"),
                        value: Some(probe()),
                    },
                    LetBinding {
                        name: SymbolRef::interned("COMMON-LISP-USER", "NIL"),
                        value: None,
                    },
                ],
                declarations: vec![optimize(Quality::Debug, 3)],
                body: vec![probe()],
            },
        ),
        (
            "progv",
            Expr::Progv {
                symbols: Box::new(probe()),
                values: Box::new(probe()),
                body: vec![probe()],
            },
        ),
        (
            "setq",
            Expr::Setq(vec![(
                SymbolRef::interned("COMMON-LISP-USER", "VALUE"),
                probe(),
            )]),
        ),
        (
            "multiple-value-call",
            Expr::MultipleValueCall {
                function: Box::new(probe()),
                arguments: vec![probe()],
            },
        ),
        (
            "multiple-value-prog1",
            Expr::MultipleValueProg1 {
                first: Box::new(probe()),
                forms: vec![probe()],
            },
        ),
        (
            "the",
            Expr::The {
                type_specifier: TypeSpecifier::new(Literal::Nil),
                value: Box::new(probe()),
            },
        ),
        (
            "load-time-value",
            Expr::LoadTimeValue {
                form: Box::new(probe()),
                read_only: true,
            },
        ),
        (
            "locally",
            Expr::Locally {
                declarations: vec![optimize(Quality::Debug, 3)],
                body: vec![probe()],
            },
        ),
        (
            "flet",
            Expr::Flet {
                definitions: vec![local_function.clone()],
                declarations: vec![optimize(Quality::Debug, 3)],
                body: vec![probe()],
            },
        ),
        (
            "labels",
            Expr::Labels {
                definitions: vec![local_function],
                declarations: vec![optimize(Quality::Debug, 3)],
                body: vec![probe()],
            },
        ),
        (
            "macrolet",
            Expr::Macrolet {
                definitions: vec![local_macro],
                declarations: vec![optimize(Quality::Debug, 3)],
                body: vec![probe()],
            },
        ),
        (
            "symbol-macrolet",
            Expr::SymbolMacrolet {
                declarations: vec![optimize(Quality::Debug, 3)],
                body: vec![probe()],
                definitions: vec![symbol_macro],
            },
        ),
    ];
    for (shape, form) in forms {
        let expression = Expr::Locally {
            declarations: vec![
                Declaration::Special(Vec::new()),
                optimize(Quality::Debug, 3),
            ],
            body: vec![form],
        };
        assert_eq!(
            optimization_level(&expression),
            super::OptimizationLevel::Debug,
            "{shape}"
        );
    }
    assert_eq!(
        optimization_level(&Expr::Progn(Vec::new())),
        super::OptimizationLevel::Safety
    );
    assert_eq!(
        optimization_level(&Expr::Locally {
            declarations: vec![optimize(Quality::Speed, 3), optimize(Quality::Safety, 1)],
            body: Vec::new(),
        }),
        super::OptimizationLevel::Speed
    );
    assert_eq!(
        optimization_level(&Expr::Constant(Literal::Nil)),
        super::OptimizationLevel::Safety
    );
    assert_eq!(
        optimization_level(&Expr::Variable(SymbolRef::interned(
            "COMMON-LISP-USER",
            "VALUE",
        ))),
        super::OptimizationLevel::Safety
    );
}

#[test]
fn unsupported_optimization_qualities_keep_the_safety_default() {
    for quality in [Quality::Space, Quality::CompilationSpeed, Quality::Unknown] {
        let expression = Expr::Locally {
            declarations: vec![optimize(quality, 3)],
            body: Vec::new(),
        };
        assert_eq!(
            optimization_level(&expression),
            super::OptimizationLevel::Safety
        );
    }
}

#[test]
fn payload_round_trips_and_rejects_tampering() {
    let payload = encode_payload(b"(+ 1 2)").expect("payload encoding");
    assert_eq!(
        decode_payload(&payload).expect("payload decoding"),
        "(+ 1 2)"
    );

    let mut wrong_version = payload.clone();
    wrong_version[9] = 2;
    assert!(
        decode_payload(&wrong_version)
            .expect_err("version must be rejected")
            .to_string()
            .contains("unsupported runtime FASL payload version")
    );

    let mut wrong_hash = payload;
    let last = wrong_hash.last_mut().expect("encoded source");
    *last ^= 1;
    assert!(
        decode_payload(&wrong_hash)
            .expect_err("hash mismatch must be rejected")
            .to_string()
            .contains("runtime FASL source hash mismatch")
    );
}

#[test]
fn fasl_headers_and_payload_shape_errors_are_rejected() {
    assert!(is_fasl(b"NCLFASL\0"));
    assert!(!is_fasl(b"NCLRTFASL"));
    assert!(decode_payload(&[]).is_err());

    let payload = encode_payload(b"(+ 1 2)").expect("payload encoding");
    let mut wrong_length = payload.clone();
    wrong_length[11] = 0xff;
    assert!(
        decode_payload(&wrong_length)
            .expect_err("payload length must be rejected")
            .to_string()
            .contains("invalid runtime FASL payload length")
    );

    let invalid_utf8 = encode_payload(&[0xff]).expect("binary payload encoding");
    assert!(
        decode_payload(&invalid_utf8)
            .expect_err("invalid UTF-8 must be rejected")
            .to_string()
            .contains("runtime FASL source is not valid UTF-8")
    );
}

#[test]
fn read_forms_collects_forms_and_propagates_reader_errors() {
    let object = ObjectRuntime::new().expect("object runtime");
    let mut context = ThreadContext::new();
    context.register(&object).expect("register object runtime");
    ncl_stdlib::register_all(&mut context, &object).expect("register standard library");

    let forms = read_forms(&mut context, &object, "(+ 1 2) 7").expect("read forms");
    assert_eq!(forms.len(), 2);
    assert!(matches!(
        read_forms(&mut context, &object, "(+ 1"),
        Err(crate::RuntimeError::Read(
            ncl_reader::ReadError::UnexpectedEof
        ))
    ));
}
