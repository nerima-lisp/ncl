//! Control special operators: `block`, `return-from`, `tagbody`, `go`,
//! `catch`, `throw`, `unwind-protect`, `if`, `progn`, `locally`, `eval-when`,
//! and `load-time-value`.

use ncl_object::{ObjectRef, Word, classify_object};

use crate::ast::{EvalSituation, Expr, TagbodyItem};
use crate::error::FrontError;
use crate::expand::FormExpander;
use crate::special::{self, SpecialForm};

/// Parse one of the control special operators.
///
/// # Errors
///
/// Returns a [`FrontError`] describing the malformed form.
pub fn parse(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    match kind {
        SpecialForm::Block => block(expander, kind, form),
        SpecialForm::ReturnFrom => return_from(expander, kind, form),
        SpecialForm::Tagbody => tagbody(expander, kind, form),
        SpecialForm::Go => go(expander, kind, form),
        SpecialForm::Catch => catch(expander, kind, form),
        SpecialForm::Throw => throw(expander, kind, form),
        SpecialForm::UnwindProtect => unwind_protect(expander, kind, form),
        SpecialForm::If => if_form(expander, kind, form),
        SpecialForm::Progn => progn(expander, kind, form),
        SpecialForm::Locally => locally(expander, kind, form),
        SpecialForm::EvalWhen => eval_when(expander, kind, form),
        _ => load_time_value(expander, kind, form),
    }
}

/// `(block name form*)`.
fn block(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let Some((name, body)) = arguments.split_first() else {
        return Err(special::arity(kind, "a block name", 0));
    };
    let name = expander.symbol(*name)?;
    expander.env_mut().push_scope();
    expander.env_mut().bind_block(name.clone());
    let body = expander.expand_all(body)?;
    expander.env_mut().pop_scope();
    Ok(Expr::Block { name, body })
}

/// `(return-from name [value])`.
fn return_from(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let Some((name, rest)) = arguments.split_first() else {
        return Err(special::arity(kind, "a block name", 0));
    };
    if rest.len() > 1 {
        return Err(special::arity(
            kind,
            "a block name and an optional value",
            rest.len() + 1,
        ));
    }
    let name = expander.symbol(*name)?;
    let value = rest
        .first()
        .map(|value| expander.expand(*value))
        .transpose()?
        .map(Box::new);
    Ok(Expr::ReturnFrom { name, value })
}

/// `(tagbody tag-or-form*)`.
fn tagbody(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    expander.env_mut().push_scope();
    let mut items = Vec::with_capacity(arguments.len());
    for argument in arguments {
        if matches!(
            classify_object(expander.ctx(), argument),
            ObjectRef::Symbol(_)
        ) {
            let tag = expander.symbol(argument)?;
            expander.env_mut().bind_tag(tag.clone());
            items.push(TagbodyItem::Tag(tag));
        } else {
            items.push(TagbodyItem::Form(expander.expand(argument)?));
        }
    }
    expander.env_mut().pop_scope();
    Ok(Expr::Tagbody(items))
}

/// `(go tag)`.
fn go(expander: &mut FormExpander<'_>, kind: SpecialForm, form: Word) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let [tag] = arguments.as_slice() else {
        return Err(special::arity(kind, "one tag", arguments.len()));
    };
    let tag = expander.symbol(*tag)?;
    Ok(Expr::Go { tag })
}

/// `(catch tag form*)`.
fn catch(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let Some((tag, body)) = arguments.split_first() else {
        return Err(special::arity(kind, "a tag and forms", 0));
    };
    let tag = expander.expand(*tag)?;
    let body = expander.expand_all(body)?;
    Ok(Expr::Catch {
        tag: Box::new(tag),
        body,
    })
}

/// `(throw tag value)`.
fn throw(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let [tag, value] = arguments.as_slice() else {
        return Err(special::arity(kind, "a tag and a value", arguments.len()));
    };
    let tag = expander.expand(*tag)?;
    let value = expander.expand(*value)?;
    Ok(Expr::Throw {
        tag: Box::new(tag),
        value: Box::new(value),
    })
}

/// `(unwind-protect protected cleanup*)`.
fn unwind_protect(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let Some((protected, cleanup)) = arguments.split_first() else {
        return Err(special::arity(kind, "a protected form", 0));
    };
    let protected = expander.expand(*protected)?;
    let cleanup = expander.expand_all(cleanup)?;
    Ok(Expr::UnwindProtect {
        protected: Box::new(protected),
        cleanup,
    })
}

/// `(if test then [else])`.
fn if_form(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let Some((test, rest)) = arguments.split_first() else {
        return Err(special::arity(kind, "a test and a then form", 0));
    };
    if rest.is_empty() || rest.len() > 2 {
        return Err(special::arity(
            kind,
            "a test and a then form",
            arguments.len(),
        ));
    }
    let test = expander.expand(*test)?;
    let then = expander.expand(rest[0])?;
    let otherwise = rest
        .get(1)
        .map(|value| expander.expand(*value))
        .transpose()?
        .map(Box::new);
    Ok(Expr::If {
        test: Box::new(test),
        then: Box::new(then),
        otherwise,
    })
}

/// `(progn form*)`.
fn progn(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    Ok(Expr::Progn(expander.expand_all(&arguments)?))
}

/// `(locally declaration* form*)`.
fn locally(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    expander.env_mut().push_scope();
    let body = expander.expand_declared_body(&arguments)?;
    expander.apply_declarations(&body.declarations);
    expander.env_mut().pop_scope();
    Ok(Expr::Locally {
        declarations: body.declarations,
        body: body.forms,
    })
}

/// `(eval-when situations form*)`.
fn eval_when(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let mut situations = Vec::new();
    let mut index = 0;
    while index < arguments.len() && is_keyword(expander, arguments[index])? {
        let name = expander.symbol(arguments[index])?;
        let situation = eval_situation(&name.name).ok_or_else(|| FrontError::MalformedForm {
            operator: kind.symbol(),
            detail: format!("unknown eval-when situation {name}"),
        })?;
        situations.push(situation);
        index += 1;
    }
    let body = expander.expand_all(&arguments[index..])?;
    Ok(Expr::EvalWhen { situations, body })
}

/// `(load-time-value form [read-only])`.
fn load_time_value(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(special::arity(kind, "one or two forms", arguments.len()));
    }
    let value = expander.expand(arguments[0])?;
    let read_only = arguments.get(1).is_some_and(|flag| *flag != Word::NIL);
    Ok(Expr::LoadTimeValue {
        form: Box::new(value),
        read_only,
    })
}

/// Whether a word is a keyword symbol.
fn is_keyword(expander: &mut FormExpander<'_>, word: Word) -> Result<bool, FrontError> {
    if !matches!(classify_object(expander.ctx(), word), ObjectRef::Symbol(_)) {
        return Ok(false);
    }
    Ok(expander.symbol(word)?.is_keyword())
}

/// Resolve an `eval-when` situation name.
fn eval_situation(name: &str) -> Option<EvalSituation> {
    Some(match name {
        "COMPILE-TOPLEVEL" => EvalSituation::CompileToplevel,
        "LOAD-TOPLEVEL" => EvalSituation::LoadToplevel,
        "EXECUTE" => EvalSituation::Execute,
        "EVAL" => EvalSituation::Eval,
        _ => return None,
    })
}
