//! Parsers for the 25 Common Lisp special operators.
//!
//! `expand` dispatches here after macroexpansion. Each submodule turns one
//! group of operators into [`Expr`] nodes.

use ncl_object::Word;

use crate::ast::Expr;
use crate::error::FrontError;
use crate::expand::FormExpander;
use crate::symbols::SymbolRef;

pub mod binding;
pub mod control;
pub mod function;

/// A special operator the front end parses.
///
/// The first 25 variants are the Common Lisp special operators listed by CLHS
/// 3.1.2.1.2.1. The last three are the SBCL extensions the ownership table
/// assigns to this crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SpecialForm {
    /// `block`.
    Block,
    /// `catch`.
    Catch,
    /// `eval-when`.
    EvalWhen,
    /// `flet`.
    Flet,
    /// `function`.
    Function,
    /// `go`.
    Go,
    /// `if`.
    If,
    /// `labels`.
    Labels,
    /// `let`.
    Let,
    /// `let*`.
    LetStar,
    /// `load-time-value`.
    LoadTimeValue,
    /// `locally`.
    Locally,
    /// `macrolet`.
    Macrolet,
    /// `multiple-value-call`.
    MultipleValueCall,
    /// `multiple-value-prog1`.
    MultipleValueProg1,
    /// `progn`.
    Progn,
    /// `progv`.
    Progv,
    /// `quote`.
    Quote,
    /// `return-from`.
    ReturnFrom,
    /// `setq`.
    Setq,
    /// `symbol-macrolet`.
    SymbolMacrolet,
    /// `tagbody`.
    Tagbody,
    /// `the`.
    The,
    /// `throw`.
    Throw,
    /// `unwind-protect`.
    UnwindProtect,
    /// `sb-sys:%primitive`.
    Primitive,
    /// `sb-sys:nlx-protect`.
    NlxProtect,
    /// `sb-ext:truly-the`.
    TrulyThe,
}

impl SpecialForm {
    /// Resolve an operator symbol to the special form it names.
    ///
    /// A Common Lisp special operator is recognised by name in `COMMON-LISP` or
    /// in no package; the three extensions require their own package.
    #[must_use]
    pub fn from_symbol(name: &SymbolRef) -> Option<Self> {
        let common_lisp = name.package.is_none() || name.package.as_deref() == Some("COMMON-LISP");
        if common_lisp {
            return common_lisp_form(&name.name);
        }
        match (name.package.as_deref(), name.name.as_str()) {
            (Some("SB-SYS"), "%PRIMITIVE") => Some(Self::Primitive),
            (Some("SB-SYS"), "NLX-PROTECT") => Some(Self::NlxProtect),
            (Some("SB-EXT"), "TRULY-THE") => Some(Self::TrulyThe),
            _ => None,
        }
    }

    /// The operator's printed name, used in diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Catch => "catch",
            Self::EvalWhen => "eval-when",
            Self::Flet => "flet",
            Self::Function => "function",
            Self::Go => "go",
            Self::If => "if",
            Self::Labels => "labels",
            Self::Let => "let",
            Self::LetStar => "let*",
            Self::LoadTimeValue => "load-time-value",
            Self::Locally => "locally",
            Self::Macrolet => "macrolet",
            Self::MultipleValueCall => "multiple-value-call",
            Self::MultipleValueProg1 => "multiple-value-prog1",
            Self::Progn => "progn",
            Self::Progv => "progv",
            Self::Quote => "quote",
            Self::ReturnFrom => "return-from",
            Self::Setq => "setq",
            Self::SymbolMacrolet => "symbol-macrolet",
            Self::Tagbody => "tagbody",
            Self::The => "the",
            Self::Throw => "throw",
            Self::UnwindProtect => "unwind-protect",
            Self::Primitive => "%primitive",
            Self::NlxProtect => "nlx-protect",
            Self::TrulyThe => "truly-the",
        }
    }

    /// The symbol naming this operator, for diagnostics.
    #[must_use]
    pub fn symbol(self) -> SymbolRef {
        match self {
            Self::Primitive | Self::NlxProtect => SymbolRef::interned("SB-SYS", self.name()),
            Self::TrulyThe => SymbolRef::interned("SB-EXT", self.name()),
            _ => SymbolRef::interned("COMMON-LISP", self.name()),
        }
    }
}

/// Resolve one of the 25 standard names.
fn common_lisp_form(name: &str) -> Option<SpecialForm> {
    Some(match name {
        "BLOCK" => SpecialForm::Block,
        "CATCH" => SpecialForm::Catch,
        "EVAL-WHEN" => SpecialForm::EvalWhen,
        "FLET" => SpecialForm::Flet,
        "FUNCTION" => SpecialForm::Function,
        "GO" => SpecialForm::Go,
        "IF" => SpecialForm::If,
        "LABELS" => SpecialForm::Labels,
        "LET" => SpecialForm::Let,
        "LET*" => SpecialForm::LetStar,
        "LOAD-TIME-VALUE" => SpecialForm::LoadTimeValue,
        "LOCALLY" => SpecialForm::Locally,
        "MACROLET" => SpecialForm::Macrolet,
        "MULTIPLE-VALUE-CALL" => SpecialForm::MultipleValueCall,
        "MULTIPLE-VALUE-PROG1" => SpecialForm::MultipleValueProg1,
        "PROGN" => SpecialForm::Progn,
        "PROGV" => SpecialForm::Progv,
        "QUOTE" => SpecialForm::Quote,
        "RETURN-FROM" => SpecialForm::ReturnFrom,
        "SETQ" => SpecialForm::Setq,
        "SYMBOL-MACROLET" => SpecialForm::SymbolMacrolet,
        "TAGBODY" => SpecialForm::Tagbody,
        "THE" => SpecialForm::The,
        "THROW" => SpecialForm::Throw,
        "UNWIND-PROTECT" => SpecialForm::UnwindProtect,
        _ => return None,
    })
}

/// Parse a special form into an [`Expr`].
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
        SpecialForm::Block
        | SpecialForm::ReturnFrom
        | SpecialForm::Tagbody
        | SpecialForm::Go
        | SpecialForm::Catch
        | SpecialForm::Throw
        | SpecialForm::UnwindProtect
        | SpecialForm::If
        | SpecialForm::Progn
        | SpecialForm::Locally
        | SpecialForm::EvalWhen
        | SpecialForm::LoadTimeValue => control::parse(expander, kind, form),
        SpecialForm::Let
        | SpecialForm::LetStar
        | SpecialForm::Progv
        | SpecialForm::Flet
        | SpecialForm::Labels
        | SpecialForm::Macrolet
        | SpecialForm::SymbolMacrolet => binding::parse(expander, kind, form),
        SpecialForm::Function
        | SpecialForm::Quote
        | SpecialForm::The
        | SpecialForm::MultipleValueCall
        | SpecialForm::MultipleValueProg1
        | SpecialForm::Setq
        | SpecialForm::Primitive
        | SpecialForm::NlxProtect
        | SpecialForm::TrulyThe => function::parse(expander, kind, form),
    }
}

/// Split a special form into its argument forms.
///
/// # Errors
///
/// Returns [`FrontError::MalformedForm`] when the form is not a non-empty list.
pub(crate) fn arguments(
    expander: &mut FormExpander<'_>,
    kind: SpecialForm,
    form: Word,
) -> Result<Vec<Word>, FrontError> {
    let elements = expander.elements(form)?;
    let Some((_, rest)) = elements.split_first() else {
        return Err(FrontError::MalformedForm {
            operator: kind.symbol(),
            detail: "empty special form".to_owned(),
        });
    };
    Ok(rest.to_vec())
}

/// Build a wrong-argument-count failure for a special form.
pub(crate) fn arity(kind: SpecialForm, expected: &'static str, found: usize) -> FrontError {
    FrontError::WrongNumberOfForms {
        operator: kind.symbol(),
        expected,
        found,
    }
}
