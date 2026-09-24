//! The lexical environment used while lowering one function.
//!
//! Each generated function owns one environment. A lambda lowered into its own
//! function starts a fresh environment whose captured variables are bound to the
//! function's leading parameters, so no environment entry outlives its function.

use ncl_ir::{BlockId, ValueId};

use crate::symbols::SymbolRef;

/// Where a variable's current value lives in the generated function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Slot {
    /// The variable is an SSA value of type `Word`.
    Value(ValueId),
    /// The variable is boxed in a one-word cell; the slot is an `Address`.
    Cell(ValueId),
}

/// A variable binding in the innermost scope.
#[derive(Clone, Debug)]
pub(super) struct VariableEntry {
    /// The bound name.
    pub name: SymbolRef,
    /// Where the current value lives.
    pub slot: Slot,
}

/// A `block` exit target.
#[derive(Clone, Debug)]
pub(super) struct BlockEntry {
    /// The block name.
    pub name: SymbolRef,
    /// The exit block.
    pub target: BlockId,
}

/// A `tagbody` tag target.
#[derive(Clone, Debug)]
pub(super) struct TagEntry {
    /// The tag name.
    pub name: SymbolRef,
    /// The block the tag labels.
    pub target: BlockId,
}

/// A local function bound by `flet` or `labels`.
#[derive(Clone, Debug)]
pub(super) struct FunctionEntry {
    /// The function name.
    pub name: SymbolRef,
    /// The placeholder callee value emitted at every call site.
    pub callee: ValueId,
    /// The captured values the callee expects before its own arguments.
    pub captures: Vec<Slot>,
}

/// One lexical scope.
#[derive(Clone, Debug, Default)]
struct Scope {
    variables: Vec<VariableEntry>,
    functions: Vec<FunctionEntry>,
    blocks: Vec<BlockEntry>,
    tags: Vec<TagEntry>,
}

/// A stack of lexical scopes for one generated function.
#[derive(Clone, Debug, Default)]
pub(super) struct LowerEnv {
    scopes: Vec<Scope>,
}

impl LowerEnv {
    /// Create an environment with one empty scope.
    pub(super) fn new() -> Self {
        Self {
            scopes: vec![Scope::default()],
        }
    }

    /// Open a new scope.
    pub(super) fn push(&mut self) {
        self.scopes.push(Scope::default());
    }

    /// Close the innermost scope; the root scope is never closed.
    pub(super) fn pop(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Bind a variable in the innermost scope.
    pub(super) fn bind_variable(&mut self, name: SymbolRef, slot: Slot) {
        self.current().variables.push(VariableEntry { name, slot });
    }

    /// Replace the slot of the innermost binding of `name`.
    ///
    /// Returns `false` when no lexical binding exists, which means the variable
    /// is special or global.
    pub(super) fn rebind_variable(&mut self, name: &SymbolRef, slot: Slot) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(entry) = scope
                .variables
                .iter_mut()
                .rev()
                .find(|entry| entry.name == *name)
            {
                entry.slot = slot;
                return true;
            }
        }
        false
    }

    /// Find the innermost slot bound to `name`.
    pub(super) fn lookup_variable(&self, name: &SymbolRef) -> Option<Slot> {
        self.scopes.iter().rev().find_map(|scope| {
            scope
                .variables
                .iter()
                .rev()
                .find(|entry| entry.name == *name)
                .map(|entry| entry.slot)
        })
    }

    /// Bind a local function in the innermost scope.
    pub(super) fn bind_function(&mut self, entry: FunctionEntry) {
        self.current().functions.push(entry);
    }

    /// Find the innermost local function named `name`.
    pub(super) fn lookup_function(&self, name: &SymbolRef) -> Option<FunctionEntry> {
        self.scopes.iter().rev().find_map(|scope| {
            scope
                .functions
                .iter()
                .rev()
                .find(|entry| entry.name == *name)
                .cloned()
        })
    }

    /// Bind a block name in the innermost scope.
    pub(super) fn bind_block(&mut self, entry: BlockEntry) {
        self.current().blocks.push(entry);
    }

    /// Find the innermost block named `name`.
    pub(super) fn lookup_block(&self, name: &SymbolRef) -> Option<BlockEntry> {
        self.scopes.iter().rev().find_map(|scope| {
            scope
                .blocks
                .iter()
                .rev()
                .find(|entry| entry.name == *name)
                .cloned()
        })
    }

    /// Bind a tag in the innermost scope.
    pub(super) fn bind_tag(&mut self, entry: TagEntry) {
        self.current().tags.push(entry);
    }

    /// Find the innermost tag named `name`.
    pub(super) fn lookup_tag(&self, name: &SymbolRef) -> Option<TagEntry> {
        self.scopes.iter().rev().find_map(|scope| {
            scope
                .tags
                .iter()
                .rev()
                .find(|entry| entry.name == *name)
                .cloned()
        })
    }

    /// The innermost scope.
    fn current(&mut self) -> &mut Scope {
        if self.scopes.is_empty() {
            self.scopes.push(Scope::default());
        }
        let index = self.scopes.len() - 1;
        &mut self.scopes[index]
    }
}
