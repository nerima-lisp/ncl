use ncl_object::{
    ObjectRef, Package, Runtime, ThreadContext, Word, classify, make_string, pop_root, push_root,
    symbol_is_special, symbol_name, symbol_value,
};

use super::{NonNegative, PrintBase, PrintCase};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SpecialValue {
    Nil,
    Fixnum(i64),
    Symbol(Word),
    Other(Word),
}

fn find_special(ctx: &mut ThreadContext, runtime: &Runtime, qualified: &str) -> Option<Word> {
    let (package_name, symbol_name) = qualified
        .split_once("::")
        .unwrap_or(("COMMON-LISP", qualified));
    let package = runtime.find_package(ctx, package_name)?;
    let mut name = make_string(ctx, runtime, &symbol_name.chars().collect::<Vec<char>>()).ok()?;
    let token = push_root(ctx, &mut name);
    let found = Package::from(package)
        .find_symbol(ctx, name)
        .ok()
        .flatten()
        .map(|(symbol, _status)| symbol);
    let _ = pop_root(ctx, token);
    let symbol = found?;
    symbol_is_special(ctx, symbol)
        .ok()
        .filter(|is_special| *is_special)
        .map(|_| symbol)
}

fn special_value(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    qualified: &str,
) -> Option<SpecialValue> {
    let symbol = find_special(ctx, runtime, qualified)?;
    let value = symbol_value(ctx, symbol).ok()?;
    if value == Word::UNBOUND {
        return None;
    }
    let classified = classify(value);
    Some(if let ObjectRef::Fixnum(number) = classified {
        SpecialValue::Fixnum(number)
    } else if let ObjectRef::Symbol(symbol) = classified {
        if symbol == Word::NIL {
            SpecialValue::Nil
        } else {
            SpecialValue::Symbol(symbol)
        }
    } else {
        SpecialValue::Other(value)
    })
}

pub(super) fn bool_special(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    qualified: &str,
    fallback: bool,
) -> bool {
    special_value(ctx, runtime, qualified)
        .map_or(fallback, |value| !matches!(value, SpecialValue::Nil))
}

pub(super) fn length_special(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    qualified: &str,
) -> Option<NonNegative> {
    special_value(ctx, runtime, qualified)
        .and_then(|value| match value {
            SpecialValue::Fixnum(value) => Some(value),
            SpecialValue::Nil | SpecialValue::Symbol(_) | SpecialValue::Other(_) => None,
        })
        .and_then(|value| usize::try_from(value).ok())
        .map(NonNegative::new)
}

pub(super) fn base_special(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    fallback: PrintBase,
) -> PrintBase {
    special_value(ctx, runtime, "*PRINT-BASE*")
        .and_then(|value| match value {
            SpecialValue::Fixnum(value) => Some(value),
            SpecialValue::Nil | SpecialValue::Symbol(_) | SpecialValue::Other(_) => None,
        })
        .and_then(|value| u32::try_from(value).ok())
        .and_then(PrintBase::new)
        .unwrap_or(fallback)
}

pub(super) fn case_special(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    fallback: PrintCase,
) -> PrintCase {
    let Some(SpecialValue::Symbol(value)) = special_value(ctx, runtime, "*PRINT-CASE*") else {
        return fallback;
    };
    let Ok(name) = symbol_name(ctx, value) else {
        return fallback;
    };
    let Ok(length) = ncl_object::string_length(ctx, name) else {
        return fallback;
    };
    let mut text = String::with_capacity(length);
    for index in 0..length {
        match ncl_object::string_ref(ctx, name, index) {
            Ok(character) => text.push(character),
            Err(_) => return fallback,
        }
    }
    match text.to_uppercase().as_str() {
        "UPCASE" => PrintCase::Upcase,
        "DOWNCASE" => PrintCase::Downcase,
        "CAPITALIZE" => PrintCase::Capitalize,
        _ => fallback,
    }
}
