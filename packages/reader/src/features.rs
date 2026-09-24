//! Feature-expression evaluation for `#+` and `#-`.

use ncl_object::{Runtime, ThreadContext, Word, car, cdr, string_length, string_ref, symbol_name};

use crate::error::ReadError;

/// Evaluate a read feature expression against the runtime's `*features*` list.
///
/// A symbol is present when its name matches a feature; `(and ...)`,
/// `(or ...)`, and `(not ...)` combine sub-expressions per CLHS 24.1.2.1.
pub fn eval_feature_expr(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    form: Word,
    features: &[String],
) -> Result<bool, ReadError> {
    if let Some(name) = symbol_name_string(ctx, form)? {
        return Ok(features
            .iter()
            .any(|feature| feature.eq_ignore_ascii_case(&name)));
    }
    if !form.is_cons() {
        return Err(ReadError::InvalidFeatureExpression);
    }
    let head = car(ctx, form)?;
    let Some(head_name) = symbol_name_string(ctx, head)? else {
        return Err(ReadError::InvalidFeatureExpression);
    };
    let args = cdr(ctx, form)?;
    match head_name.as_str() {
        "AND" => eval_all(ctx, runtime, args, features, false),
        "OR" => eval_all(ctx, runtime, args, features, true),
        "NOT" => {
            let operand = car(ctx, args)?;
            eval_feature_expr(ctx, runtime, operand, features).map(|present| !present)
        }
        _ => Err(ReadError::InvalidFeatureExpression),
    }
}

/// Fold `and`/`or` over a feature-expression list.
fn eval_all(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: Word,
    features: &[String],
    or: bool,
) -> Result<bool, ReadError> {
    let mut cursor = args;
    while cursor != Word::NIL {
        let item = car(ctx, cursor)?;
        let present = eval_feature_expr(ctx, runtime, item, features)?;
        if or == present {
            return Ok(or);
        }
        cursor = cdr(ctx, cursor)?;
    }
    Ok(!or)
}

/// The name of a symbol as a Rust string, or `None` when `word` is not a symbol.
fn symbol_name_string(ctx: &ThreadContext, word: Word) -> Result<Option<String>, ReadError> {
    let Ok(name) = symbol_name(ctx, word) else {
        return Ok(None);
    };
    let length = string_length(ctx, name)?;
    let mut result = String::with_capacity(length);
    for index in 0..length {
        result.push(string_ref(ctx, name, index)?);
    }
    Ok(Some(result))
}
