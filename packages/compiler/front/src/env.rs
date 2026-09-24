//! The lexical environment used while expanding forms.
//!
//! The environment tracks what expansion must resolve: variables (lexical or
//! special), functions, local macros, symbol macros, blocks, and tags, together
//! with the declarations that establish them. It is a stack of frames, and a
//! lookup walks from the innermost frame outward.
//!
//! Special declarations are recorded here rather than in the AST, because
//! whether a reference is dynamic does not change its shape. A lowering lane
//! re-derives special-ness from the declaration nodes the AST keeps.

use crate::ast::{LocalMacro, SymbolMacro};
use crate::symbols::SymbolRef;
use crate::types::TypeSpecifier;

/// How a name is bound as a variable in one frame.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum VariableBinding {
    /// A lexical variable, with the type its declaration asserted.
    Lexical {
        /// The variable name.
        name: SymbolRef,
        /// The declared type, if any.
        type_specifier: Option<TypeSpecifier>,
    },
    /// A variable declared special, so references to it are dynamic.
    Special(SymbolRef),
}

impl VariableBinding {
    /// The bound name.
    #[must_use]
    pub const fn name(&self) -> &SymbolRef {
        match self {
            Self::Lexical { name, .. } | Self::Special(name) => name,
        }
    }

    /// Whether the binding is dynamic.
    #[must_use]
    pub const fn is_special(&self) -> bool {
        matches!(self, Self::Special(_))
    }

    /// The declared type, if the binding carries one.
    #[must_use]
    pub const fn type_specifier(&self) -> Option<&TypeSpecifier> {
        match self {
            Self::Lexical { type_specifier, .. } => type_specifier.as_ref(),
            Self::Special(_) => None,
        }
    }
}

/// One lexical scope.
#[derive(Clone, Debug, Default)]
struct Frame {
    variables: Vec<VariableBinding>,
    functions: Vec<SymbolRef>,
    macros: Vec<LocalMacro>,
    symbol_macros: Vec<SymbolMacro>,
    blocks: Vec<SymbolRef>,
    tags: Vec<SymbolRef>,
}

/// A stack of lexical scopes.
#[derive(Clone, Debug, Default)]
pub struct LexicalEnv {
    frames: Vec<Frame>,
}

impl LexicalEnv {
    /// Create an environment with one empty frame.
    #[must_use]
    pub fn new() -> Self {
        Self {
            frames: vec![Frame::default()],
        }
    }

    /// The number of open scopes.
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Open a new scope.
    pub fn push_scope(&mut self) {
        self.frames.push(Frame::default());
    }

    /// Close the innermost scope.
    ///
    /// The root frame is never closed, so this is total even if a caller pops
    /// more often than it pushes.
    pub fn pop_scope(&mut self) {
        if self.frames.len() > 1 {
            self.frames.pop();
        }
    }

    /// Bind a lexical variable in the innermost scope.
    pub fn bind_variable(&mut self, name: SymbolRef, type_specifier: Option<TypeSpecifier>) {
        self.current().variables.push(VariableBinding::Lexical {
            name,
            type_specifier,
        });
    }

    /// Bind a variable as special in the innermost scope.
    pub fn bind_special(&mut self, name: SymbolRef) {
        self.current()
            .variables
            .push(VariableBinding::Special(name));
    }

    /// Record a type declaration for a variable in the innermost scope.
    ///
    /// The new binding shadows an earlier one, so a lookup finds the declared
    /// type.
    pub fn declare_type(&mut self, name: SymbolRef, type_specifier: TypeSpecifier) {
        self.current().variables.push(VariableBinding::Lexical {
            name,
            type_specifier: Some(type_specifier),
        });
    }

    /// Bind a function name in the innermost scope.
    pub fn bind_function(&mut self, name: SymbolRef) {
        self.current().functions.push(name);
    }

    /// Bind a local macro in the innermost scope.
    pub fn bind_macro(&mut self, definition: LocalMacro) {
        self.current().macros.push(definition);
    }

    /// Bind a symbol macro in the innermost scope.
    pub fn bind_symbol_macro(&mut self, definition: SymbolMacro) {
        self.current().symbol_macros.push(definition);
    }

    /// Bind a block name in the innermost scope.
    pub fn bind_block(&mut self, name: SymbolRef) {
        self.current().blocks.push(name);
    }

    /// Bind a tag in the innermost scope.
    pub fn bind_tag(&mut self, name: SymbolRef) {
        self.current().tags.push(name);
    }

    /// Find the innermost variable binding for `name`.
    #[must_use]
    pub fn lookup_variable(&self, name: &SymbolRef) -> Option<&VariableBinding> {
        self.frames.iter().rev().find_map(|frame| {
            frame
                .variables
                .iter()
                .rev()
                .find(|binding| binding.name() == name)
        })
    }

    /// Whether `name` is bound as a variable anywhere in scope.
    #[must_use]
    pub fn is_bound_variable(&self, name: &SymbolRef) -> bool {
        self.lookup_variable(name).is_some()
    }

    /// Whether `name` is special in the innermost binding that mentions it.
    #[must_use]
    pub fn is_special(&self, name: &SymbolRef) -> bool {
        self.lookup_variable(name)
            .is_some_and(VariableBinding::is_special)
    }

    /// The declared type of `name`, if a binding carries one.
    #[must_use]
    pub fn variable_type(&self, name: &SymbolRef) -> Option<&TypeSpecifier> {
        self.lookup_variable(name)
            .and_then(VariableBinding::type_specifier)
    }

    /// Whether `name` is bound as a local function.
    #[must_use]
    pub fn lookup_function(&self, name: &SymbolRef) -> bool {
        self.frames
            .iter()
            .rev()
            .any(|frame| frame.functions.iter().any(|bound| bound == name))
    }

    /// Find the innermost local macro definition for `name`.
    #[must_use]
    pub fn lookup_macro(&self, name: &SymbolRef) -> Option<&LocalMacro> {
        self.frames
            .iter()
            .rev()
            .find_map(|frame| frame.macros.iter().rev().find(|entry| entry.name == *name))
    }

    /// Find the innermost symbol-macro definition for `name`.
    #[must_use]
    pub fn lookup_symbol_macro(&self, name: &SymbolRef) -> Option<&SymbolMacro> {
        self.frames.iter().rev().find_map(|frame| {
            frame
                .symbol_macros
                .iter()
                .rev()
                .find(|entry| entry.name == *name)
        })
    }

    /// Whether `name` is an enclosing block name.
    #[must_use]
    pub fn find_block(&self, name: &SymbolRef) -> bool {
        self.frames
            .iter()
            .rev()
            .any(|frame| frame.blocks.iter().any(|block| block == name))
    }

    /// Whether `name` is an enclosing tag.
    #[must_use]
    pub fn find_tag(&self, name: &SymbolRef) -> bool {
        self.frames
            .iter()
            .rev()
            .any(|frame| frame.tags.iter().any(|tag| tag == name))
    }

    /// The innermost scope.
    fn current(&mut self) -> &mut Frame {
        if self.frames.is_empty() {
            self.frames.push(Frame::default());
        }
        let index = self.frames.len() - 1;
        &mut self.frames[index]
    }
}
