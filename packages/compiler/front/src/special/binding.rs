//! Binding special operators: `let`, `let*`, `progv`, `flet`, `labels`,
//! `macrolet`, and `symbol-macrolet`.

use ncl_object::Word;

use crate::ast::{Expr, LetBinding, LocalFunction, LocalMacro, SymbolMacro};
use crate::error::FrontError;
use crate::expand::{FormExpander, LambdaListKind};
use crate::special::{self, SpecialForm};
use crate::symbols::SymbolRef;

/// Parse one of the binding special operators.
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
        SpecialForm::Let | SpecialForm::LetStar => let_form(expander, kind, form),
        SpecialForm::Progv => progv(expander, kind, form),
        SpecialForm::Flet => local_functions(expander, kind, form, false),
        SpecialForm::Labels => local_functions(expander, kind, form, true),
        SpecialForm::Macrolet => macrolet(expander, kind, form),
        _ => symbol_macrolet(expander, kind, form),
    }
}

/// `(let binding* declaration* form*)` or `let*`.
fn let_form(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let sequential = kind == SpecialForm::LetStar;
    let arguments = special::arguments(expander, kind, form)?;
    let Some((bindings_word, body_forms)) = arguments.split_first() else {
        return Err(special::arity(kind, "a binding list", 0));
    };
    let specifications = expander.elements(*bindings_word)?;
    expander.env_mut().push_scope();
    let mut bindings = Vec::with_capacity(specifications.len());
    if sequential {
        for specification in &specifications {
            let (name, value) = binding(expander, kind, *specification)?;
            let value = value.map(|value| expander.expand(value)).transpose()?;
            expander.env_mut().bind_variable(name.clone(), None);
            bindings.push(LetBinding { name, value });
        }
    } else {
        for specification in &specifications {
            let (name, value) = binding(expander, kind, *specification)?;
            let value = value.map(|value| expander.expand(value)).transpose()?;
            bindings.push(LetBinding { name, value });
        }
        for bound in &bindings {
            expander.env_mut().bind_variable(bound.name.clone(), None);
        }
    }
    let body = expander.expand_declared_body(body_forms)?;
    expander.apply_declarations(&body.declarations);
    expander.env_mut().pop_scope();
    Ok(Expr::Let {
        sequential,
        bindings,
        declarations: body.declarations,
        body: body.forms,
    })
}

/// One `let` binding: a symbol, or a `(symbol [init])` list.
fn binding(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    specification: Word,
) -> Result<(SymbolRef, Option<Word>), FrontError> {
    if !specification.is_cons() {
        return Ok((expander.symbol(specification)?, None));
    }
    let elements = expander.elements(specification)?;
    let Some((name, rest)) = elements.split_first() else {
        return Err(FrontError::MalformedForm {
            operator: kind.symbol(),
            detail: "empty binding".to_owned(),
        });
    };
    if rest.len() > 1 {
        return Err(FrontError::MalformedForm {
            operator: kind.symbol(),
            detail: "a binding takes a name and at most one init form".to_owned(),
        });
    }
    Ok((expander.symbol(*name)?, rest.first().copied()))
}

/// `(progv symbols values form*)`.
fn progv(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    if arguments.len() < 2 {
        return Err(special::arity(kind, "symbols and values", arguments.len()));
    }
    let symbols = expander.expand(arguments[0])?;
    let values = expander.expand(arguments[1])?;
    let body = expander.expand_all(&arguments[2..])?;
    Ok(Expr::Progv {
        symbols: Box::new(symbols),
        values: Box::new(values),
        body,
    })
}

/// `(flet definition* declaration* form*)`, or `labels`.
fn local_functions(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
    recursive: bool,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let Some((definitions_word, body_forms)) = arguments.split_first() else {
        return Err(special::arity(kind, "a definition list", 0));
    };
    let definition_forms = expander.elements(*definitions_word)?;
    expander.env_mut().push_scope();
    if recursive {
        for definition in &definition_forms {
            let name = definition_name(expander, kind, *definition)?;
            expander.env_mut().bind_function(name);
        }
    }
    let mut definitions = Vec::with_capacity(definition_forms.len());
    for definition in &definition_forms {
        let (name, lambda) = local_function(expander, kind, *definition)?;
        if !recursive {
            expander.env_mut().bind_function(name.clone());
        }
        definitions.push(LocalFunction { name, lambda });
    }
    let body = expander.expand_declared_body(body_forms)?;
    expander.apply_declarations(&body.declarations);
    expander.env_mut().pop_scope();
    let declarations = body.declarations;
    let body = body.forms;
    Ok(if recursive {
        Expr::Labels {
            definitions,
            declarations,
            body,
        }
    } else {
        Expr::Flet {
            definitions,
            declarations,
            body,
        }
    })
}

/// `(macrolet definition* declaration* form*)`.
fn macrolet(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let Some((definitions_word, body_forms)) = arguments.split_first() else {
        return Err(special::arity(kind, "a definition list", 0));
    };
    let definition_forms = expander.elements(*definitions_word)?;
    expander.env_mut().push_scope();
    let mut definitions = Vec::with_capacity(definition_forms.len());
    for definition in &definition_forms {
        let elements = expander.elements(*definition)?;
        let Some((name, rest)) = elements.split_first() else {
            return Err(FrontError::MalformedForm {
                operator: kind.symbol(),
                detail: "empty macrolet definition".to_owned(),
            });
        };
        let name = expander.symbol(*name)?;
        let Some((lambda_list_form, expander_forms)) = rest.split_first() else {
            return Err(special::arity(kind, "a macro lambda list", 0));
        };
        let lambda = expander.expand_lambda_body(
            *lambda_list_form,
            expander_forms,
            LambdaListKind::Macro,
        )?;
        let local = LocalMacro {
            name: name.clone(),
            lambda_list: lambda.lambda_list,
            declarations: lambda.declarations,
            docstring: lambda.docstring,
            body: lambda.body,
        };
        expander.env_mut().bind_macro(local.clone());
        definitions.push(local);
    }
    let body = expander.expand_declared_body(body_forms)?;
    expander.apply_declarations(&body.declarations);
    expander.env_mut().pop_scope();
    Ok(Expr::Macrolet {
        definitions,
        declarations: body.declarations,
        body: body.forms,
    })
}

/// `(symbol-macrolet definition* declaration* form*)`.
fn symbol_macrolet(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let Some((definitions_word, body_forms)) = arguments.split_first() else {
        return Err(special::arity(kind, "a definition list", 0));
    };
    let definition_forms = expander.elements(*definitions_word)?;
    let mut definitions = Vec::with_capacity(definition_forms.len());
    for definition in &definition_forms {
        let elements = expander.elements(*definition)?;
        let [name, expansion] = elements.as_slice() else {
            return Err(FrontError::MalformedForm {
                operator: kind.symbol(),
                detail: "a symbol macro takes a name and one expansion".to_owned(),
            });
        };
        let name = expander.symbol(*name)?;
        let expansion = expander.expand(*expansion)?;
        definitions.push(SymbolMacro { name, expansion });
    }
    expander.env_mut().push_scope();
    for definition in &definitions {
        expander.env_mut().bind_symbol_macro(definition.clone());
    }
    let body = expander.expand_declared_body(body_forms)?;
    expander.apply_declarations(&body.declarations);
    expander.env_mut().pop_scope();
    Ok(Expr::SymbolMacrolet {
        definitions,
        declarations: body.declarations,
        body: body.forms,
    })
}

/// The name of a local function definition.
fn definition_name(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    definition: Word,
) -> Result<SymbolRef, FrontError> {
    let elements = expander.elements(definition)?;
    let Some((name, _)) = elements.split_first() else {
        return Err(FrontError::MalformedForm {
            operator: kind.symbol(),
            detail: "empty local function definition".to_owned(),
        });
    };
    if !matches!(
        ncl_object::classify_object(expander.ctx(), *name),
        ncl_object::ObjectRef::Symbol(_)
    ) {
        return Err(FrontError::MalformedForm {
            operator: kind.symbol(),
            detail: "a local function name must be a symbol".to_owned(),
        });
    }
    expander.symbol(*name)
}

/// One local function definition: a name and a lambda body.
fn local_function(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    definition: Word,
) -> Result<(SymbolRef, crate::ast::LambdaExpr), FrontError> {
    let elements = expander.elements(definition)?;
    let Some((name, rest)) = elements.split_first() else {
        return Err(FrontError::MalformedForm {
            operator: kind.symbol(),
            detail: "empty local function definition".to_owned(),
        });
    };
    let name = expander.symbol(*name)?;
    let Some((lambda_list_form, body_forms)) = rest.split_first() else {
        return Err(special::arity(kind, "a lambda list", 0));
    };
    let lambda =
        expander.expand_lambda_body(*lambda_list_form, body_forms, LambdaListKind::Ordinary)?;
    Ok((name, lambda))
}
