//! Binding special operators: `let`, `let*`, `progv`, `flet`, `labels`,
//! `macrolet`, and `symbol-macrolet`.

use ncl_object::{HeapRootedWord, Word, symbol_is_special};

use crate::ast::{Expr, LetBinding, LocalFunction, LocalMacro, SymbolMacro};
use crate::declaration::Declaration;
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
    let specifications = expander.elements(bindings_word.get())?;
    expander.env_mut().push_scope();
    let mut bindings = Vec::with_capacity(specifications.len());
    // Bindings whose symbol is already proclaimed special (by `defvar`,
    // `defparameter`, `declaim`, or `proclaim`) before this `let`/`let*` is
    // expanded. These are collected here, at expand time, because only the
    // expander has access to the live symbol table; the lowering lane (which
    // has no such access) re-derives special-ness purely from the
    // declarations an `Expr::Let` carries, so a synthesized
    // `Declaration::Special` is how that fact crosses the boundary.
    let mut global_specials = Vec::new();
    if sequential {
        for specification in specifications.iter() {
            let (name, is_special, value) = binding(expander, kind, specification.get())?;
            let value = value
                .map(|value| expander.expand(value.get()))
                .transpose()?;
            if is_special {
                global_specials.push(name.clone());
                expander.env_mut().bind_special(name.clone());
            } else {
                expander.env_mut().bind_variable(name.clone(), None);
            }
            bindings.push(LetBinding { name, value });
        }
    } else {
        for specification in specifications.iter() {
            let (name, is_special, value) = binding(expander, kind, specification.get())?;
            let value = value
                .map(|value| expander.expand(value.get()))
                .transpose()?;
            if is_special {
                global_specials.push(name.clone());
            }
            bindings.push(LetBinding { name, value });
        }
        for bound in &bindings {
            if global_specials.contains(&bound.name) {
                expander.env_mut().bind_special(bound.name.clone());
            } else {
                expander.env_mut().bind_variable(bound.name.clone(), None);
            }
        }
    }
    let body = expander.expand_rooted_declared_body(body_forms)?;
    expander.apply_declarations(&body.declarations);
    expander.env_mut().pop_scope();
    let mut declarations = body.declarations;
    if !global_specials.is_empty() {
        declarations.push(Declaration::Special(global_specials));
    }
    Ok(Expr::Let {
        sequential,
        bindings,
        declarations,
        body: body.forms,
    })
}

/// One `let` binding: a symbol, or a `(symbol [init])` list.
///
/// Returns the binding's name, whether it is globally proclaimed special, and
/// its rooted init form, if any.
fn binding(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    specification: Word,
) -> Result<(SymbolRef, bool, Option<HeapRootedWord>), FrontError> {
    if !specification.is_cons() {
        let is_special = symbol_is_special(expander.ctx(), specification).unwrap_or(false);
        let name = expander.symbol(specification)?;
        return Ok((name, is_special, None));
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
    let is_special = symbol_is_special(expander.ctx(), name.get()).unwrap_or(false);
    let name = expander.symbol(name.get())?;
    let init_form = rest
        .first()
        .map(|init| HeapRootedWord::new(expander.ctx(), init.get()))
        .transpose()?;
    Ok((name, is_special, init_form))
}

/// `(progv symbols values form*)`.
fn progv(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Expr, FrontError> {
    let arguments = special::arguments(expander, kind, form)?;
    let [symbols, values, body_forms @ ..] = arguments.as_slice() else {
        return Err(special::arity(kind, "symbols and values", arguments.len()));
    };
    let symbols = expander.expand(symbols.get())?;
    let values = expander.expand(values.get())?;
    let body = expander.expand_rooted_all(body_forms)?;
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
    let definition_forms = expander.elements(definitions_word.get())?;
    expander.env_mut().push_scope();
    if recursive {
        for definition in definition_forms.iter() {
            let name = definition_name(expander, kind, definition.get())?;
            expander.env_mut().bind_function(name);
        }
    }
    let mut definitions = Vec::with_capacity(definition_forms.len());
    for definition in definition_forms.iter() {
        let (name, lambda) = local_function(expander, kind, definition.get())?;
        if !recursive {
            expander.env_mut().bind_function(name.clone());
        }
        definitions.push(LocalFunction { name, lambda });
    }
    let body = expander.expand_rooted_declared_body(body_forms)?;
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
    let definition_forms = expander.elements(definitions_word.get())?;
    expander.env_mut().push_scope();
    let mut definitions = Vec::with_capacity(definition_forms.len());
    for definition in definition_forms.iter() {
        let elements = expander.elements(definition.get())?;
        let Some((name, rest)) = elements.split_first() else {
            return Err(FrontError::MalformedForm {
                operator: kind.symbol(),
                detail: "empty macrolet definition".to_owned(),
            });
        };
        let name = expander.symbol(name.get())?;
        let Some((lambda_list_form, expander_forms)) = rest.split_first() else {
            return Err(special::arity(kind, "a macro lambda list", 0));
        };
        let lambda = expander.expand_rooted_lambda_body(
            lambda_list_form,
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
    let body = expander.expand_rooted_declared_body(body_forms)?;
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
    let definition_forms = expander.elements(definitions_word.get())?;
    let mut definitions = Vec::with_capacity(definition_forms.len());
    let mut source_forms = Vec::with_capacity(definition_forms.len());
    for definition in definition_forms.iter() {
        let elements = expander.elements(definition.get())?;
        let [name, expansion] = elements.as_slice() else {
            return Err(FrontError::MalformedForm {
                operator: kind.symbol(),
                detail: "a symbol macro takes a name and one expansion".to_owned(),
            });
        };
        let name = expander.symbol(name.get())?;
        let expanded = expander.expand(expansion.get())?;
        source_forms.push(expansion.get());
        definitions.push(SymbolMacro {
            name,
            expansion: expanded,
        });
    }
    expander.env_mut().push_scope();
    for (definition, source) in definitions.iter().zip(source_forms) {
        expander
            .env_mut()
            .bind_symbol_macro_with_source(definition.clone(), source);
    }
    let body = expander.expand_rooted_declared_body(body_forms)?;
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
        crate::form::classify_form(expander.ctx(), name.get()),
        ncl_object::ObjectRef::Symbol(_)
    ) {
        return Err(FrontError::MalformedForm {
            operator: kind.symbol(),
            detail: "a local function name must be a symbol".to_owned(),
        });
    }
    expander.symbol(name.get())
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
    let name = expander.symbol(name.get())?;
    let Some((lambda_list_form, body_forms)) = rest.split_first() else {
        return Err(special::arity(kind, "a lambda list", 0));
    };
    let lambda = expander.expand_rooted_lambda_body(
        lambda_list_form,
        body_forms,
        LambdaListKind::Ordinary,
    )?;
    Ok((name, lambda))
}
