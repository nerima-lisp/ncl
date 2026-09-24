//! Turning reader forms into the internal AST.
//!
//! The expander walks a `Word` form, resolves macros, and produces the frozen
//! AST. Macroexpansion goes through [`MacroCaller`]; the expander never
//! reverse-depends on the runtime.

use ncl_object::{ObjectRef, Runtime, ThreadContext, Word, car};

use crate::ast::{Expr, LambdaExpr};
use crate::compiler_macro::MacroRegistry;
use crate::declaration::{Declaration, parse_declare_form};
use crate::env::LexicalEnv;
use crate::error::FrontError;
use crate::form::{self, UninternedTable, classify_form};
use crate::lambda_list::{LambdaList, ParamName};
use crate::literal::Literal;
use crate::macro_caller::MacroCaller;
use crate::symbols::SymbolRef;

mod call;
mod parameters;

use call::Step;

/// How many times one form is re-expanded before the expander gives up.
const MACROEXPANSION_LIMIT: usize = 256;

/// How many symbol macros one reference follows before the expander stops.
pub(super) const SYMBOL_MACRO_LIMIT: usize = 64;

/// Which lambda list syntax to parse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LambdaListKind {
    /// An ordinary lambda list, as used by `lambda`, `flet`, and `labels`.
    Ordinary,
    /// A macro lambda list, as used by `macrolet`.
    Macro,
}

/// A body: declarations, an optional docstring, and forms.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    /// The declarations that opened the body.
    pub declarations: Vec<Declaration>,
    /// The documentation string, if the body opened with one.
    pub docstring: Option<String>,
    /// The body forms.
    pub forms: Vec<Expr>,
}

/// Expansion state for one compilation unit.
pub struct FormExpander<'a> {
    ctx: &'a mut ThreadContext,
    runtime: &'a Runtime,
    caller: Option<&'a mut dyn MacroCaller>,
    registry: &'a MacroRegistry,
    env: LexicalEnv,
    uninterned: UninternedTable,
}

impl std::fmt::Debug for FormExpander<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FormExpander")
            .field("has_caller", &self.caller.is_some())
            .field("environment", &self.env)
            .finish_non_exhaustive()
    }
}

impl<'a> FormExpander<'a> {
    /// Create an expander over one compilation unit.
    #[must_use]
    pub fn new(
        ctx: &'a mut ThreadContext,
        runtime: &'a Runtime,
        registry: &'a MacroRegistry,
    ) -> Self {
        Self {
            ctx,
            runtime,
            caller: None,
            registry,
            env: LexicalEnv::new(),
            uninterned: UninternedTable::new(),
        }
    }

    /// Install the macro caller used for global and local macro calls.
    pub fn set_caller(&mut self, caller: &'a mut dyn MacroCaller) {
        self.caller = Some(caller);
    }

    /// The object-layer context.
    #[must_use]
    pub const fn ctx(&mut self) -> &mut ThreadContext {
        self.ctx
    }

    /// The shared runtime.
    #[must_use]
    pub const fn runtime(&self) -> &Runtime {
        self.runtime
    }

    /// The lexical environment.
    #[must_use]
    pub const fn env(&self) -> &LexicalEnv {
        &self.env
    }

    /// The lexical environment, mutably.
    pub const fn env_mut(&mut self) -> &mut LexicalEnv {
        &mut self.env
    }

    /// Expand one form, following macro expansions to a fixed point.
    ///
    /// # Errors
    ///
    /// Returns a [`FrontError`] when the form is malformed, when a macro cannot
    /// be called, or when expansion exceeds `MACROEXPANSION_LIMIT`.
    pub fn expand(&mut self, form: Word) -> Result<Expr, FrontError> {
        let mut current = form;
        let mut remaining = MACROEXPANSION_LIMIT;
        loop {
            match self.expand_step(current)? {
                Step::Done(expr) => return Ok(expr),
                Step::Retry { name, form } => {
                    if remaining == 0 {
                        return Err(FrontError::MacroExpansion {
                            name,
                            detail: "expansion limit exceeded".to_owned(),
                        });
                    }
                    remaining -= 1;
                    current = form;
                }
            }
        }
    }

    /// Expand every form of a sequence.
    ///
    /// # Errors
    ///
    /// Returns the first [`FrontError`] raised by any form.
    pub fn expand_all(&mut self, forms: &[Word]) -> Result<Vec<Expr>, FrontError> {
        forms.iter().map(|form| self.expand(*form)).collect()
    }

    /// Split the leading `(declare ...)` forms from a body.
    ///
    /// Returns the declarations and the index of the first body form, so a
    /// caller can apply the declarations to the environment before expanding.
    ///
    /// # Errors
    ///
    /// Returns a [`FrontError`] when a declaration is malformed.
    pub fn split_declarations(
        &mut self,
        forms: &[Word],
    ) -> Result<(Vec<Declaration>, usize), FrontError> {
        let mut index = 0;
        let mut declarations = Vec::new();
        while index < forms.len() && self.is_declare_form(forms[index])? {
            declarations.extend(self.declarations(forms[index])?);
            index += 1;
        }
        Ok((declarations, index))
    }

    /// Expand a body: leading declarations, an optional docstring, then forms.
    ///
    /// # Errors
    ///
    /// Returns a [`FrontError`] when a declaration or a form is malformed.
    pub fn expand_body(&mut self, forms: &[Word]) -> Result<Body, FrontError> {
        let (declarations, mut index) = self.split_declarations(forms)?;
        let mut docstring = None;
        if let Some(word) = forms.get(index)
            && let Some(text) = self.docstring(*word)?
        {
            docstring = Some(text);
            index += 1;
        }
        let expanded = self.expand_all(&forms[index..])?;
        Ok(Body {
            declarations,
            docstring,
            forms: expanded,
        })
    }

    /// Expand a body that has no docstring position, such as a `let` body.
    ///
    /// # Errors
    ///
    /// Returns a [`FrontError`] when a declaration or a form is malformed.
    pub fn expand_declared_body(&mut self, forms: &[Word]) -> Result<Body, FrontError> {
        let (declarations, index) = self.split_declarations(forms)?;
        let expanded = self.expand_all(&forms[index..])?;
        Ok(Body {
            declarations,
            docstring: None,
            forms: expanded,
        })
    }

    /// Record the declarations that affect the lexical environment.
    ///
    /// `special` declarations make later references dynamic; `type` declarations
    /// record the declared type. Other specifiers are recorded in the AST by the
    /// caller and do not change the environment.
    pub fn apply_declarations(&mut self, declarations: &[Declaration]) {
        for declaration in declarations {
            match declaration {
                Declaration::Special(names) => {
                    for name in names {
                        self.env.bind_special(name.clone());
                    }
                }
                Declaration::Type {
                    type_specifier,
                    names,
                } => {
                    for name in names {
                        self.env.declare_type(name.clone(), type_specifier.clone());
                    }
                }
                _ => {}
            }
        }
    }

    /// Expand a `(lambda lambda-list body)` form.
    ///
    /// # Errors
    ///
    /// Returns a [`FrontError`] when the form is not a lambda expression or its
    /// lambda list is malformed.
    pub fn expand_lambda(&mut self, form: Word) -> Result<LambdaExpr, FrontError> {
        let elements = self.elements(form)?;
        let Some((head, rest)) = elements.split_first() else {
            return Err(FrontError::InvalidOperator {
                detail: "empty lambda expression".to_owned(),
            });
        };
        if !self.is_named(*head, "LAMBDA")? {
            return Err(FrontError::InvalidOperator {
                detail: "lambda expression does not start with lambda".to_owned(),
            });
        }
        let Some((lambda_list_form, body_forms)) = rest.split_first() else {
            return Err(FrontError::WrongNumberOfForms {
                operator: self.symbol(*head)?,
                expected: "a lambda list",
                found: 0,
            });
        };
        let lambda_list = self.expand_lambda_list(*lambda_list_form, LambdaListKind::Ordinary)?;
        self.env.push_scope();
        self.bind_parameters(&lambda_list);
        let body = self.expand_body(body_forms)?;
        self.apply_declarations(&body.declarations);
        self.env.pop_scope();
        Ok(LambdaExpr {
            lambda_list,
            declarations: body.declarations,
            docstring: body.docstring,
            body: body.forms,
        })
    }

    /// Expand a lambda list and body into a lambda expression.
    ///
    /// This is the shared body of `lambda`, `flet`/`labels` definitions, and
    /// `macrolet` expanders; the caller supplies the lambda-list kind.
    ///
    /// # Errors
    ///
    /// Returns a [`FrontError`] when the lambda list or a body form is
    /// malformed.
    pub fn expand_lambda_body(
        &mut self,
        lambda_list_form: Word,
        body_forms: &[Word],
        kind: LambdaListKind,
    ) -> Result<LambdaExpr, FrontError> {
        let lambda_list = self.expand_lambda_list(lambda_list_form, kind)?;
        self.env.push_scope();
        self.bind_parameters(&lambda_list);
        let body = self.expand_body(body_forms)?;
        self.apply_declarations(&body.declarations);
        self.env.pop_scope();
        Ok(LambdaExpr {
            lambda_list,
            declarations: body.declarations,
            docstring: body.docstring,
            body: body.forms,
        })
    }

    /// Convert a form into a [`Literal`] datum.
    ///
    /// # Errors
    ///
    /// Returns a [`FrontError`] when the object is not representable.
    pub fn datum(&mut self, word: Word) -> Result<Literal, FrontError> {
        form::literal(self.ctx, &mut self.uninterned, word)
    }

    /// Convert a symbol word into a [`SymbolRef`].
    ///
    /// # Errors
    ///
    /// Returns a [`FrontError`] when the word is not a symbol.
    pub fn symbol(&mut self, word: Word) -> Result<SymbolRef, FrontError> {
        form::symbol_ref(self.ctx, &mut self.uninterned, word)
    }

    /// Collect the elements of a proper list.
    ///
    /// # Errors
    ///
    /// Returns a [`FrontError`] when the list is improper.
    pub fn elements(&mut self, word: Word) -> Result<Vec<Word>, FrontError> {
        form::list(self.ctx, word)
    }

    /// Parse a `(declare ...)` form.
    ///
    /// # Errors
    ///
    /// Returns a [`FrontError`] when the form is not a well-formed declaration.
    pub fn declarations(&mut self, form: Word) -> Result<Vec<Declaration>, FrontError> {
        let datum = self.datum(form)?;
        parse_declare_form(&datum)
    }

    /// Whether `word` is the symbol `name`.
    ///
    /// # Errors
    ///
    /// Returns a [`FrontError`] when the object layer rejects a symbol.
    pub fn is_named(&mut self, word: Word, name: &str) -> Result<bool, FrontError> {
        if !matches!(classify_form(self.ctx, word), ObjectRef::Symbol(_)) {
            return Ok(false);
        }
        Ok(self.symbol(word)?.name == name)
    }

    /// Bind every parameter name of a lambda list as a lexical variable.
    pub fn bind_parameters(&mut self, lambda_list: &LambdaList) {
        for name in &lambda_list.required {
            self.bind_parameter(name);
        }
        for optional in &lambda_list.optional {
            self.bind_parameter(&optional.name);
            if let Some(supplied) = &optional.supplied_p {
                self.bind_parameter(supplied);
            }
        }
        if let Some(rest) = &lambda_list.rest {
            self.bind_parameter(rest);
        }
        for key in &lambda_list.keys {
            self.bind_parameter(&key.name);
            if let Some(supplied) = &key.supplied_p {
                self.bind_parameter(supplied);
            }
        }
        for aux in &lambda_list.aux {
            self.bind_parameter(&aux.name);
        }
        if let Some(body) = &lambda_list.body {
            self.bind_parameter(body);
        }
    }

    /// Whether a form is a `(declare ...)` form.
    fn is_declare_form(&mut self, form: Word) -> Result<bool, FrontError> {
        if !form.is_cons() {
            return Ok(false);
        }
        let head = car(self.ctx, form)?;
        self.is_named(head, "DECLARE")
    }

    /// Read a leading docstring, if the word is a string.
    fn docstring(&self, word: Word) -> Result<Option<String>, FrontError> {
        match classify_form(self.ctx, word) {
            ObjectRef::String(_) => Ok(Some(form::word_string(self.ctx, word)?)),
            _ => Ok(None),
        }
    }

    /// Bind one parameter name, if it is a symbol rather than a pattern.
    fn bind_parameter(&mut self, name: &ParamName) {
        if let ParamName::Symbol(symbol) = name {
            self.env.bind_variable(symbol.clone(), None);
        }
    }
}
