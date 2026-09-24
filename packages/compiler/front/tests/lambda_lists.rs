//! Ordinary and macro lambda lists parse into the frozen shape, and malformed
//! lambda lists are rejected.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests assert on results"
)]

#[path = "support/forms.rs"]
mod forms;

use forms::Fixture;

use ncl_compiler_front::{FrontError, LambdaListKind};
use ncl_object::Word;

/// Build a lambda list from raw element words.
fn lambda_list(f: &mut Fixture, elements: &[Word]) -> Word {
    f.list(elements)
}

#[test]
fn required_parameters_parse_in_order() {
    let mut f = Fixture::new();
    let a = f.user("A");
    let b = f.user("B");
    let form = lambda_list(&mut f, &[a, b]);
    let parsed = f.lambda_list(form, LambdaListKind::Ordinary).unwrap();
    assert_eq!(parsed.required.len(), 2);
    assert!(parsed.optional.is_empty());
}

#[test]
fn optional_parses_default_and_supplied_p() {
    let mut f = Fixture::new();
    let optional = f.cl("&OPTIONAL");
    let name = f.user("X");
    let default = Word::fixnum(1);
    let supplied = f.user("P");
    let spec = f.list(&[name, default, supplied]);
    let form = lambda_list(&mut f, &[optional, spec]);
    let parsed = f.lambda_list(form, LambdaListKind::Ordinary).unwrap();
    assert_eq!(parsed.optional.len(), 1);
    assert!(parsed.optional[0].default.is_some());
    assert!(parsed.optional[0].supplied_p.is_some());
}

#[test]
fn rest_parses_a_parameter() {
    let mut f = Fixture::new();
    let rest = f.cl("&REST");
    let name = f.user("R");
    let form = lambda_list(&mut f, &[rest, name]);
    let parsed = f.lambda_list(form, LambdaListKind::Ordinary).unwrap();
    assert!(parsed.rest.is_some());
    assert!(parsed.body.is_none());
}

#[test]
fn key_parses_keyword_name_default_and_supplied_p() {
    let mut f = Fixture::new();
    let key = f.cl("&KEY");
    let keyword = f.keyword("SIZE");
    let name = f.user("SIZE");
    let specifier = f.list(&[keyword, name]);
    let default = Word::fixnum(3);
    let supplied = f.user("P");
    let spec = f.list(&[specifier, default, supplied]);
    let form = lambda_list(&mut f, &[key, spec]);
    let parsed = f.lambda_list(form, LambdaListKind::Ordinary).unwrap();
    assert_eq!(parsed.keys.len(), 1);
    assert_eq!(parsed.keys[0].keyword.name, "SIZE");
    assert_eq!(parsed.keys[0].name.symbol().unwrap().name, "SIZE");
    assert!(parsed.keys[0].default.is_some());
    assert!(parsed.keys[0].supplied_p.is_some());
}

#[test]
fn a_bare_key_parameter_implies_its_keyword() {
    let mut f = Fixture::new();
    let key = f.cl("&KEY");
    let name = f.user("SIZE");
    let form = lambda_list(&mut f, &[key, name]);
    let parsed = f.lambda_list(form, LambdaListKind::Ordinary).unwrap();
    assert_eq!(parsed.keys[0].keyword.name, "SIZE");
    assert!(parsed.keys[0].keyword.is_keyword());
}

#[test]
fn allow_other_keys_is_recorded_after_key() {
    let mut f = Fixture::new();
    let key = f.cl("&KEY");
    let name = f.user("X");
    let allow = f.cl("&ALLOW-OTHER-KEYS");
    let form = lambda_list(&mut f, &[key, name, allow]);
    let parsed = f.lambda_list(form, LambdaListKind::Ordinary).unwrap();
    assert!(parsed.allow_other_keys);
    assert!(parsed.accepts_keywords());
}

#[test]
fn aux_parses_an_initializer() {
    let mut f = Fixture::new();
    let aux = f.cl("&AUX");
    let name = f.user("X");
    let default = Word::fixnum(1);
    let spec = f.list(&[name, default]);
    let form = lambda_list(&mut f, &[aux, spec]);
    let parsed = f.lambda_list(form, LambdaListKind::Ordinary).unwrap();
    assert_eq!(parsed.aux.len(), 1);
    assert!(parsed.aux[0].default.is_some());
}

#[test]
fn a_macro_lambda_list_accepts_whole_environment_and_body() {
    let mut f = Fixture::new();
    let whole = f.cl("&WHOLE");
    let whole_name = f.user("W");
    let environment = f.cl("&ENVIRONMENT");
    let environment_name = f.user("E");
    let body = f.cl("&BODY");
    let body_name = f.user("B");
    let form = lambda_list(
        &mut f,
        &[
            whole,
            whole_name,
            environment,
            environment_name,
            body,
            body_name,
        ],
    );
    let parsed = f.lambda_list(form, LambdaListKind::Macro).unwrap();
    assert!(parsed.whole.is_some());
    assert!(parsed.environment.is_some());
    assert!(parsed.body.is_some());
    assert!(parsed.rest.is_none());
}

#[test]
fn a_macro_lambda_list_accepts_a_destructuring_pattern() {
    let mut f = Fixture::new();
    let optional = f.cl("&OPTIONAL");
    let inner_name = f.user("X");
    let pattern = f.list(&[inner_name]);
    let spec = f.list(&[pattern]);
    let form = lambda_list(&mut f, &[optional, spec]);
    let parsed = f.lambda_list(form, LambdaListKind::Macro).unwrap();
    assert_eq!(parsed.optional.len(), 1);
    assert!(parsed.optional[0].name.symbol().is_none());
}

#[test]
fn macro_only_keywords_are_rejected_in_an_ordinary_list() {
    let mut f = Fixture::new();
    let body = f.cl("&BODY");
    let name = f.user("B");
    let form = lambda_list(&mut f, &[body, name]);
    assert!(matches!(
        f.lambda_list(form, LambdaListKind::Ordinary),
        Err(FrontError::UnknownLambdaListKeyword { .. })
    ));
}

#[test]
fn an_unknown_keyword_is_rejected() {
    let mut f = Fixture::new();
    let unknown = f.cl("&BOGUS");
    let form = lambda_list(&mut f, &[unknown]);
    assert!(matches!(
        f.lambda_list(form, LambdaListKind::Ordinary),
        Err(FrontError::UnknownLambdaListKeyword { .. })
    ));
}

#[test]
fn an_out_of_order_keyword_is_rejected() {
    let mut f = Fixture::new();
    let key = f.cl("&KEY");
    let name = f.user("X");
    let optional = f.cl("&OPTIONAL");
    let form = lambda_list(&mut f, &[key, name, optional]);
    assert!(matches!(
        f.lambda_list(form, LambdaListKind::Ordinary),
        Err(FrontError::LambdaListOrder { .. })
    ));
}

#[test]
fn a_repeated_keyword_is_rejected() {
    let mut f = Fixture::new();
    let optional = f.cl("&OPTIONAL");
    let form = lambda_list(&mut f, &[optional, optional]);
    assert!(matches!(
        f.lambda_list(form, LambdaListKind::Ordinary),
        Err(FrontError::DuplicateLambdaListKeyword { .. })
    ));
}

#[test]
fn a_repeated_parameter_name_is_rejected() {
    let mut f = Fixture::new();
    let name = f.user("X");
    let form = lambda_list(&mut f, &[name, name]);
    assert!(matches!(
        f.lambda_list(form, LambdaListKind::Ordinary),
        Err(FrontError::DuplicateName { .. })
    ));
}

#[test]
fn allow_other_keys_before_key_is_rejected() {
    let mut f = Fixture::new();
    let allow = f.cl("&ALLOW-OTHER-KEYS");
    let form = lambda_list(&mut f, &[allow]);
    assert!(matches!(
        f.lambda_list(form, LambdaListKind::Ordinary),
        Err(FrontError::LambdaListOrder { .. })
    ));
}

#[test]
fn an_empty_lambda_list_is_empty() {
    let mut f = Fixture::new();
    let parsed = f.lambda_list(Word::NIL, LambdaListKind::Ordinary).unwrap();
    assert!(parsed.is_empty());
}
