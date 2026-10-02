//! Direct coverage for front-end value objects and small public APIs.
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "support/forms.rs"]
mod forms;

use forms::Fixture;
use ncl_compiler_front::{
    ArityPattern, CompilerMacro, Expr, FrontError, LambdaList, LexicalEnv, Literal, LocalMacro,
    MacroCaller, MacroRegistry, NumberLiteral, SymbolMacro, SymbolRef, TypeSpecifier,
    VariableBinding,
};
use ncl_object::{
    ArrayElementType, ArrayOptions, Word, make_array, make_bignum_from_i128, make_complex,
    make_double, make_ratio, make_simple_vector,
};

#[allow(clippy::missing_const_for_fn, clippy::unnecessary_wraps)]
fn noop_expander(
    _ctx: &mut ncl_object::ThreadContext,
    _runtime: &ncl_object::Runtime,
    _form: Word,
) -> Result<Option<Word>, FrontError> {
    Ok(None)
}

#[test]
fn symbol_refs_preserve_package_identity_and_display() {
    let common = SymbolRef::interned("COMMON-LISP", "CAR");
    let keyword = SymbolRef::keyword("SIZE");
    let gensym = SymbolRef::uninterned("TEMP", 7);

    assert_eq!(common.to_string(), "COMMON-LISP:CAR");
    assert!(common.is_common_lisp("CAR"));
    assert!(!common.is_keyword());
    assert_eq!(keyword.package_name(), Some("KEYWORD"));
    assert!(keyword.is_keyword());
    assert_eq!(gensym.to_string(), "#:TEMP#7");
    assert!(gensym.is_uninterned());
    assert!(!gensym.is_named("COMMON-LISP", "TEMP"));
}

#[test]
fn literals_distinguish_self_evaluating_and_dotted_data() {
    let proper = Literal::Cons(
        Box::new(Literal::fixnum(1)),
        Box::new(Literal::Cons(Box::new(Literal::T), Box::new(Literal::Nil))),
    );
    let dotted = Literal::Cons(Box::new(Literal::fixnum(1)), Box::new(Literal::T));

    assert_eq!(proper.list_elements().unwrap().len(), 2);
    assert!(dotted.list_elements().is_none());
    assert!(!proper.is_self_evaluating());
    assert!(Literal::String(vec!['o', 'k']).is_self_evaluating());
    assert_eq!(
        Literal::double(1.5),
        Literal::Number(NumberLiteral::DoubleFloat(1.5))
    );
}

#[test]
fn type_specifier_round_trips_its_source_form() {
    let source = Literal::Cons(
        Box::new(Literal::Symbol(SymbolRef::interned(
            "COMMON-LISP",
            "INTEGER",
        ))),
        Box::new(Literal::Nil),
    );
    let specifier = TypeSpecifier::new(source.clone());

    assert_eq!(specifier.form(), &source);
    assert_eq!(specifier.into_form(), source);
}

#[test]
fn compiler_macro_registry_replaces_entries_and_reports_iteration_state() {
    let name = SymbolRef::interned("COMMON-LISP-USER", "M");
    let mut registry = MacroRegistry::new();
    assert!(registry.is_empty());

    registry.insert(CompilerMacro {
        name: name.clone(),
        arity: ArityPattern::new(1, Some(2)),
        expander: noop_expander,
        feature: Some("old"),
    });
    assert_eq!(registry.len(), 1);
    assert!(registry.lookup(&name).unwrap().arity.accepts(2));
    assert!(!registry.lookup(&name).unwrap().arity.accepts(3));

    registry.insert(CompilerMacro {
        name: name.clone(),
        arity: ArityPattern::new(0, None),
        expander: noop_expander,
        feature: None,
    });
    assert_eq!(registry.len(), 1);
    assert_eq!(registry.lookup(&name).unwrap().feature, None);
    assert_eq!(registry.iter().count(), 1);
}

#[test]
fn lexical_environment_resolves_inner_bindings_and_scope_side_effects() {
    let name = SymbolRef::interned("COMMON-LISP-USER", "X");
    let function = SymbolRef::interned("COMMON-LISP-USER", "F");
    let block = SymbolRef::interned("COMMON-LISP-USER", "B");
    let tag = SymbolRef::interned("COMMON-LISP-USER", "TAG");
    let macro_name = SymbolRef::interned("COMMON-LISP-USER", "M");
    let symbol_macro_name = SymbolRef::interned("COMMON-LISP-USER", "S");
    let local_macro = LocalMacro {
        name: macro_name.clone(),
        lambda_list: LambdaList::new(),
        declarations: vec![],
        docstring: None,
        body: vec![Expr::Constant(Literal::T)],
    };
    let symbol_macro = SymbolMacro {
        name: symbol_macro_name.clone(),
        expansion: Expr::Constant(Literal::fixnum(9)),
    };
    let mut env = LexicalEnv::new();
    env.bind_variable(name.clone(), None);
    env.bind_function(function.clone());
    env.bind_block(block.clone());
    env.bind_tag(tag.clone());
    env.bind_macro(local_macro.clone());
    env.bind_symbol_macro(symbol_macro.clone());
    env.push_scope();
    env.bind_special(name.clone());

    assert_eq!(env.depth(), 2);
    assert_eq!(
        env.lookup_variable(&name),
        Some(&VariableBinding::Special(name.clone()))
    );
    assert!(env.is_special(&name));
    assert!(env.lookup_function(&function));
    assert_eq!(env.lookup_macro(&macro_name), Some(&local_macro));
    assert_eq!(
        env.lookup_symbol_macro(&symbol_macro_name),
        Some(&symbol_macro)
    );
    assert!(env.find_block(&block));
    assert!(env.find_tag(&tag));
    env.pop_scope();
    assert_eq!(
        env.lookup_variable(&name),
        Some(&VariableBinding::Lexical {
            name,
            type_specifier: None
        })
    );
    env.pop_scope();
    env.pop_scope();
    assert_eq!(env.depth(), 1);
}

struct DefaultCaller;

impl MacroCaller for DefaultCaller {
    fn call_macro(
        &mut self,
        _ctx: &mut ncl_object::ThreadContext,
        _runtime: &ncl_object::Runtime,
        _name: &SymbolRef,
        _form: Word,
    ) -> Result<Word, FrontError> {
        Ok(Word::TRUE)
    }
}

#[test]
fn default_local_macro_call_is_a_typed_error_with_stable_output() {
    let mut caller = DefaultCaller;
    let mut fixture = Fixture::new();
    let definition = LocalMacro {
        name: SymbolRef::interned("COMMON-LISP-USER", "M"),
        lambda_list: LambdaList::new(),
        declarations: vec![],
        docstring: None,
        body: vec![],
    };
    let error = caller
        .call_local_macro(&mut fixture.ctx, &fixture.runtime, &definition, Word::NIL)
        .unwrap_err();

    assert_eq!(
        error,
        FrontError::MacroExpansion {
            name: definition.name.clone(),
            detail: "local macro expansion is not available".to_owned()
        }
    );
    assert_eq!(
        error.to_string(),
        "macro expansion of COMMON-LISP-USER:M failed: local macro expansion is not available"
    );
}

#[test]
fn form_conversion_preserves_numbers_collections_and_errors() {
    let mut fixture = Fixture::new();
    let mut table = ncl_compiler_front::form::UninternedTable::new();
    let double = make_double(&mut fixture.ctx, &fixture.runtime, 1.25).unwrap();
    let bignum =
        make_bignum_from_i128(&mut fixture.ctx, &fixture.runtime, -((1_i128 << 40) + 5)).unwrap();
    let ratio = make_ratio(
        &mut fixture.ctx,
        &fixture.runtime,
        bignum.into(),
        double.into(),
    )
    .unwrap();
    let complex = make_complex(
        &mut fixture.ctx,
        &fixture.runtime,
        ratio.into(),
        Word::fixnum(2),
    )
    .unwrap();
    let true_symbol = fixture.cl("T");
    let vector = make_simple_vector(
        &mut fixture.ctx,
        &fixture.runtime,
        &[Word::fixnum(1), true_symbol],
    )
    .unwrap();
    let array = make_array(
        &mut fixture.ctx,
        &fixture.runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(4),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    let string = fixture.string("hi");

    assert_eq!(
        ncl_compiler_front::form::number_literal(&mut fixture.ctx, complex.into()).unwrap(),
        NumberLiteral::Complex {
            real: Box::new(NumberLiteral::Ratio {
                numerator: Box::new(NumberLiteral::Bignum {
                    negative: true,
                    limbs: vec![5, 256]
                }),
                denominator: Box::new(NumberLiteral::DoubleFloat(1.25))
            }),
            imaginary: Box::new(NumberLiteral::Fixnum(2))
        }
    );
    assert_eq!(
        ncl_compiler_front::form::literal(&mut fixture.ctx, &mut table, vector).unwrap(),
        Literal::Vector(vec![Literal::fixnum(1), Literal::T])
    );
    assert_eq!(
        ncl_compiler_front::form::literal(&mut fixture.ctx, &mut table, array).unwrap(),
        Literal::Array {
            dimensions: vec![2],
            elements: vec![Literal::fixnum(4), Literal::fixnum(4)]
        }
    );
    assert_eq!(
        ncl_compiler_front::form::literal(&mut fixture.ctx, &mut table, string).unwrap(),
        Literal::String(vec!['h', 'i'])
    );
    assert_eq!(
        ncl_compiler_front::form::list(&mut fixture.ctx, Word::NIL).unwrap(),
        Vec::<Word>::new()
    );
    assert_eq!(
        ncl_compiler_front::form::list(&mut fixture.ctx, Word::TRUE),
        Err(FrontError::ImproperList)
    );
}

#[test]
fn front_errors_render_their_variant_and_source_contracts() {
    let name = SymbolRef::keyword("TAG");
    let errors = [
        (
            FrontError::ImproperList,
            "improper list where a proper list is required",
        ),
        (
            FrontError::WrongNumberOfForms {
                operator: name.clone(),
                expected: "one form",
                found: 2,
            },
            "KEYWORD:TAG expects one form, found 2 subforms",
        ),
        (
            FrontError::UnsupportedLiteral,
            "quoted object is not representable in the literal set",
        ),
        (
            FrontError::InvalidOperator {
                detail: "number".to_owned(),
            },
            "invalid operator: number",
        ),
        (
            FrontError::MalformedForm {
                operator: name,
                detail: "bad body".to_owned(),
            },
            "malformed KEYWORD:TAG form: bad body",
        ),
    ];

    for (error, output) in errors {
        assert_eq!(error.to_string(), output);
        assert!(std::error::Error::source(&error).is_none());
    }
}

#[test]
fn variable_binding_accessors_report_name_type_and_specialness() {
    let name = SymbolRef::interned("COMMON-LISP-USER", "X");
    let type_specifier = TypeSpecifier::new(Literal::Symbol(SymbolRef::interned(
        "COMMON-LISP",
        "INTEGER",
    )));
    let lexical = VariableBinding::Lexical {
        name: name.clone(),
        type_specifier: Some(type_specifier.clone()),
    };
    let special = VariableBinding::Special(name.clone());

    assert_eq!(lexical.name(), &name);
    assert!(!lexical.is_special());
    assert_eq!(lexical.type_specifier(), Some(&type_specifier));
    assert!(special.is_special());
    assert_eq!(special.type_specifier(), None);
}
