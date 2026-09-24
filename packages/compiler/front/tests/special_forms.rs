//! Every Common Lisp special operator parses into the frozen AST.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests assert on results"
)]

#[path = "support/forms.rs"]
mod forms;

use forms::Fixture;

use ncl_compiler_front::Expr;
use ncl_object::Word;

#[test]
fn block_parses_with_its_name_and_body() {
    let mut f = Fixture::new();
    let name = f.user("B");
    let body = f.user("X");
    let form = f.form("BLOCK", &[name, body]);
    match f.expand(form).unwrap() {
        Expr::Block { name, body } => {
            assert_eq!(name.name, "B");
            assert_eq!(body.len(), 1);
        }
        other => panic!("expected Block, got {other:?}"),
    }
}

#[test]
fn return_from_parses_with_and_without_a_value() {
    let mut f = Fixture::new();
    let name = f.user("B");
    let form = f.form("RETURN-FROM", &[name]);
    assert!(matches!(
        f.expand(form).unwrap(),
        Expr::ReturnFrom { value: None, .. }
    ));
    let value = Word::fixnum(1);
    let form = f.form("RETURN-FROM", &[name, value]);
    assert!(matches!(
        f.expand(form).unwrap(),
        Expr::ReturnFrom { value: Some(_), .. }
    ));
}

#[test]
fn tagbody_parses_tags_and_forms() {
    let mut f = Fixture::new();
    let tag = f.user("TAG");
    let body = Word::fixnum(1);
    let form = f.form("TAGBODY", &[tag, body]);
    match f.expand(form).unwrap() {
        Expr::Tagbody(items) => assert_eq!(items.len(), 2),
        other => panic!("expected Tagbody, got {other:?}"),
    }
}

#[test]
fn go_parses_a_tag() {
    let mut f = Fixture::new();
    let tag = f.user("TAG");
    let form = f.form("GO", &[tag]);
    match f.expand(form).unwrap() {
        Expr::Go { tag } => assert_eq!(tag.name, "TAG"),
        other => panic!("expected Go, got {other:?}"),
    }
}

#[test]
fn catch_parses_a_tag_and_body() {
    let mut f = Fixture::new();
    let tag = f.keyword("TAG");
    let body = f.user("X");
    let form = f.form("CATCH", &[tag, body]);
    assert!(matches!(f.expand(form).unwrap(), Expr::Catch { .. }));
}

#[test]
fn throw_parses_a_tag_and_value() {
    let mut f = Fixture::new();
    let tag = f.keyword("TAG");
    let value = Word::fixnum(1);
    let form = f.form("THROW", &[tag, value]);
    assert!(matches!(f.expand(form).unwrap(), Expr::Throw { .. }));
}

#[test]
fn unwind_protect_parses_protected_and_cleanup() {
    let mut f = Fixture::new();
    let protected = f.user("X");
    let cleanup = f.user("Y");
    let form = f.form("UNWIND-PROTECT", &[protected, cleanup]);
    match f.expand(form).unwrap() {
        Expr::UnwindProtect { cleanup, .. } => assert_eq!(cleanup.len(), 1),
        other => panic!("expected UnwindProtect, got {other:?}"),
    }
}

#[test]
fn if_parses_two_and_three_form_shapes() {
    let mut f = Fixture::new();
    let a = f.user("A");
    let b = f.user("B");
    let c = f.user("C");
    let form = f.form("IF", &[a, b]);
    assert!(matches!(
        f.expand(form).unwrap(),
        Expr::If {
            otherwise: None,
            ..
        }
    ));
    let form = f.form("IF", &[a, b, c]);
    assert!(matches!(
        f.expand(form).unwrap(),
        Expr::If {
            otherwise: Some(_),
            ..
        }
    ));
}

#[test]
fn progn_parses_its_forms() {
    let mut f = Fixture::new();
    let a = f.user("A");
    let b = f.user("B");
    let form = f.form("PROGN", &[a, b]);
    match f.expand(form).unwrap() {
        Expr::Progn(body) => assert_eq!(body.len(), 2),
        other => panic!("expected Progn, got {other:?}"),
    }
}

#[test]
fn locally_parses_declarations_and_body() {
    let mut f = Fixture::new();
    let x = f.user("X");
    let special = f.cl("SPECIAL");
    let specifier = f.list(&[special, x]);
    let declaration = f.form("DECLARE", &[specifier]);
    let body = f.user("X");
    let form = f.form("LOCALLY", &[declaration, body]);
    match f.expand(form).unwrap() {
        Expr::Locally {
            declarations, body, ..
        } => {
            assert_eq!(declarations.len(), 1);
            assert_eq!(body.len(), 1);
        }
        other => panic!("expected Locally, got {other:?}"),
    }
}

#[test]
fn eval_when_parses_its_situations() {
    let mut f = Fixture::new();
    let situation = f.keyword("EXECUTE");
    let body = Word::fixnum(1);
    let form = f.form("EVAL-WHEN", &[situation, body]);
    match f.expand(form).unwrap() {
        Expr::EvalWhen { situations, body } => {
            assert_eq!(situations.len(), 1);
            assert_eq!(body.len(), 1);
        }
        other => panic!("expected EvalWhen, got {other:?}"),
    }
}

#[test]
fn load_time_value_parses_read_only() {
    let mut f = Fixture::new();
    let value = Word::fixnum(1);
    let t = f.cl("T");
    let form = f.form("LOAD-TIME-VALUE", &[value, t]);
    assert!(matches!(
        f.expand(form).unwrap(),
        Expr::LoadTimeValue {
            read_only: true,
            ..
        }
    ));
}

#[test]
fn let_and_let_star_differ_in_sequencing() {
    let mut f = Fixture::new();
    let x = f.user("X");
    let binding = f.list(&[x, Word::fixnum(1)]);
    let bindings = f.list(&[binding]);
    let body = f.user("X");
    let form = f.form("LET", &[bindings, body]);
    assert!(matches!(
        f.expand(form).unwrap(),
        Expr::Let {
            sequential: false,
            ..
        }
    ));
    let form = f.form("LET*", &[bindings, body]);
    assert!(matches!(
        f.expand(form).unwrap(),
        Expr::Let {
            sequential: true,
            ..
        }
    ));
}

#[test]
fn progv_parses_symbols_values_and_body() {
    let mut f = Fixture::new();
    let symbol = f.user("X");
    let symbol_list = f.list(&[symbol]);
    let symbols = f.form("QUOTE", &[symbol_list]);
    let one = f.list(&[Word::fixnum(1)]);
    let values = f.form("QUOTE", &[one]);
    let body = f.user("X");
    let form = f.form("PROGV", &[symbols, values, body]);
    match f.expand(form).unwrap() {
        Expr::Progv { body, .. } => assert_eq!(body.len(), 1),
        other => panic!("expected Progv, got {other:?}"),
    }
}

#[test]
fn flet_and_labels_parse_local_functions() {
    let mut f = Fixture::new();
    let name = f.user("F");
    let lambda_list = Word::NIL;
    let body = Word::fixnum(1);
    let definition = f.list(&[name, lambda_list, body]);
    let definitions = f.list(&[definition]);
    let call = f.form("F", &[]);
    let form = f.form("FLET", &[definitions, call]);
    match f.expand(form).unwrap() {
        Expr::Flet { definitions, .. } => assert_eq!(definitions.len(), 1),
        other => panic!("expected Flet, got {other:?}"),
    }
    let form = f.form("LABELS", &[definitions, call]);
    match f.expand(form).unwrap() {
        Expr::Labels { definitions, .. } => assert_eq!(definitions.len(), 1),
        other => panic!("expected Labels, got {other:?}"),
    }
}

#[test]
fn macrolet_parses_local_macros() {
    let mut f = Fixture::new();
    let name = f.user("M");
    let parameter = f.user("X");
    let lambda_list = f.list(&[parameter]);
    let body = f.form("QUOTE", &[parameter]);
    let definition = f.list(&[name, lambda_list, body]);
    let definitions = f.list(&[definition]);
    let form = f.form("MACROLET", &[definitions]);
    match f.expand(form).unwrap() {
        Expr::Macrolet { definitions, .. } => {
            assert_eq!(definitions.len(), 1);
            assert_eq!(definitions[0].name.name, "M");
        }
        other => panic!("expected Macrolet, got {other:?}"),
    }
}

#[test]
fn symbol_macrolet_parses_local_symbol_macros() {
    let mut f = Fixture::new();
    let name = f.user("X");
    let expansion = f.user("Y");
    let definition = f.list(&[name, expansion]);
    let definitions = f.list(&[definition]);
    let body = f.user("X");
    let form = f.form("SYMBOL-MACROLET", &[definitions, body]);
    match f.expand(form).unwrap() {
        Expr::SymbolMacrolet { definitions, .. } => assert_eq!(definitions.len(), 1),
        other => panic!("expected SymbolMacrolet, got {other:?}"),
    }
}

#[test]
fn function_parses_a_name_and_a_lambda() {
    let mut f = Fixture::new();
    let name = f.user("F");
    let form = f.form("FUNCTION", &[name]);
    assert!(matches!(f.expand(form).unwrap(), Expr::Function(_)));
    let lambda_list = Word::NIL;
    let lambda_head = f.cl("LAMBDA");
    let lambda = f.list(&[lambda_head, lambda_list]);
    let form = f.form("FUNCTION", &[lambda]);
    match f.expand(form).unwrap() {
        Expr::Function(designator) => assert!(matches!(
            designator,
            ncl_compiler_front::FunctionDesignator::Lambda(_)
        )),
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn quote_parses_a_constant() {
    let mut f = Fixture::new();
    let datum = f.user("X");
    let form = f.form("QUOTE", &[datum]);
    match f.expand(form).unwrap() {
        Expr::Constant(literal) => {
            assert!(matches!(literal, ncl_compiler_front::Literal::Symbol(_)));
        }
        other => panic!("expected Constant, got {other:?}"),
    }
}

#[test]
fn the_parses_a_type_and_a_form() {
    let mut f = Fixture::new();
    let type_name = f.cl("INTEGER");
    let value = f.user("X");
    let form = f.form("THE", &[type_name, value]);
    assert!(matches!(f.expand(form).unwrap(), Expr::The { .. }));
}

#[test]
fn truly_the_maps_to_the() {
    let mut f = Fixture::new();
    let type_name = f.cl("INTEGER");
    let value = f.user("X");
    let operator = f.intern("SB-EXT", "TRULY-THE");
    let form = f.list(&[operator, type_name, value]);
    assert!(matches!(f.expand(form).unwrap(), Expr::The { .. }));
}

#[test]
fn multiple_value_call_parses_a_function_and_arguments() {
    let mut f = Fixture::new();
    let name = f.user("F");
    let function = f.form("FUNCTION", &[name]);
    let argument = f.user("X");
    let form = f.form("MULTIPLE-VALUE-CALL", &[function, argument]);
    match f.expand(form).unwrap() {
        Expr::MultipleValueCall { arguments, .. } => assert_eq!(arguments.len(), 1),
        other => panic!("expected MultipleValueCall, got {other:?}"),
    }
}

#[test]
fn multiple_value_prog1_parses_first_and_forms() {
    let mut f = Fixture::new();
    let first = f.user("X");
    let other = f.user("Y");
    let form = f.form("MULTIPLE-VALUE-PROG1", &[first, other]);
    match f.expand(form).unwrap() {
        Expr::MultipleValueProg1 { forms, .. } => assert_eq!(forms.len(), 1),
        other => panic!("expected MultipleValueProg1, got {other:?}"),
    }
}

#[test]
fn setq_parses_ordered_pairs() {
    let mut f = Fixture::new();
    let x = f.user("X");
    let y = f.user("Y");
    let form = f.form("SETQ", &[x, Word::fixnum(1), y, Word::fixnum(2)]);
    match f.expand(form).unwrap() {
        Expr::Setq(pairs) => assert_eq!(pairs.len(), 2),
        other => panic!("expected Setq, got {other:?}"),
    }
}

#[test]
fn a_call_with_a_lambda_operator_parses() {
    let mut f = Fixture::new();
    let lambda_head = f.cl("LAMBDA");
    let lambda = f.list(&[lambda_head, Word::NIL, Word::fixnum(1)]);
    let form = f.list(&[lambda, Word::fixnum(2)]);
    match f.expand(form).unwrap() {
        Expr::Call { operator, .. } => {
            assert!(matches!(operator, ncl_compiler_front::Operator::Lambda(_)));
        }
        other => panic!("expected Call, got {other:?}"),
    }
}

#[test]
fn a_named_call_parses_its_arguments() {
    let mut f = Fixture::new();
    let argument = f.user("X");
    let form = f.form("LIST", &[argument]);
    match f.expand(form).unwrap() {
        Expr::Call {
            operator,
            arguments,
        } => {
            assert!(matches!(operator, ncl_compiler_front::Operator::Name(_)));
            assert_eq!(arguments.len(), 1);
        }
        other => panic!("expected Call, got {other:?}"),
    }
}

#[test]
fn primitive_reports_the_missing_representation() {
    let mut f = Fixture::new();
    let operator = f.intern("SB-SYS", "%PRIMITIVE");
    let name = f.user("NAME");
    let form = f.list(&[operator, name]);
    assert!(f.expand(form).is_err());
}
