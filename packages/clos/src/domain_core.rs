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
        let mut parent_precedences = Vec::new();
        for parent_id in &self.direct_supers {
            let parent = parents
                .iter()
                .find(|parent| parent.id == *parent_id)
                .ok_or(DomainError::UnknownClass)?;
            if !parent.finalized {
                return Err(DomainError::UnfinalizedClass);
            }
            parent_precedences.push(parent.precedence.clone());
        }
        parent_precedences.push(self.direct_supers.clone());
        let mut precedence = vec![self.id];
        while parent_precedences.iter().any(|sequence| !sequence.is_empty()) {
            let candidate = parent_precedences
                .iter()
                .filter_map(|sequence| sequence.first().copied())
                .find(|candidate| {
                    !parent_precedences
                        .iter()
                        .any(|sequence| sequence.get(1..).is_some_and(|tail| tail.contains(candidate)))
                })
                .ok_or(DomainError::InvalidClassPrecedenceList)?;
            precedence.push(candidate);
            for sequence in &mut parent_precedences {
                if sequence.first() == Some(&candidate) {
                    sequence.remove(0);
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

#[cfg(test)]
mod additional_tests {
    #![allow(clippy::unwrap_used, reason = "coverage tests assert on domain results")]

    use super::*;

    #[test]
    fn value_accessors_and_class_redefinition_report_state() {
        let slot = SlotDefinition::new(SlotId::new(4), Allocation::Class, Some(19));
        assert_eq!(slot.id(), SlotId::new(4));
        assert_eq!(slot.allocation(), Allocation::Class);
        assert_eq!(slot.initarg(), Some(19));
        assert_eq!(slot.location(), None);

        let mut class = Class::new(ClassId::new(8), Vec::new(), vec![slot]);
        assert_eq!(class.id(), ClassId::new(8));
        assert!(class.direct_superclasses().is_empty());
        assert_eq!(class.version(), 0);
        class.finalize_inheritance(&[]).unwrap();
        assert_eq!(class.version(), 1);
        assert_eq!(class.effective_slots()[0].location(), Some(0));
        class.redefine(Vec::new());
        assert_eq!(class.version(), 2);
        assert!(class.class_precedence_list().is_empty());
        assert!(class.effective_slots().is_empty());
    }

    #[test]
    fn class_finalization_rejects_missing_unfinalized_and_duplicate_inputs() {
        let mut missing = Class::new(ClassId::new(2), vec![ClassId::new(99)], Vec::new());
        assert_eq!(
            missing.finalize_inheritance(&[]),
            Err(DomainError::UnknownClass)
        );

        let mut parent = Class::new(ClassId::new(1), Vec::new(), Vec::new());
        let mut child = Class::new(ClassId::new(2), vec![ClassId::new(1)], Vec::new());
        assert_eq!(
            child.finalize_inheritance(&[parent.clone()]),
            Err(DomainError::UnfinalizedClass)
        );
        parent.finalize_inheritance(&[]).unwrap();

        let slot = SlotDefinition::new(SlotId::new(7), Allocation::Instance, None);
        let mut duplicate = Class::new(
            ClassId::new(3),
            vec![ClassId::new(1)],
            vec![slot, slot],
        );
        assert_eq!(
            duplicate.finalize_inheritance(&[parent]),
            Err(DomainError::DuplicateSlot)
        );
    }

    #[test]
    fn generic_function_rejects_duplicates_and_missing_primary() {
        let mut generic = GenericFunction::default();
        let method = Method::new(
            MethodId::new(1),
            vec![Specializer::Class(ClassId::new(1))],
            MethodQualifier::Before,
            MethodId::new(10),
        );
        assert_eq!(method.id(), MethodId::new(1));
        assert_eq!(method.specializers(), &[Specializer::Class(ClassId::new(1))]);
        assert_eq!(method.qualifier(), MethodQualifier::Before);
        assert_eq!(method.body(), MethodId::new(10));
        generic.add_method(method).unwrap();
        assert_eq!(
            generic.add_method(Method::new(
                MethodId::new(2),
                vec![Specializer::Class(ClassId::new(1))],
                MethodQualifier::Before,
                MethodId::new(11),
            )),
            Err(DomainError::DuplicateMethod)
        );
        assert!(generic.find_method(&[], MethodQualifier::Primary).is_none());
        assert!(!generic.remove_method(MethodId::new(99)));
        assert_eq!(
            generic.compute_standard_method_combination(&[DispatchArgument::Class(ClassId::new(1))]),
            Err(DomainError::MissingPrimaryMethod)
        );
    }

    #[test]
    fn generic_function_filters_arity_and_specializer_kind() {
        let mut generic = GenericFunction::default();
        generic
            .add_method(Method::new(
                MethodId::new(1),
                vec![Specializer::Class(ClassId::new(1)), Specializer::Class(ClassId::new(2))],
                MethodQualifier::Primary,
                MethodId::new(10),
            ))
            .unwrap();
        generic
            .add_method(Method::new(
                MethodId::new(2),
                vec![Specializer::Eql(EqlValueId::new(3))],
                MethodQualifier::Primary,
                MethodId::new(11),
            ))
            .unwrap();
        assert!(generic.applicable_methods(&[ClassId::new(1)]).is_empty());
        assert!(generic
            .compute_applicable_methods(&[DispatchArgument::Class(ClassId::new(3))])
            .is_empty());
        assert_eq!(
            generic.compute_applicable_methods(&[DispatchArgument::Eql(EqlValueId::new(3))]),
            vec![MethodId::new(2)]
        );
    }

    #[test]
    fn class_finalization_deduplicates_slots_in_multiple_inheritance() {
        let slot = SlotDefinition::new(SlotId::new(5), Allocation::Instance, None);
        let mut left = Class::new(ClassId::new(1), Vec::new(), vec![slot]);
        let mut right = Class::new(ClassId::new(2), Vec::new(), vec![slot]);
        left.finalize_inheritance(&[]).unwrap();
        right.finalize_inheritance(&[]).unwrap();
        let mut child = Class::new(ClassId::new(3), vec![ClassId::new(1), ClassId::new(2)], Vec::new());
        assert_eq!(child.finalize_inheritance(&[left, right]), Ok(()));
        assert_eq!(child.effective_slots().len(), 1);
        assert_eq!(child.effective_slots()[0].location(), Some(0));
    }
}
