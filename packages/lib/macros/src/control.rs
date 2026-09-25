//! Structural expanders for the control macros.

use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{ObjectError, Runtime, ThreadContext, Word};

type Result = std::result::Result<Word, ObjectError>;

fn form(ctx: &mut ThreadContext, runtime: &Runtime, name: &str, args: &[Word]) -> Result {
    let mut values = Vec::with_capacity(args.len() + 1);
    values.push(symbol(ctx, runtime, name)?);
    values.extend_from_slice(args);
    list(ctx, runtime, &values)
}
fn progn(ctx: &mut ThreadContext, runtime: &Runtime, body: &[Word]) -> Result {
    form(ctx, runtime, "PROGN", body)
}
fn quote(ctx: &mut ThreadContext, runtime: &Runtime, value: Word) -> Result {
    form(ctx, runtime, "QUOTE", &[value])
}
fn binding(ctx: &mut ThreadContext, runtime: &Runtime, name: Word, value: Word) -> Result {
    list(ctx, runtime, &[name, value])
}
fn bindings(ctx: &mut ThreadContext, runtime: &Runtime, pairs: &[(Word, Word)]) -> Result {
    let values = pairs
        .iter()
        .map(|(name, value)| binding(ctx, runtime, *name, *value))
        .collect::<Result<Vec<_>>>()?;
    list(ctx, runtime, &values)
}
fn args(ctx: &mut ThreadContext, form: Word) -> Result<Vec<Word>> {
    let mut values = elements(ctx, form)?;
    if values.is_empty() { return Err(ObjectError::TypeError); }
    values.remove(0);
    Ok(values)
}
fn body(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result<Word> {
    progn(ctx, runtime, values)
}
fn and(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result<Word> {
    match values {
        [] => Ok(Word::TRUE),
        [value] => Ok(*value),
        [value, rest @ ..] => form(ctx, runtime, "IF", &[*value, and(ctx, runtime, rest)?, Word::NIL]),
    }
}
fn or(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result<Word> {
    match values {
        [] => Ok(Word::NIL),
        [value] => Ok(*value),
        [value, rest @ ..] => {
            let temporary = fresh_symbol(ctx, runtime)?;
            let binding = bindings(ctx, runtime, &[(temporary, *value)])?;
            let next = or(ctx, runtime, rest)?;
            let test = form(ctx, runtime, "IF", &[temporary, temporary, next])?;
            form(ctx, runtime, "LET", &[binding, test])
        }
    }
}
fn cond(ctx: &mut ThreadContext, runtime: &Runtime, clauses: &[Word]) -> Result<Word> {
    let Some(clause) = clauses.first() else { return Ok(Word::NIL); };
    let values = elements(ctx, *clause)?;
    let test = *values.first().ok_or(ObjectError::TypeError)?;
    let yes = if values.len() == 1 { test } else { body(ctx, runtime, &values[1..])? };
    form(ctx, runtime, "IF", &[test, yes, cond(ctx, runtime, &clauses[1..])?])
}
fn equality_test(ctx: &mut ThreadContext, runtime: &Runtime, value: Word, keys: Word) -> Result<Word> {
    let keys = elements(ctx, keys)?;
    let tests = keys.into_iter().map(|key| {
        let quoted = quote(ctx, runtime, key)?;
        form(ctx, runtime, "EQL", &[value, quoted])
    }).collect::<Result<Vec<_>>>()?;
    or(ctx, runtime, &tests)
}
fn case_clauses(ctx: &mut ThreadContext, runtime: &Runtime, value: Word, clauses: &[Word], error: bool) -> Result<Word> {
    let Some(clause) = clauses.first() else {
        return if error { form(ctx, runtime, "ERROR", &[quote(ctx, runtime, symbol(ctx, runtime, "CASE-FAILURE")?)?]) } else { Ok(Word::NIL) };
    };
    let values = elements(ctx, *clause)?;
    let keys = *values.first().ok_or(ObjectError::TypeError)?;
    let yes = body(ctx, runtime, &values[1..])?;
    let otherwise = symbol(ctx, runtime, "OTHERWISE")?;
    let test = if keys == otherwise || keys == Word::TRUE { Word::TRUE } else { equality_test(ctx, runtime, value, keys)? };
    form(ctx, runtime, "IF", &[test, yes, case_clauses(ctx, runtime, value, &clauses[1..], error)?])
}
fn case(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word], error: bool) -> Result<Word> {
    let value = *values.first().ok_or(ObjectError::TypeError)?;
    let temporary = fresh_symbol(ctx, runtime)?;
    let binding = bindings(ctx, runtime, &[(temporary, value)])?;
    let result = case_clauses(ctx, runtime, temporary, &values[1..], error)?;
    form(ctx, runtime, "LET", &[binding, result])
}
fn type_test(ctx: &mut ThreadContext, runtime: &Runtime, value: Word, types: Word) -> Result<Word> {
    let types = elements(ctx, types)?;
    let tests = types.into_iter().map(|ty| form(ctx, runtime, "TYPEP", &[value, quote(ctx, runtime, ty)?])).collect::<Result<Vec<_>>>()?;
    or(ctx, runtime, &tests)
}
fn type_clauses(ctx: &mut ThreadContext, runtime: &Runtime, value: Word, clauses: &[Word], error: bool) -> Result<Word> {
    let Some(clause) = clauses.first() else {
        return if error { form(ctx, runtime, "ERROR", &[quote(ctx, runtime, symbol(ctx, runtime, "TYPE-ERROR")?)?]) } else { Ok(Word::NIL) };
    };
    let values = elements(ctx, *clause)?;
    let test = type_test(ctx, runtime, value, *values.first().ok_or(ObjectError::TypeError)?)?;
    let yes = body(ctx, runtime, &values[1..])?;
    form(ctx, runtime, "IF", &[test, yes, type_clauses(ctx, runtime, value, &clauses[1..], error)?])
}
fn typecase(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word], error: bool) -> Result<Word> {
    let value = *values.first().ok_or(ObjectError::TypeError)?;
    let temporary = fresh_symbol(ctx, runtime)?;
    let binding = bindings(ctx, runtime, &[(temporary, value)])?;
    let result = type_clauses(ctx, runtime, temporary, &values[1..], error)?;
    form(ctx, runtime, "LET", &[binding, result])
}
fn prog(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word], star: bool) -> Result<Word> {
    let specs = elements(ctx, *values.first().ok_or(ObjectError::TypeError)?)?;
    let mut pairs = Vec::new();
    for spec in specs { let spec = elements(ctx, spec)?; let name = *spec.first().ok_or(ObjectError::TypeError)?; pairs.push((name, spec.get(1).copied().unwrap_or(Word::NIL))); }
    let let_form = if star { "LET*" } else { "LET" };
    let scoped = form(ctx, runtime, let_form, &[bindings(ctx, runtime, &pairs)?, form(ctx, runtime, "TAGBODY", &values[1..])?])?;
    form(ctx, runtime, "BLOCK", &[Word::NIL, scoped])
}
fn prog1(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result<Word> {
    let temporary = fresh_symbol(ctx, runtime)?;
    let mut body = values[1..].to_vec(); body.push(temporary);
    form(ctx, runtime, "LET", &[bindings(ctx, runtime, &[(temporary, values.first().copied().unwrap_or(Word::NIL))])?, progn(ctx, runtime, &body)?])
}
fn do_macro(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word], star: bool) -> Result<Word> {
    let specs = elements(ctx, *values.first().ok_or(ObjectError::TypeError)?)?;
    let test = elements(ctx, *values.get(1).ok_or(ObjectError::TypeError)?)?;
    let mut pairs = Vec::new(); let mut updates = Vec::new();
    for spec in specs { let spec = elements(ctx, spec)?; let name = *spec.first().ok_or(ObjectError::TypeError)?; pairs.push((name, spec.get(1).copied().unwrap_or(Word::NIL))); updates.push((name, spec.get(2).copied().unwrap_or(name))); }
    let finish = form(ctx, runtime, "RETURN-FROM", &[Word::NIL, test.get(1).copied().unwrap_or(Word::NIL)])?;
    let mut tagbody = vec![form(ctx, runtime, "IF", &[*test.first().ok_or(ObjectError::TypeError)?, finish, Word::NIL])?]; tagbody.extend_from_slice(&values[2..]);
    for (name, step) in updates { tagbody.push(form(ctx, runtime, "SETQ", &[name, step])?); }
    let scoped = form(ctx, runtime, if star { "LET*" } else { "LET" }, &[bindings(ctx, runtime, &pairs)?, form(ctx, runtime, "TAGBODY", &tagbody)?])?;
    form(ctx, runtime, "BLOCK", &[Word::NIL, scoped])
}

macro_rules! wrappers { ($($name:ident => $kind:expr),* $(,)?) => {$ (
    fn $name(runtime: &Runtime, ctx: &mut ThreadContext, args: &[Word], _: &mut ncl_object::MultipleValues) -> Result<Word> {
        let values = args(ctx, *args.first().ok_or(ObjectError::TypeError)?)?;
        expand_named(runtime, ctx, &values, $kind)
    }
)* } }
const WHEN: u8 = 0; const UNLESS: u8 = 1; const AND: u8 = 2; const OR: u8 = 3; const COND: u8 = 4;
const CASE: u8 = 5; const ECASE: u8 = 6; const TYPECASE: u8 = 7; const ETYPECASE: u8 = 8;
const PROG: u8 = 9; const PROGSTAR: u8 = 10; const PROG1: u8 = 11; const PROG2: u8 = 12;
const RETURN: u8 = 13; const NTHVALUE: u8 = 14; const DO: u8 = 15; const DOSTAR: u8 = 16;
wrappers! { expand_when=>WHEN, expand_unless=>UNLESS, expand_and=>AND, expand_or=>OR, expand_cond=>COND,
    expand_case=>CASE, expand_ecase=>ECASE, expand_ccase=>ECASE, expand_typecase=>TYPECASE,
    expand_etypecase=>ETYPECASE, expand_ctypecase=>ETYPECASE, expand_prog=>PROG, expand_prog_star=>PROGSTAR,
    expand_prog1=>PROG1, expand_prog2=>PROG2, expand_return=>RETURN, expand_nth_value=>NTHVALUE,
    expand_do=>DO, expand_do_star=>DOSTAR }
pub fn callback_for(name: &str) -> Option<ncl_object::RustBuiltin> {
    Some(match name { "WHEN"=>expand_when, "UNLESS"=>expand_unless, "AND"=>expand_and, "OR"=>expand_or,
        "COND"=>expand_cond, "CASE"=>expand_case, "ECASE"=>expand_ecase, "CCASE"=>expand_ccase,
        "TYPECASE"=>expand_typecase, "ETYPECASE"=>expand_etypecase, "CTYPECASE"=>expand_ctypecase,
        "PROG"=>expand_prog, "PROG*"=>expand_prog_star, "PROG1"=>expand_prog1, "PROG2"=>expand_prog2,
        "RETURN"=>expand_return, "NTH-VALUE"=>expand_nth_value, "DO"=>expand_do, "DO*"=>expand_do_star,
        _=>return None })
}
fn expand_named(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word], kind: u8) -> Result<Word> {
    let name = kind;
    match name.as_str() {
        "WHEN" => form(ctx, runtime, "IF", &[values[0], body(ctx, runtime, &values[1..])?, Word::NIL]),
        "UNLESS" => form(ctx, runtime, "IF", &[values[0], Word::NIL, body(ctx, runtime, &values[1..])?]),
        "AND" => and(ctx, runtime, &values), "OR" => or(ctx, runtime, &values), "COND" => cond(ctx, runtime, &values),
        "CASE" => case(ctx, runtime, &values, false), "ECASE"|"CCASE" => case(ctx, runtime, &values, true),
        "TYPECASE" => typecase(ctx, runtime, &values, false), "ETYPECASE"|"CTYPECASE" => typecase(ctx, runtime, &values, true),
        "PROG" => prog(ctx, runtime, &values, false), "PROG*" => prog(ctx, runtime, &values, true),
        "PROG1" => prog1(ctx, runtime, &values), "PROG2" => form(ctx, runtime, "PROGN", &[values.first().copied().unwrap_or(Word::NIL), prog1(ctx, runtime, &values[1..])?]),
        "RETURN" => form(ctx, runtime, "RETURN-FROM", &[Word::NIL, values.first().copied().unwrap_or(Word::NIL)]),
        "NTH-VALUE" => form(ctx, runtime, "MULTIPLE-VALUE-CALL", &[form(ctx, runtime, "LAMBDA", &[list(ctx, runtime, &[symbol(ctx, runtime, "&REST")?, fresh_symbol(ctx, runtime)?])?, Word::NIL])?, values.get(1).copied().ok_or(ObjectError::TypeError)?]),
        "DO" => do_macro(ctx, runtime, &values, false), "DO*" => do_macro(ctx, runtime, &values, true),
        "DOLIST"|"DOTIMES" => Err(ObjectError::Unsupported), _ => Err(ObjectError::Unsupported),
    }
}
