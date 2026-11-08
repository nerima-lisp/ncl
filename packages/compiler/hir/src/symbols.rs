//! Compilation-unit symbol identities and interned spellings.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::ids::{CompilationUnitId, FormId, IdError, SymbolId};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum SymbolKey {
    Interned { package: String, name: String },
    Uninterned { form: FormId, identity: u32 },
}

/// A source symbol's package, spelling, and optional uninterned identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSymbol {
    package: Option<String>,
    name: String,
    uninterned_identity: Option<u32>,
}

impl HirSymbol {
    /// Returns the home package, or `None` for an uninterned symbol.
    #[must_use]
    pub fn package(&self) -> Option<&str> {
        self.package.as_deref()
    }

    /// Returns the symbol's source spelling.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the expansion-local identity of an uninterned symbol.
    #[must_use]
    pub const fn uninterned_identity(&self) -> Option<u32> {
        self.uninterned_identity
    }
}

/// A compilation-unit interner for interned and expansion-local symbols.
#[derive(Clone, Debug)]
pub struct SymbolTable {
    compilation_unit: CompilationUnitId,
    symbols: Vec<HirSymbol>,
    indices: HashMap<SymbolKey, SymbolId>,
}

impl SymbolTable {
    /// Creates an empty symbol table for one compilation unit.
    #[must_use]
    pub fn new(compilation_unit: CompilationUnitId) -> Self {
        Self {
            compilation_unit,
            symbols: Vec::new(),
            indices: HashMap::new(),
        }
    }

    /// Returns the owning compilation unit.
    #[must_use]
    pub const fn compilation_unit(&self) -> CompilationUnitId {
        self.compilation_unit
    }

    /// Interns a symbol, keeping uninterned identities local to their form.
    ///
    /// # Errors
    ///
    /// Returns `SymbolError` when the form belongs to another compilation unit,
    /// the package and identity do not describe a valid symbol, or the ID space
    /// is exhausted.
    pub fn intern(
        &mut self,
        form: FormId,
        package: Option<&str>,
        name: &str,
        uninterned_identity: Option<u32>,
    ) -> Result<SymbolId, SymbolError> {
        if form.compilation_unit() != self.compilation_unit {
            return Err(SymbolError::ForeignForm(form));
        }

        let (key, symbol) = match (package, uninterned_identity) {
            (Some(package), None) => {
                let package = package.to_owned();
                (
                    SymbolKey::Interned {
                        package: package.clone(),
                        name: name.to_owned(),
                    },
                    HirSymbol {
                        package: Some(package),
                        name: name.to_owned(),
                        uninterned_identity: None,
                    },
                )
            }
            (None, Some(identity)) => (
                SymbolKey::Uninterned { form, identity },
                HirSymbol {
                    package: None,
                    name: name.to_owned(),
                    uninterned_identity: Some(identity),
                },
            ),
            (Some(_), Some(_)) | (None, None) => {
                return Err(SymbolError::InvalidIdentityShape);
            }
        };

        if let Some(id) = self.indices.get(&key).copied() {
            let existing = self.get(id).ok_or(SymbolError::InvalidIdentityShape)?;
            if existing.name != symbol.name {
                if let Some(identity) = uninterned_identity {
                    return Err(SymbolError::ConflictingUninternedName { form, identity });
                }
                return Err(SymbolError::InvalidIdentityShape);
            }
            return Ok(id);
        }

        let id = SymbolId::try_from_index(self.compilation_unit, self.symbols.len())
            .map_err(SymbolError::Id)?;
        self.symbols.push(symbol);
        let _ = self.indices.insert(key, id);
        Ok(id)
    }

    /// Returns a symbol when its identity belongs to this table.
    #[must_use]
    pub fn get(&self, id: SymbolId) -> Option<&HirSymbol> {
        if id.compilation_unit() != self.compilation_unit {
            return None;
        }
        self.symbols.get(usize::try_from(id.index()).ok()?)
    }

    /// Returns every dense identity allocated in this compilation unit.
    #[must_use]
    pub fn ids(&self) -> HashSet<SymbolId> {
        self.symbols
            .iter()
            .enumerate()
            .filter_map(|(index, _)| SymbolId::try_from_index(self.compilation_unit, index).ok())
            .collect()
    }
}

/// An invalid symbol identity or interning request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SymbolError {
    /// The source form is owned by another compilation unit.
    ForeignForm(FormId),
    /// Package and uninterned identity do not agree.
    InvalidIdentityShape,
    /// An uninterned identity was reused with a different spelling.
    ConflictingUninternedName {
        /// The form containing the expansion-local identity.
        form: FormId,
        /// The reused expansion-local identity.
        identity: u32,
    },
    /// The compilation-unit symbol index space is exhausted.
    Id(IdError),
}

impl Display for SymbolError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "symbol table error: {self:?}")
    }
}

impl Error for SymbolError {}
