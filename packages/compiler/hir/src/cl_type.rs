//! Compiler-owned abstract types for Common Lisp values.

#[path = "cl_type/compound.rs"]
pub mod compound;
#[path = "cl_type/numeric.rs"]
pub mod numeric;
#[path = "cl_type/relation.rs"]
pub mod relation;

use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::arena::Arena;
use crate::ids::{FormId, IdError, SymbolId, TypeId, TypeSpecConstId};

pub use compound::{
    ArrayDimensions, ArrayType, BuiltinType, FunctionParameters, FunctionType, MemberValue,
};
pub use numeric::{NumericBound, NumericDomain, NumericRange, NumericRangeError, NumericValue};
pub use relation::SubtypeRelation;

/// One node in the Common Lisp type lattice.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ClType {
    kind: ClTypeKind,
}

impl ClType {
    /// Returns this node's type constructor and operands.
    #[must_use]
    pub const fn kind(&self) -> &ClTypeKind {
        &self.kind
    }
}

/// A Common Lisp type constructor represented independently of runtime values.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ClTypeKind {
    /// The universal T type.
    Any,
    /// The empty NIL type.
    Empty,
    /// A recognized atomic Common Lisp type name.
    Named(BuiltinType),
    /// A bounded integer or floating-point domain.
    NumericRange(NumericRange),
    /// A finite MEMBER type using Common Lisp EQL equality.
    Member(Vec<MemberValue>),
    /// A canonical union corresponding to OR.
    Union(Vec<TypeId>),
    /// A canonical intersection corresponding to AND.
    Intersection(Vec<TypeId>),
    /// A complement type corresponding to NOT.
    Complement(TypeId),
    /// A pair of element types for a CONS type.
    Cons {
        /// Type of the car value.
        car: TypeId,
        /// Type of the cdr value.
        cdr: TypeId,
    },
    /// An array type, including SIMPLE-ARRAY constraints.
    Array(ArrayType),
    /// A function type and its return constraint.
    Function(FunctionType),
    /// A fixed sequence of returned value types.
    MultipleValues(Vec<TypeId>),
    /// A registered class identified by source symbol identity.
    Class(SymbolId),
    /// A source type form whose meaning is not yet known to the compiler.
    Opaque {
        /// Name of the unresolved type specifier.
        specifier: SymbolId,
        /// Operands retained from the source form.
        operands: Vec<OpaqueOperand>,
    },
}

/// A source operand retained by an unresolved type specifier.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum OpaqueOperand {
    /// A recognized type-valued operand.
    Type(TypeId),
    /// A non-type source constant retained by identity.
    Constant(TypeSpecConstId),
    /// A source symbol operand.
    Symbol(SymbolId),
}

/// A form-local arena of interned Common Lisp types.
#[derive(Debug)]
pub struct TypeArena {
    nodes: Arena<ClType, TypeId>,
    interned: HashMap<ClTypeKind, TypeId>,
    any: TypeId,
    empty: TypeId,
}

impl TypeArena {
    /// Creates a type arena with canonical T and NIL nodes.
    ///
    /// # Errors
    ///
    /// Returns `ClTypeError::Id` if the first type identities cannot be represented.
    pub fn new(form: FormId) -> Result<Self, ClTypeError> {
        let mut nodes = Arena::new(form);
        let any_kind = ClTypeKind::Any;
        let empty_kind = ClTypeKind::Empty;
        let any = nodes.allocate(ClType {
            kind: any_kind.clone(),
        })?;
        let empty = nodes.allocate(ClType {
            kind: empty_kind.clone(),
        })?;
        let mut interned = HashMap::new();
        let _ = interned.insert(any_kind, any);
        let _ = interned.insert(empty_kind, empty);
        Ok(Self {
            nodes,
            interned,
            any,
            empty,
        })
    }

    /// Returns the identity of the universal type.
    #[must_use]
    pub const fn any(&self) -> TypeId {
        self.any
    }

    /// Returns the identity of the empty type.
    #[must_use]
    pub const fn empty(&self) -> TypeId {
        self.empty
    }

    /// Looks up a type belonging to this arena.
    #[must_use]
    pub fn get(&self, id: TypeId) -> Option<&ClType> {
        self.nodes.get(id)
    }

    /// Appends a recognized atomic type.
    ///
    /// # Errors
    ///
    /// Returns `ClTypeError::Id` if the arena is out of type identities.
    pub fn named(&mut self, name: BuiltinType) -> Result<TypeId, ClTypeError> {
        self.allocate(ClTypeKind::Named(name))
    }

    /// Appends a bounded numeric type.
    ///
    /// # Errors
    ///
    /// Returns `ClTypeError::Id` if the arena is out of type identities.
    pub fn numeric_range(&mut self, range: NumericRange) -> Result<TypeId, ClTypeError> {
        self.allocate(ClTypeKind::NumericRange(range))
    }

    /// Appends a finite type after removing duplicate EQL values.
    ///
    /// An empty member set is the empty type.
    ///
    /// # Errors
    ///
    /// Returns `ForeignSymbol` for a foreign symbol identity, `ForeignObject`
    /// for a foreign object identity, or `Id` on exhaustion.
    pub fn member(&mut self, values: Vec<MemberValue>) -> Result<TypeId, ClTypeError> {
        let mut distinct: Vec<MemberValue> = Vec::with_capacity(values.len());
        for value in values {
            let value = value.canonicalized();
            self.validate_member_owner(value)?;
            if !distinct.iter().any(|existing| existing.eql(value)) {
                distinct.push(value);
            }
        }
        if distinct.is_empty() {
            return Ok(self.empty);
        }
        self.allocate(ClTypeKind::Member(distinct))
    }

    /// Returns a canonical union of the supplied types.
    ///
    /// Nested unions are flattened, `T` absorbs every member, `NIL` is removed,
    /// and duplicate identities are removed. An empty input is NIL.
    ///
    /// # Errors
    ///
    /// Returns `UnknownType` if an identity is not in this arena, or `Id` on
    /// exhaustion.
    pub fn union<I>(&mut self, types: I) -> Result<TypeId, ClTypeError>
    where
        I: IntoIterator<Item = TypeId>,
    {
        let mut flattened = Vec::new();
        for id in types {
            let node = self.node(id)?;
            if id == self.any {
                return Ok(self.any);
            }
            if id == self.empty {
                continue;
            }
            if let ClTypeKind::Union(members) = node.kind() {
                flattened.extend(members.iter().copied());
            } else {
                flattened.push(id);
            }
        }
        self.canonical_boolean_type(flattened, true)
    }

    /// Returns a canonical intersection of the supplied types.
    ///
    /// Nested intersections are flattened, `NIL` absorbs every member, `T` is
    /// removed, and duplicate identities are removed. An empty input is T.
    ///
    /// # Errors
    ///
    /// Returns `UnknownType` if an identity is not in this arena, or `Id` on
    /// exhaustion.
    pub fn intersection<I>(&mut self, types: I) -> Result<TypeId, ClTypeError>
    where
        I: IntoIterator<Item = TypeId>,
    {
        let mut flattened = Vec::new();
        for id in types {
            let node = self.node(id)?;
            if id == self.empty {
                return Ok(self.empty);
            }
            if id == self.any {
                continue;
            }
            if let ClTypeKind::Intersection(members) = node.kind() {
                flattened.extend(members.iter().copied());
            } else {
                flattened.push(id);
            }
        }
        self.canonical_boolean_type(flattened, false)
    }

    /// Appends or simplifies a complement type.
    ///
    /// # Errors
    ///
    /// Returns `UnknownType` if the operand is not in this arena, or `Id` on
    /// exhaustion.
    pub fn complement(&mut self, operand: TypeId) -> Result<TypeId, ClTypeError> {
        let node = self.node(operand)?;
        if operand == self.any {
            return Ok(self.empty);
        }
        if operand == self.empty {
            return Ok(self.any);
        }
        if let ClTypeKind::Complement(inner) = node.kind() {
            return Ok(*inner);
        }
        self.allocate(ClTypeKind::Complement(operand))
    }

    /// Appends a CONS type.
    ///
    /// # Errors
    ///
    /// Returns `UnknownType` for an invalid component identity, or `Id` on
    /// exhaustion.
    pub fn cons(&mut self, car: TypeId, cdr: TypeId) -> Result<TypeId, ClTypeError> {
        self.validate_type(car)?;
        self.validate_type(cdr)?;
        self.allocate(ClTypeKind::Cons { car, cdr })
    }

    /// Appends an array type.
    ///
    /// # Errors
    ///
    /// Returns `UnknownType` for an invalid element identity, or `Id` on exhaustion.
    pub fn array(&mut self, array: ArrayType) -> Result<TypeId, ClTypeError> {
        if let Some(element_type) = array.element_type {
            self.validate_type(element_type)?;
        }
        self.allocate(ClTypeKind::Array(array))
    }

    /// Appends a function type.
    ///
    /// # Errors
    ///
    /// Returns `UnknownType` for an invalid parameter or return identity, or `Id`
    /// on exhaustion.
    pub fn function(&mut self, function: FunctionType) -> Result<TypeId, ClTypeError> {
        match &function.parameters {
            FunctionParameters::Any => {}
            FunctionParameters::Fixed(parameters) => {
                for parameter in parameters {
                    self.validate_type(*parameter)?;
                }
            }
        }
        self.validate_type(function.returns)?;
        self.allocate(ClTypeKind::Function(function))
    }

    /// Appends a fixed multiple-value return type.
    ///
    /// # Errors
    ///
    /// Returns `UnknownType` for an invalid value identity, or `Id` on exhaustion.
    pub fn multiple_values<I>(&mut self, values: I) -> Result<TypeId, ClTypeError>
    where
        I: IntoIterator<Item = TypeId>,
    {
        let values = values.into_iter().collect::<Vec<_>>();
        for value in &values {
            self.validate_type(*value)?;
        }
        self.allocate(ClTypeKind::MultipleValues(values))
    }

    /// Appends a user class identified by a source symbol.
    ///
    /// # Errors
    ///
    /// Returns `ForeignSymbol` for a symbol from another compilation unit, or
    /// `Id` on exhaustion.
    pub fn class(&mut self, class: SymbolId) -> Result<TypeId, ClTypeError> {
        self.validate_symbol(class)?;
        self.allocate(ClTypeKind::Class(class))
    }

    /// Retains an unresolved type specifier and all source operand identities.
    ///
    /// # Errors
    ///
    /// Returns `ForeignSymbol` for a foreign name, `ForeignConstant` for a
    /// constant from another form, or `UnknownType` for an invalid type operand.
    pub fn opaque<I>(&mut self, specifier: SymbolId, operands: I) -> Result<TypeId, ClTypeError>
    where
        I: IntoIterator<Item = OpaqueOperand>,
    {
        self.validate_symbol(specifier)?;
        let operands = operands.into_iter().collect::<Vec<_>>();
        for operand in &operands {
            match operand {
                OpaqueOperand::Type(id) => self.validate_type(*id)?,
                OpaqueOperand::Constant(id) if id.form() != self.nodes.owner() => {
                    return Err(ClTypeError::ForeignConstant(*id));
                }
                OpaqueOperand::Symbol(symbol) => self.validate_symbol(*symbol)?,
                OpaqueOperand::Constant(_) => {}
            }
        }
        self.allocate(ClTypeKind::Opaque {
            specifier,
            operands,
        })
    }

    /// Queries the conservative subtype relation for two types.
    ///
    /// # Errors
    ///
    /// Returns `UnknownType` if either identity is not in this arena.
    pub fn subtype(&self, sub: TypeId, sup: TypeId) -> Result<SubtypeRelation, ClTypeError> {
        relation::subtype(self, sub, sup)
    }

    fn canonical_boolean_type(
        &mut self,
        mut members: Vec<TypeId>,
        is_union: bool,
    ) -> Result<TypeId, ClTypeError> {
        members.sort_by_key(|id| id.index());
        members.dedup();
        if members.is_empty() {
            return Ok(if is_union { self.empty } else { self.any });
        }
        if members.len() == 1 {
            return members
                .into_iter()
                .next()
                .ok_or(ClTypeError::UnknownType(self.any));
        }
        let kind = if is_union {
            ClTypeKind::Union(members)
        } else {
            ClTypeKind::Intersection(members)
        };
        self.allocate(kind)
    }

    fn allocate(&mut self, kind: ClTypeKind) -> Result<TypeId, ClTypeError> {
        if let Some(id) = self.interned.get(&kind) {
            return Ok(*id);
        }
        let id = self.nodes.allocate(ClType { kind: kind.clone() })?;
        let _ = self.interned.insert(kind, id);
        Ok(id)
    }

    fn node(&self, id: TypeId) -> Result<&ClType, ClTypeError> {
        self.get(id).ok_or(ClTypeError::UnknownType(id))
    }

    fn validate_type(&self, id: TypeId) -> Result<(), ClTypeError> {
        self.node(id).map(|_| ())
    }

    fn validate_symbol(&self, symbol: SymbolId) -> Result<(), ClTypeError> {
        if symbol.compilation_unit() != self.nodes.owner().compilation_unit() {
            return Err(ClTypeError::ForeignSymbol(symbol));
        }
        Ok(())
    }

    fn validate_member_owner(&self, value: MemberValue) -> Result<(), ClTypeError> {
        match value {
            MemberValue::Symbol(symbol) => self.validate_symbol(symbol),
            MemberValue::OpaqueObject { owner, .. }
                if owner != self.nodes.owner().compilation_unit() =>
            {
                Err(ClTypeError::ForeignObject(owner))
            }
            _ => Ok(()),
        }
    }
}

/// Errors produced while constructing or querying the type lattice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ClTypeError {
    /// A type identity does not exist in the form-local type arena.
    UnknownType(TypeId),
    /// A symbol identity belongs to another compilation unit.
    ForeignSymbol(SymbolId),
    /// An opaque object identity belongs to another compilation unit.
    ForeignObject(crate::ids::CompilationUnitId),
    /// A source constant identity belongs to another form.
    ForeignConstant(TypeSpecConstId),
    /// A typed identity could not be represented.
    Id(IdError),
}

impl Display for ClTypeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "HIR type error: {self:?}")
    }
}

impl Error for ClTypeError {}

impl From<IdError> for ClTypeError {
    fn from(error: IdError) -> Self {
        Self::Id(error)
    }
}
