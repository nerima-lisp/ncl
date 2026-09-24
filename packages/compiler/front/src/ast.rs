//! The internal AST consumed by the lowering lane.
//!
//! Every node is a pure Rust value and stores no `ncl-object` `Word`, so a tree
//! outlives the heap activity that produced it. The tree covers the 25 Common
//! Lisp special operators plus function calls, variable references, lambda
//! expressions, and declarations.
//!
//! `quote` and self-evaluating atoms both parse to [`Expr::Constant`]; a
//! lowering lane loads a constant for either. Declarations appear in the
//! declaration-bearing forms named by CLHS 3.1.2.1.2: `lambda`, `let`, `let*`,
//! `locally`, `flet`, `labels`, `macrolet`, and `symbol-macrolet`.

use crate::declaration::Declaration;
use crate::lambda_list::LambdaList;
use crate::literal::Literal;
use crate::symbols::SymbolRef;
use crate::types::TypeSpecifier;

/// A parsed form.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Expr {
    /// `(quote datum)`, or a self-evaluating atom.
    Constant(Literal),
    /// A reference to a lexical or special variable.
    Variable(SymbolRef),
    /// A call, `(operator argument*)`.
    Call {
        /// The operator position: a function name or a lambda expression.
        operator: Operator,
        /// The argument forms.
        arguments: Vec<Self>,
    },
    /// `(function name)` or `#'(lambda ...)`.
    Function(FunctionDesignator),
    /// A `(lambda ...)` expression.
    Lambda(Box<LambdaExpr>),
    /// `(if test then [else])`.
    If {
        /// The test form.
        test: Box<Self>,
        /// The form evaluated when the test is true.
        then: Box<Self>,
        /// The form evaluated when the test is false; `None` means `nil`.
        otherwise: Option<Box<Self>>,
    },
    /// `(progn form*)`.
    Progn(Vec<Self>),
    /// `(block name form*)`.
    Block {
        /// The block name.
        name: SymbolRef,
        /// The body forms.
        body: Vec<Self>,
    },
    /// `(return-from name [value])`.
    ReturnFrom {
        /// The block name.
        name: SymbolRef,
        /// The returned value; `None` means `nil`.
        value: Option<Box<Self>>,
    },
    /// `(tagbody item*)`, where items are tags or forms.
    Tagbody(Vec<TagbodyItem>),
    /// `(go tag)`.
    Go {
        /// The target tag.
        tag: SymbolRef,
    },
    /// `(catch tag form*)`.
    Catch {
        /// The catch tag form.
        tag: Box<Self>,
        /// The body forms.
        body: Vec<Self>,
    },
    /// `(throw tag value)`.
    Throw {
        /// The catch tag form.
        tag: Box<Self>,
        /// The thrown value form.
        value: Box<Self>,
    },
    /// `(unwind-protect protected cleanup*)`.
    UnwindProtect {
        /// The protected form.
        protected: Box<Self>,
        /// The cleanup forms.
        cleanup: Vec<Self>,
    },
    /// `(let binding* declaration* form*)`, or `let*`.
    Let {
        /// Whether the bindings are sequential, which is true for `let*`.
        sequential: bool,
        /// The variable bindings.
        bindings: Vec<LetBinding>,
        /// The declarations that open the body.
        declarations: Vec<Declaration>,
        /// The body forms.
        body: Vec<Self>,
    },
    /// `(progv symbols values form*)`.
    Progv {
        /// The form producing the list of special symbols.
        symbols: Box<Self>,
        /// The form producing the list of values.
        values: Box<Self>,
        /// The body forms.
        body: Vec<Self>,
    },
    /// `(setq place value*)`, stored as ordered pairs.
    Setq(Vec<(SymbolRef, Self)>),
    /// `(multiple-value-call function form*)`.
    MultipleValueCall {
        /// The function form.
        function: Box<Self>,
        /// The argument forms.
        arguments: Vec<Self>,
    },
    /// `(multiple-value-prog1 first form*)`.
    MultipleValueProg1 {
        /// The form whose values are preserved.
        first: Box<Self>,
        /// The remaining forms.
        forms: Vec<Self>,
    },
    /// `(the type form)`.
    The {
        /// The asserted type.
        type_specifier: TypeSpecifier,
        /// The value form.
        value: Box<Self>,
    },
    /// `(eval-when situations form*)`.
    EvalWhen {
        /// The situations in which the body is evaluated.
        situations: Vec<EvalSituation>,
        /// The body forms.
        body: Vec<Self>,
    },
    /// `(load-time-value form [read-only])`.
    LoadTimeValue {
        /// The form evaluated at load time.
        form: Box<Self>,
        /// Whether the value is read-only.
        read_only: bool,
    },
    /// `(locally declaration* form*)`.
    Locally {
        /// The declarations.
        declarations: Vec<Declaration>,
        /// The body forms.
        body: Vec<Self>,
    },
    /// `(flet definition* declaration* form*)`.
    Flet {
        /// The local function definitions.
        definitions: Vec<LocalFunction>,
        /// The declarations that open the body.
        declarations: Vec<Declaration>,
        /// The body forms.
        body: Vec<Self>,
    },
    /// `(labels definition* declaration* form*)`.
    Labels {
        /// The local function definitions.
        definitions: Vec<LocalFunction>,
        /// The declarations that open the body.
        declarations: Vec<Declaration>,
        /// The body forms.
        body: Vec<Self>,
    },
    /// `(macrolet definition* declaration* form*)`.
    Macrolet {
        /// The local macro definitions.
        definitions: Vec<LocalMacro>,
        /// The declarations that open the body.
        declarations: Vec<Declaration>,
        /// The body forms.
        body: Vec<Self>,
    },
    /// `(symbol-macrolet definition* declaration* form*)`.
    SymbolMacrolet {
        /// The local symbol-macro definitions.
        definitions: Vec<SymbolMacro>,
        /// The declarations that open the body.
        declarations: Vec<Declaration>,
        /// The body forms.
        body: Vec<Self>,
    },
}

/// The operator position of a call.
///
/// CLHS 3.1.2.1.2 restricts the operator to a symbol naming a function or a
/// lambda expression.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Operator {
    /// A symbol naming a function.
    Name(SymbolRef),
    /// A lambda expression, as in `((lambda (x) x) 1)`.
    Lambda(Box<LambdaExpr>),
}

/// A lambda expression: a lambda list, declarations, an optional docstring, and a body.
#[derive(Clone, Debug, PartialEq)]
pub struct LambdaExpr {
    /// The lambda list.
    pub lambda_list: LambdaList,
    /// The declarations that open the body.
    pub declarations: Vec<Declaration>,
    /// The documentation string, if the body opened with one.
    pub docstring: Option<String>,
    /// The body forms.
    pub body: Vec<Expr>,
}

/// One `let` or `let*` binding.
#[derive(Clone, Debug, PartialEq)]
pub struct LetBinding {
    /// The variable name.
    pub name: SymbolRef,
    /// The initial value form; `None` means `nil`.
    pub value: Option<Expr>,
}

/// One `flet` or `labels` definition.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalFunction {
    /// The function name.
    pub name: SymbolRef,
    /// The function body.
    pub lambda: LambdaExpr,
}

/// One `macrolet` definition.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalMacro {
    /// The macro name.
    pub name: SymbolRef,
    /// The macro lambda list.
    pub lambda_list: LambdaList,
    /// The declarations that open the expander body.
    pub declarations: Vec<Declaration>,
    /// The documentation string, if present.
    pub docstring: Option<String>,
    /// The expansion forms.
    pub body: Vec<Expr>,
}

/// One `symbol-macrolet` definition.
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolMacro {
    /// The symbol being defined.
    pub name: SymbolRef,
    /// The expansion form.
    pub expansion: Expr,
}

/// One element of a `tagbody`: a tag or a form.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum TagbodyItem {
    /// A tag.
    Tag(SymbolRef),
    /// A form.
    Form(Expr),
}

/// One situation named by `eval-when`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum EvalSituation {
    /// `:compile-toplevel`.
    CompileToplevel,
    /// `:load-toplevel`.
    LoadToplevel,
    /// `:execute`.
    Execute,
    /// `:eval`, an implementation extension accepted by SBCL.
    Eval,
}

/// The designator of a `(function ...)` form.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum FunctionDesignator {
    /// A named function.
    Name(SymbolRef),
    /// An anonymous function.
    Lambda(Box<LambdaExpr>),
}
