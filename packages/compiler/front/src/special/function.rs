//! Function and value special operators: `function`, `quote`, `the`,
//! `multiple-value-call`, `multiple-value-prog1`, `setq`, and the SBCL
//! extensions `%primitive`, `nlx-protect`, and `truly-the`.

use ncl_object::{Word, car};

use crate::ast::{Expr, FunctionDesignator};
use crate::error::FrontError;
use crate::expand::FormExpander;
use crate::special::{self, SpecialForm};
use crate::types::TypeSpecifier;

/// Parse one of the function and value special operators.
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
        SpecialForm::Function => function_form(expander, kind, form),
        SpecialForm::Quote => quote(expander, kind, form),
        SpecialForm::The | SpecialForm::TrulyThe => the(expander, kind, form),
        SpecialForm::MultipleValueCall => multiple_value_call(expander, kind, form),
        SpecialForm::MultipleValueProg1 => multiple_value_prog1(expander, kind, form),
        SpecialForm::Setq => setq(expander, kind, form),
        SpecialForm::NlxProtect => nlx_protect(expander, kind, form),
        _ => primitive(expander, kind, form),
    }
}

/// `(function name)` or `(function (lambda ...))`.
fn function_form(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let [argument] = arguments.as_slice() else {
        return Err(special::arity(
            kind,
            "one function designator",
            arguments.len(),
        ));
    };
    if argument.is_cons() {
        let head = car(expander.ctx(), *argument)?;
        if expander.is_named(head, "LAMBDA")? {
            let lambda = expander.expand_lambda(*argument)?;
            return Ok(Expr::Function(FunctionDesignator::Lambda(Box::new(lambda))));
        }
        return Err(FrontError::MalformedForm {
            operator: kind.symbol(),
            detail: "a compound function name has no AST representation".to_owned(),
        });
    }
    let name = expander.symbol(*argument)?;
    Ok(Expr::Function(FunctionDesignator::Name(name)))
}

/// `(quote datum)`.
fn quote(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let [argument] = arguments.as_slice() else {
        return Err(special::arity(kind, "one datum", arguments.len()));
    };
    Ok(Expr::Constant(expander.datum(*argument)?))
}

/// `(the type form)`, and `(sb-ext:truly-the type form)`.
fn the(expander: &mut FormExpander<'_>, kind: SpecialForm, form: Word) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let [type_form, value] = arguments.as_slice() else {
        return Err(special::arity(kind, "a type and a form", arguments.len()));
    };
    let type_specifier = TypeSpecifier::new(expander.datum(*type_form)?);
    let value = expander.expand(*value)?;
    Ok(Expr::The {
        type_specifier,
        value: Box::new(value),
    })
}

/// `(multiple-value-call function form*)`.
fn multiple_value_call(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let Some((function, rest)) = arguments.split_first() else {
        return Err(special::arity(kind, "a function form", 0));
    };
    let function = expander.expand(*function)?;
    let arguments = expander.expand_all(rest)?;
    Ok(Expr::MultipleValueCall {
        function: Box::new(function),
        arguments,
    })
}

/// `(multiple-value-prog1 first form*)`.
fn multiple_value_prog1(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let Some((first, rest)) = arguments.split_first() else {
        return Err(special::arity(kind, "a first form", 0));
    };
    let first = expander.expand(*first)?;
    let forms = expander.expand_all(rest)?;
    Ok(Expr::MultipleValueProg1 {
        first: Box::new(first),
        forms,
    })
}

/// `(setq place value*)`.
fn setq(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    if arguments.is_empty() || arguments.len() % 2 != 0 {
        return Err(special::arity(
            kind,
            "an even number of place and value forms",
            arguments.len(),
        ));
    }
    let mut pairs = Vec::with_capacity(arguments.len() / 2);
    for pair in arguments.chunks(2) {
        let [place, value] = pair else {
            return Err(special::arity(kind, "an even number of forms", pair.len()));
        };
        let name = expander.symbol(*place)?;
        if expander.env().lookup_symbol_macro(&name).is_some() {
            return Err(FrontError::MalformedForm {
                operator: kind.symbol(),
                detail: format!("{name} is a symbol macro and cannot be assigned"),
            });
        }
        let value = expander.expand(*value)?;
        pairs.push((name, value));
    }
    Ok(Expr::Setq(pairs))
}

/// `(sb-sys:nlx-protect protected cleanup*)`.
///
/// The front end approximates it as `unwind-protect`; the distinction between
/// a cleanup that runs on every exit and one that runs only on a non-local exit
/// is not represented in the frozen AST.
fn nlx_protect(
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

/// `(sb-sys:%primitive name arg*)`.
///
/// The frozen AST has no node that distinguishes a runtime builtin call from an
/// ordinary call, so the form is reported as a gap rather than lowered to a
/// shape the lowering lane would misread.
fn primitive(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let _ = special::arguments(expander, kind, form)?;
    Err(FrontError::MalformedForm {
        operator: kind.symbol(),
        detail: "no AST representation for sb-sys:%primitive yet".to_owned(),
    })
}
