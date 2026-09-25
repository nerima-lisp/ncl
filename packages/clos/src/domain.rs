//! Pure CLOS domain aggregates.

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

/// A method entity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Method {
    id: MethodId,
    specializers: Vec<Specializer>,
    qualifier: MethodQualifier,
    body: MethodId,
}

impl Method {
    /// Create a method entity.
    #[must_use]
    pub const fn new(
        id: MethodId,
        specializers: Vec<Specializer>,
        qualifier: MethodQualifier,
        body: MethodId,
    ) -> Self {
        Self {
            id,
            specializers,
            qualifier,
            body,
        }
    }

    /// Return the method identity.
    #[must_use]
    pub const fn id(&self) -> MethodId {
        self.id
    }

    /// Return specializers in lambda-list order.
    #[must_use]
    pub fn specializers(&self) -> &[Specializer] {
        &self.specializers
    }

    /// Return the method qualifier.
    #[must_use]
    pub const fn qualifier(&self) -> MethodQualifier {
        self.qualifier
    }

    /// Return the adapter-owned body identity.
    #[must_use]
    pub const fn body(&self) -> MethodId {
        self.body
    }
}

/// Key for a dispatch cache entry.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DispatchKey {
    arguments: Vec<DispatchArgument>,
    generation: u64,
}

/// A generic function aggregate and its invalidatable dispatch cache.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GenericFunction {
    methods: Vec<Method>,
    cache: HashMap<DispatchKey, Vec<MethodId>>,
    generation: u64,
}

impl GenericFunction {
    /// Add a method and invalidate cached dispatch results.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::DuplicateMethod`] when the method signature is
    /// already present.
    pub fn add_method(&mut self, method: Method) -> Result<(), DomainError> {
        if self.methods.iter().any(|existing| {
            existing.specializers == method.specializers && existing.qualifier == method.qualifier
        }) {
            return Err(DomainError::DuplicateMethod);
        }
        self.methods.push(method);
        self.invalidate_cache();
        Ok(())
    }

    /// Find a method by its specializers and qualifier.
    #[must_use]
    pub fn find_method(
        &self,
        specializers: &[Specializer],
        qualifier: MethodQualifier,
    ) -> Option<&Method> {
        self.methods
            .iter()
            .find(|method| method.specializers == specializers && method.qualifier == qualifier)
    }

    /// Remove a method by identity and invalidate cached dispatch results.
    pub fn remove_method(&mut self, id: MethodId) -> bool {
        let before = self.methods.len();
        self.methods.retain(|method| method.id != id);
        if self.methods.len() == before {
            false
        } else {
            self.invalidate_cache();
            true
        }
    }

    /// Compute and cache applicable methods for a class tuple.
    pub fn applicable_methods(&mut self, classes: &[ClassId]) -> Vec<MethodId> {
        let arguments: Vec<_> = classes
            .iter()
            .copied()
            .map(DispatchArgument::Class)
            .collect();
        self.compute_applicable_methods(&arguments)
    }

    /// Compute and cache applicable methods for typed dispatch arguments.
    pub fn compute_applicable_methods(&mut self, arguments: &[DispatchArgument]) -> Vec<MethodId> {
        let key = DispatchKey {
            arguments: arguments.to_vec(),
            generation: self.generation,
        };
        if let Some(cached) = self.cache.get(&key) {
            return cached.clone();
        }
        let applicable: Vec<MethodId> =
            self.methods
                .iter()
                .filter(|method| {
                    method.specializers.len() == arguments.len()
                        && method.specializers.iter().enumerate().all(
                            |(index, specializer)| match (specializer, arguments.get(index)) {
                                (
                                    Specializer::Class(expected),
                                    Some(DispatchArgument::Class(actual)),
                                ) => expected == actual,
                                (
                                    Specializer::Eql(expected),
                                    Some(DispatchArgument::Eql(actual)),
                                ) => expected == actual,
                                _ => false,
                            },
                        )
                })
                .map(Method::id)
                .collect();
        self.cache.insert(key, applicable.clone());
        applicable
    }

    /// Compute the standard primary/before/after/around method combination.
    pub fn compute_standard_method_combination(
        &mut self,
        arguments: &[DispatchArgument],
    ) -> Result<StandardMethodCombination, DomainError> {
        let applicable = self.compute_applicable_methods(arguments);
        let mut combination = StandardMethodCombination {
            around: Vec::new(),
            before: Vec::new(),
            primary: Vec::new(),
            after: Vec::new(),
        };
        for method_id in applicable {
            let method = self
                .methods
                .iter()
                .find(|method| method.id == method_id)
                .ok_or(DomainError::UnknownMethod)?;
            match method.qualifier {
                MethodQualifier::Around => combination.around.push(method_id),
                MethodQualifier::Before => combination.before.push(method_id),
                MethodQualifier::Primary => combination.primary.push(method_id),
                MethodQualifier::After => combination.after.push(method_id),
            }
        }
        combination.after.reverse();
        if combination.primary.is_empty() {
            return Err(DomainError::MissingPrimaryMethod);
        }
        Ok(combination)
    }

    /// Invalidate dispatch after a class redefinition or method change.
    pub fn invalidate_cache(&mut self) {
        self.generation = self.generation.saturating_add(1);
        self.cache.clear();
    }

    /// Invalidate entries affected by a class redefinition.
    pub fn invalidate_for_class_redefinition(&mut self, _class: ClassId) {
        self.invalidate_cache();
    }

    /// Return the number of cached dispatch tuples.
    #[must_use]
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finalization_assigns_effective_slot_locations() {
        let slot = SlotDefinition::new(SlotId::new(1), Allocation::Instance, Some(7));
        let mut class = Class::new(ClassId::new(1), Vec::new(), vec![slot]);
        assert!(class.finalize_inheritance(&[]).is_ok());
        assert_eq!(class.class_precedence_list(), &[ClassId::new(1)]);
        assert_eq!(class.effective_slots()[0].location(), Some(0));
    }

    #[test]
    fn method_changes_invalidate_dispatch_cache() {
        let mut generic = GenericFunction::default();
        let method = Method::new(
            MethodId::new(1),
            vec![Specializer::Class(ClassId::new(1))],
            MethodQualifier::Primary,
            MethodId::new(9),
        );
        assert!(generic.add_method(method).is_ok());
        assert_eq!(
            generic.applicable_methods(&[ClassId::new(1)]),
            vec![MethodId::new(1)]
        );
        assert_eq!(generic.cache_len(), 1);
        assert!(generic.remove_method(MethodId::new(1)));
        assert!(generic.applicable_methods(&[ClassId::new(1)]).is_empty());
        assert_eq!(generic.cache_len(), 1);
    }

    #[test]
    fn typed_dispatch_distinguishes_class_and_eql_specializers() {
        let mut generic = GenericFunction::default();
        let class_method = Method::new(
            MethodId::new(1),
            vec![Specializer::Class(ClassId::new(7))],
            MethodQualifier::Primary,
            MethodId::new(11),
        );
        let eql_method = Method::new(
            MethodId::new(2),
            vec![Specializer::Eql(EqlValueId::new(7))],
            MethodQualifier::Primary,
            MethodId::new(12),
        );
        assert!(generic.add_method(class_method).is_ok());
        assert!(generic.add_method(eql_method).is_ok());
        assert_eq!(
            generic.compute_applicable_methods(&[DispatchArgument::Class(ClassId::new(7))]),
            vec![MethodId::new(1)]
        );
        assert_eq!(generic.cache_len(), 1);
        assert_eq!(
            generic.compute_applicable_methods(&[DispatchArgument::Eql(EqlValueId::new(7))]),
            vec![MethodId::new(2)]
        );
        assert_eq!(generic.cache_len(), 2);
        assert!(
            generic
                .find_method(
                    &[Specializer::Eql(EqlValueId::new(7))],
                    MethodQualifier::Primary,
                )
                .is_some()
        );
    }

    #[test]
    fn standard_method_combination_groups_and_orders_qualifiers() {
        let mut generic = GenericFunction::default();
        for (id, qualifier) in [
            (1, MethodQualifier::After),
            (2, MethodQualifier::Before),
            (3, MethodQualifier::Primary),
            (4, MethodQualifier::Around),
        ] {
            assert!(
                generic
                    .add_method(Method::new(
                        MethodId::new(id),
                        vec![Specializer::Class(ClassId::new(1))],
                        qualifier,
                        MethodId::new(id + 10),
                    ))
                    .is_ok()
            );
        }
        let combination = generic
            .compute_standard_method_combination(&[DispatchArgument::Class(ClassId::new(1))])
            .expect("primary method exists");
        assert_eq!(combination.around(), &[MethodId::new(4)]);
        assert_eq!(combination.before(), &[MethodId::new(2)]);
        assert_eq!(combination.primary(), &[MethodId::new(3)]);
        assert_eq!(combination.after(), &[MethodId::new(1)]);
        assert_eq!(
            combination.ordered_method_ids(),
            vec![
                MethodId::new(4),
                MethodId::new(2),
                MethodId::new(3),
                MethodId::new(1),
            ]
        );
    }
}
