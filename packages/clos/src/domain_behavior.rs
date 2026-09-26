
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
    ///
    /// # Errors
    /// Returns an error when an applicable method is missing or no primary
    /// method is available.
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
    fn finalization_uses_c3_precedence_for_a_diamond() {
        let mut left = Class::new(ClassId::new(2), vec![ClassId::new(1)], Vec::new());
        let mut right = Class::new(ClassId::new(3), vec![ClassId::new(1)], Vec::new());
        let root = Class::new(ClassId::new(1), Vec::new(), Vec::new());
        let mut root = root;
        assert!(root.finalize_inheritance(&[]).is_ok());
        assert!(left.finalize_inheritance(&[root.clone()]).is_ok());
        assert!(right.finalize_inheritance(&[root]).is_ok());
        let mut leaf = Class::new(
            ClassId::new(4),
            vec![ClassId::new(2), ClassId::new(3)],
            Vec::new(),
        );
        assert!(leaf.finalize_inheritance(&[left, right]).is_ok());
        assert_eq!(
            leaf.class_precedence_list(),
            &[ClassId::new(4), ClassId::new(2), ClassId::new(3), ClassId::new(1)]
        );
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
        let result = generic
            .compute_standard_method_combination(&[DispatchArgument::Class(ClassId::new(1))]);
        assert!(result.is_ok());
        let Ok(combination) = result else { return };
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
