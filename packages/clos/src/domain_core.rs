/// Pure CLOS domain aggregates.
use std::collections::HashMap;

/// Stable identifier for a class in the CLOS domain model.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
pub struct ClassId(u32);

impl ClassId {
    /// Construct an identifier at an integration boundary.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

/// Stable identifier for a slot definition.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
pub struct SlotId(u32);

impl SlotId {
    /// Construct an identifier at an integration boundary.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

/// Stable identifier for a method body owned by the runtime adapter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
pub struct MethodId(u32);

impl MethodId {
    /// Construct an identifier at an integration boundary.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

/// Stable identifier for an eql-specialized value.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
pub struct EqlValueId(u32);

impl EqlValueId {
    /// Construct an identifier at an integration boundary.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

/// The allocation policy of a slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Allocation {
    /// A value stored in each instance.
    Instance,
    /// A value shared by the class.
    Class,
}

/// A slot definition entity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlotDefinition {
    id: SlotId,
    allocation: Allocation,
    initarg: Option<u32>,
    location: Option<u32>,
}

impl SlotDefinition {
    /// Create an unlocated slot. Finalization assigns its instance location.
    #[must_use]
    pub const fn new(id: SlotId, allocation: Allocation, initarg: Option<u32>) -> Self {
        Self {
            id,
            allocation,
            initarg,
            location: None,
        }
    }

    /// Return the entity identifier.
    #[must_use]
    pub const fn id(self) -> SlotId {
        self.id
    }

    /// Return the slot allocation policy.
    #[must_use]
    pub const fn allocation(self) -> Allocation {
        self.allocation
    }

    /// Return the accepted initarg identifier, if any.
    #[must_use]
    pub const fn initarg(self) -> Option<u32> {
        self.initarg
    }

    /// Return the finalized instance location.
    #[must_use]
    pub const fn location(self) -> Option<u32> {
        self.location
    }

    const fn assign_location(self, location: u32) -> Self {
        Self {
            location: Some(location),
            ..self
        }
    }
}

/// Errors enforcing aggregate invariants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DomainError {
    /// Two effective slots use the same identifier.
    DuplicateSlot,
    /// Two methods have the same specializers and qualifier.
    DuplicateMethod,
    /// The computed precedence list is invalid.
    InvalidClassPrecedenceList,
    /// A superclass has not been finalized.
    UnfinalizedClass,
    /// A referenced superclass is absent.
    UnknownClass,
    /// A method combination has no primary method.
    MissingPrimaryMethod,
    /// A dispatch result refers to a method not held by its generic function.
    UnknownMethod,
}

/// A class aggregate with finalized precedence and effective slots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Class {
    id: ClassId,
    direct_supers: Vec<ClassId>,
    direct_slots: Vec<SlotDefinition>,
    precedence: Vec<ClassId>,
    effective_slots: Vec<SlotDefinition>,
    finalized: bool,
    version: u64,
}

impl Class {
    /// Create a class aggregate before inheritance finalization.
    #[must_use]
    pub const fn new(
        id: ClassId,
        direct_supers: Vec<ClassId>,
        direct_slots: Vec<SlotDefinition>,
    ) -> Self {
        Self {
            id,
            direct_supers,
            direct_slots,
            precedence: Vec::new(),
            effective_slots: Vec::new(),
            finalized: false,
            version: 0,
        }
    }

    /// Return the class identifier.
    #[must_use]
    pub const fn id(&self) -> ClassId {
        self.id
    }

    /// Return the direct superclasses.
    #[must_use]
    pub fn direct_superclasses(&self) -> &[ClassId] {
        &self.direct_supers
    }

    /// Return the effective class precedence list.
    #[must_use]
    pub fn class_precedence_list(&self) -> &[ClassId] {
        &self.precedence
    }

    /// Return the finalized effective slots.
    #[must_use]
    pub fn effective_slots(&self) -> &[SlotDefinition] {
        &self.effective_slots
    }

    /// Return the redefinition generation used by dispatch caches.
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Finalize a class using already-finalized parent aggregates.
    ///
    /// # Errors
    ///
    /// Returns an error when a superclass is absent or unfinalized, or when a
    /// slot identifier is duplicated.
    pub fn finalize_inheritance(&mut self, parents: &[Self]) -> Result<(), DomainError> {
        let mut precedence = vec![self.id];
        for parent_id in &self.direct_supers {
            let parent = parents
                .iter()
                .find(|parent| parent.id == *parent_id)
                .ok_or(DomainError::UnknownClass)?;
            if !parent.finalized {
                return Err(DomainError::UnfinalizedClass);
            }
            for ancestor in &parent.precedence {
                if !precedence.contains(ancestor) {
                    precedence.push(*ancestor);
                }
            }
        }
        let mut slots = Vec::new();
        for parent_id in &self.direct_supers {
            let parent = parents
                .iter()
                .find(|parent| parent.id == *parent_id)
                .ok_or(DomainError::UnknownClass)?;
            for slot in &parent.effective_slots {
                if slots
                    .iter()
                    .any(|existing: &SlotDefinition| existing.id == slot.id)
                {
                    continue;
                }
                slots.push(*slot);
            }
        }
        for slot in &self.direct_slots {
            if slots
                .iter()
                .any(|existing: &SlotDefinition| existing.id == slot.id)
            {
                return Err(DomainError::DuplicateSlot);
            }
            slots.push(*slot);
        }
        self.effective_slots = slots
            .into_iter()
            .enumerate()
            .map(|(index, slot)| {
                u32::try_from(index)
                    .map(|location| slot.assign_location(location))
                    .map_err(|_| DomainError::InvalidClassPrecedenceList)
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.precedence = precedence;
        self.finalized = true;
        self.version = self.version.saturating_add(1);
        Ok(())
    }

    /// Redefine direct slots and invalidate all dependent dispatch entries.
    pub fn redefine(&mut self, direct_slots: Vec<SlotDefinition>) {
        self.direct_slots = direct_slots;
        self.precedence.clear();
        self.effective_slots.clear();
        self.finalized = false;
        self.version = self.version.saturating_add(1);
    }
}

/// A method qualifier in standard method combination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MethodQualifier {
    /// The primary method.
    Primary,
    /// Runs before the primary method.
    Before,
    /// Runs after the primary method.
    After,
    /// Wraps the primary method.
    Around,
}

/// A method specializer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Specializer {
    /// A class specializer.
    Class(ClassId),
    /// An EQL value specializer.
    Eql(EqlValueId),
}

/// A typed argument used by generic-function dispatch.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum DispatchArgument {
    /// The argument's runtime class.
    Class(ClassId),
    /// The argument's identity for an EQL specializer.
    Eql(EqlValueId),
}

/// The methods selected by standard method combination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandardMethodCombination {
    around: Vec<MethodId>,
    before: Vec<MethodId>,
    primary: Vec<MethodId>,
    after: Vec<MethodId>,
}

impl StandardMethodCombination {
    /// Methods wrapping the complete primary invocation, most-specific first.
    #[must_use]
    pub fn around(&self) -> &[MethodId] {
        &self.around
    }

    /// Methods run before the primary invocation, most-specific first.
    #[must_use]
    pub fn before(&self) -> &[MethodId] {
        &self.before
    }

    /// Primary methods, most-specific first.
    #[must_use]
    pub fn primary(&self) -> &[MethodId] {
        &self.primary
    }

    /// Methods run after the primary invocation, least-specific first.
    #[must_use]
    pub fn after(&self) -> &[MethodId] {
        &self.after
    }

    /// Return the linear execution order of the non-around methods.
    #[must_use]
    pub fn ordered_method_ids(&self) -> Vec<MethodId> {
        self.around
            .iter()
            .copied()
            .chain(self.before.iter().copied())
            .chain(self.primary.iter().copied())
            .chain(self.after.iter().copied())
            .collect()
    }
}
