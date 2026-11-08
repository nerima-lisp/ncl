//! Lexical variable and local-function identity with explicit scope visibility.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::arena::Arena;
use crate::ids::{BindingId, FormId, IdError, NodeId, SymbolId};

/// Storage for bindings in one top-level form.
pub type BindingArena = Arena<Binding, BindingId>;

/// The kind of local function scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LocalFunctionKind {
    /// Function definitions do not see their own local group.
    Flet,
    /// Function definitions see every function in their local group.
    Labels,
}

/// A variable or local-function binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Binding {
    id: BindingId,
    symbol: SymbolId,
    kind: BindingKind,
    scope: ScopeId,
    reads: Vec<NodeId>,
    writes: Vec<NodeId>,
}

impl Binding {
    /// Returns this binding's typed identity.
    #[must_use]
    pub const fn id(&self) -> BindingId {
        self.id
    }

    /// Returns the interned name of this binding.
    #[must_use]
    pub const fn symbol(&self) -> SymbolId {
        self.symbol
    }

    /// Returns whether this is a lexical variable or local function binding.
    #[must_use]
    pub const fn kind(&self) -> BindingKind {
        self.kind
    }

    /// Returns the lexical scope that owns this binding.
    #[must_use]
    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    /// Returns the reference nodes that read this binding.
    #[must_use]
    pub fn reads(&self) -> &[NodeId] {
        &self.reads
    }

    /// Returns the assignment nodes that write this binding.
    #[must_use]
    pub fn writes(&self) -> &[NodeId] {
        &self.writes
    }
}

/// A binding's variable or function identity class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BindingKind {
    /// A lexical variable.
    LexicalVariable,
    /// A lexical function introduced by FLET or LABELS.
    LocalFunction,
}

/// A namespace used while resolving names.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum BindingNamespace {
    /// Names in this namespace resolve to lexical variable bindings.
    Variable,
    /// Names in this namespace resolve to lexical function bindings.
    Function,
}

/// A lexical scope identity within one binding environment.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ScopeId {
    form: FormId,
    index: u32,
}

impl ScopeId {
    /// Returns the top-level form that owns this scope.
    #[must_use]
    pub const fn form(self) -> FormId {
        self.form
    }

    /// Returns this scope's position in the environment.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }
}

/// A local function binding and the scope used to compile its body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalFunctionBinding {
    /// The interned function name.
    pub symbol: SymbolId,
    /// The unique lexical function identity.
    pub binding: BindingId,
    /// The scope visible while compiling this function body.
    pub definition_scope: ScopeId,
}

/// A local function group's body scope and per-definition visibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalFunctionGroup {
    /// The scope where every local function name is visible.
    pub body_scope: ScopeId,
    /// The local definitions and their definition-body scopes.
    pub definitions: Vec<LocalFunctionBinding>,
}

/// Errors from scope or binding construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BindingError {
    /// An ID index exceeded the supported `u32` range.
    Id(IdError),
    /// A scope is not owned by this environment or does not exist.
    UnknownScope(ScopeId),
    /// A binding identity is not present in this environment.
    UnknownBinding(BindingId),
    /// A name is already bound in the same scope and namespace.
    DuplicateName {
        /// The scope containing the first binding.
        scope: ScopeId,
        /// The duplicate symbol identity.
        symbol: SymbolId,
        /// The conflicting namespace.
        namespace: BindingNamespace,
    },
    /// A usage node belongs to a different top-level form.
    ForeignUse(NodeId),
}

impl Display for BindingError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "binding error: {self:?}")
    }
}

impl Error for BindingError {}

impl From<IdError> for BindingError {
    fn from(error: IdError) -> Self {
        Self::Id(error)
    }
}

#[derive(Clone, Debug)]
struct ScopeFrame {
    id: ScopeId,
    parent: Option<ScopeId>,
    variables: HashMap<SymbolId, BindingId>,
    functions: HashMap<SymbolId, BindingId>,
}

/// A form-local lexical environment and binding table.
#[derive(Debug)]
pub struct BindingEnvironment {
    form: FormId,
    bindings: BindingArena,
    scopes: Vec<ScopeFrame>,
    root: ScopeId,
}

impl BindingEnvironment {
    /// Creates an environment with one empty root scope.
    #[must_use]
    pub fn new(form: FormId) -> Self {
        let root = ScopeId { form, index: 0 };
        Self {
            form,
            bindings: BindingArena::new(form),
            scopes: vec![ScopeFrame {
                id: root,
                parent: None,
                variables: HashMap::new(),
                functions: HashMap::new(),
            }],
            root,
        }
    }

    /// Returns the scope used to resolve top-level lexical bindings.
    #[must_use]
    pub const fn root_scope(&self) -> ScopeId {
        self.root
    }

    /// Returns the parent scope, or `None` for the root scope.
    ///
    /// # Errors
    ///
    /// Returns `BindingError::UnknownScope` when `scope` is outside this environment.
    pub fn scope_parent(&self, scope: ScopeId) -> Result<Option<ScopeId>, BindingError> {
        Ok(self.scope_frame(scope)?.parent)
    }

    /// Creates a child scope under `parent`.
    ///
    /// # Errors
    ///
    /// Returns `BindingError::UnknownScope` when `parent` is invalid, or
    /// `BindingError::Id` if the scope index cannot fit in `u32`.
    pub fn push_scope(&mut self, parent: ScopeId) -> Result<ScopeId, BindingError> {
        self.scope_frame(parent)?;
        let index = u32::try_from(self.scopes.len()).map_err(|_| IdError::IndexOverflow {
            index: self.scopes.len(),
        })?;
        let id = ScopeId {
            form: self.form,
            index,
        };
        self.scopes.push(ScopeFrame {
            id,
            parent: Some(parent),
            variables: HashMap::new(),
            functions: HashMap::new(),
        });
        Ok(id)
    }

    /// Adds a lexical variable binding to `scope`.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid scope, duplicate name, or exhausted
    /// binding identity space.
    pub fn bind_variable(
        &mut self,
        scope: ScopeId,
        symbol: SymbolId,
    ) -> Result<BindingId, BindingError> {
        self.insert(
            scope,
            symbol,
            BindingKind::LexicalVariable,
            BindingNamespace::Variable,
        )
    }

    /// Adds a local function binding to `scope`.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid scope, duplicate name, or exhausted
    /// binding identity space.
    pub fn bind_function(
        &mut self,
        scope: ScopeId,
        symbol: SymbolId,
    ) -> Result<BindingId, BindingError> {
        self.insert(
            scope,
            symbol,
            BindingKind::LocalFunction,
            BindingNamespace::Function,
        )
    }

    /// Creates a FLET or LABELS group with the specified visibility rules.
    ///
    /// FLET definition bodies resolve in the outer scope. LABELS definitions
    /// resolve in the group's body scope, so they can refer to themselves and
    /// to each other.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid parent, duplicate name, or exhausted ID
    /// space. Duplicate names are checked before the environment is changed.
    pub fn bind_local_functions(
        &mut self,
        parent: ScopeId,
        kind: LocalFunctionKind,
        names: &[SymbolId],
    ) -> Result<LocalFunctionGroup, BindingError> {
        self.scope_frame(parent)?;
        let mut seen = HashSet::with_capacity(names.len());
        for symbol in names {
            if !seen.insert(*symbol) {
                return Err(BindingError::DuplicateName {
                    scope: parent,
                    symbol: *symbol,
                    namespace: BindingNamespace::Function,
                });
            }
        }
        check_append_capacity(self.scopes.len(), 1)?;
        check_append_capacity(self.bindings.len(), names.len())?;

        let body_scope = self.push_scope(parent)?;
        let mut definitions = Vec::with_capacity(names.len());
        for symbol in names {
            let binding = self.bind_function(body_scope, *symbol)?;
            let definition_scope = match kind {
                LocalFunctionKind::Flet => parent,
                LocalFunctionKind::Labels => body_scope,
            };
            definitions.push(LocalFunctionBinding {
                symbol: *symbol,
                binding,
                definition_scope,
            });
        }
        Ok(LocalFunctionGroup {
            body_scope,
            definitions,
        })
    }

    /// Resolves the nearest lexical variable binding for `symbol`.
    ///
    /// # Errors
    ///
    /// Returns `BindingError::UnknownScope` for a scope outside this environment.
    pub fn resolve_variable(
        &self,
        scope: ScopeId,
        symbol: SymbolId,
    ) -> Result<Option<BindingId>, BindingError> {
        self.resolve(scope, symbol, BindingNamespace::Variable)
    }

    /// Resolves the nearest local function binding for `symbol`.
    ///
    /// # Errors
    ///
    /// Returns `BindingError::UnknownScope` for a scope outside this environment.
    pub fn resolve_function(
        &self,
        scope: ScopeId,
        symbol: SymbolId,
    ) -> Result<Option<BindingId>, BindingError> {
        self.resolve(scope, symbol, BindingNamespace::Function)
    }

    /// Returns a binding when its identity belongs to this environment.
    #[must_use]
    pub fn binding(&self, id: BindingId) -> Option<&Binding> {
        self.bindings.get(id)
    }

    /// Iterates over bindings in allocation order.
    pub fn iter(&self) -> impl Iterator<Item = &Binding> {
        self.bindings.iter().map(|(_, binding)| binding)
    }

    /// Records a read or write node on a binding.
    ///
    /// # Errors
    ///
    /// Returns `BindingError::UnknownBinding` for an invalid binding, or
    /// `BindingError::ForeignUse` when `node` belongs to another form.
    pub fn record_use(
        &mut self,
        id: BindingId,
        node: NodeId,
        kind: BindingUseKind,
    ) -> Result<(), BindingError> {
        if node.form() != self.form {
            return Err(BindingError::ForeignUse(node));
        }
        let binding = self
            .bindings
            .get_mut(id)
            .ok_or(BindingError::UnknownBinding(id))?;
        match kind {
            BindingUseKind::Read => binding.reads.push(node),
            BindingUseKind::Write => binding.writes.push(node),
        }
        Ok(())
    }

    fn insert(
        &mut self,
        scope: ScopeId,
        symbol: SymbolId,
        kind: BindingKind,
        namespace: BindingNamespace,
    ) -> Result<BindingId, BindingError> {
        let frame = self.scope_frame(scope)?;
        let exists = match namespace {
            BindingNamespace::Variable => frame.variables.contains_key(&symbol),
            BindingNamespace::Function => frame.functions.contains_key(&symbol),
        };
        if exists {
            return Err(BindingError::DuplicateName {
                scope,
                symbol,
                namespace,
            });
        }
        check_append_capacity(self.bindings.len(), 1)?;
        let id = BindingId::try_from_index(self.form, self.bindings.len())?;
        let allocated = self.bindings.allocate(Binding {
            id,
            symbol,
            kind,
            scope,
            reads: Vec::new(),
            writes: Vec::new(),
        })?;
        let frame = self.scope_frame_mut(scope)?;
        match namespace {
            BindingNamespace::Variable => frame.variables.insert(symbol, allocated),
            BindingNamespace::Function => frame.functions.insert(symbol, allocated),
        };
        Ok(allocated)
    }

    fn resolve(
        &self,
        mut scope: ScopeId,
        symbol: SymbolId,
        namespace: BindingNamespace,
    ) -> Result<Option<BindingId>, BindingError> {
        loop {
            let frame = self.scope_frame(scope)?;
            let binding = match namespace {
                BindingNamespace::Variable => frame.variables.get(&symbol),
                BindingNamespace::Function => frame.functions.get(&symbol),
            };
            if let Some(binding) = binding {
                return Ok(Some(*binding));
            }
            let Some(parent) = frame.parent else {
                return Ok(None);
            };
            scope = parent;
        }
    }

    fn scope_frame(&self, scope: ScopeId) -> Result<&ScopeFrame, BindingError> {
        if scope.form != self.form {
            return Err(BindingError::UnknownScope(scope));
        }
        let index = usize::try_from(scope.index).map_err(|_| BindingError::UnknownScope(scope))?;
        self.scopes
            .get(index)
            .filter(|frame| frame.id == scope)
            .ok_or(BindingError::UnknownScope(scope))
    }

    fn scope_frame_mut(&mut self, scope: ScopeId) -> Result<&mut ScopeFrame, BindingError> {
        if scope.form != self.form {
            return Err(BindingError::UnknownScope(scope));
        }
        let index = usize::try_from(scope.index).map_err(|_| BindingError::UnknownScope(scope))?;
        self.scopes
            .get_mut(index)
            .filter(|frame| frame.id == scope)
            .ok_or(BindingError::UnknownScope(scope))
    }
}

/// Whether a reference node reads or assigns a binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BindingUseKind {
    /// A reference operation observes the binding's current value.
    Read,
    /// A store operation changes the value associated with the binding.
    Write,
}

fn check_append_capacity(current: usize, additional: usize) -> Result<(), BindingError> {
    if additional == 0 {
        return Ok(());
    }
    let last = current
        .checked_add(additional)
        .and_then(|end| end.checked_sub(1))
        .ok_or(IdError::IndexOverflow { index: usize::MAX })?;
    u32::try_from(last).map_err(|_| IdError::IndexOverflow { index: last })?;
    Ok(())
}
