//! Fact epochs and release-verifier policy.

use crate::ids::{FormId, IdError, NodeId, SourceOriginId, TypeId, TypeSpecConstId, ValueId};

use super::{Verifier, VerifyError};

/// A source domain whose mutations invalidate cached facts.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FactDomain {
    /// Inferred Common Lisp value types.
    Type,
    /// Observable effects and ordering.
    Effect,
    /// Escape and capture properties.
    Escape,
    /// Dominance facts.
    Dominance,
    /// CFG reachability and edge facts.
    ControlFlow,
}

/// The source construct or analysis that produced a cached fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FactProvenance {
    /// A retained source declaration.
    Declared,
    /// An explicit source assertion.
    Asserted,
    /// A compiler analysis result.
    Inferred,
    /// An explicit runtime guard.
    Guard,
}

/// Whether a cached fact is established or unresolved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FactCertainty {
    /// The analysis established the fact.
    Proven,
    /// The analysis could not establish the fact.
    Unknown,
}

/// A check-elision claim or a fact retained for analysis only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FactAuthorization {
    /// This fact does not authorize removing a runtime check.
    Informational,
    /// The proof offered for removing this exact type check.
    ElideTypeCheck {
        /// The SSA value whose dynamic check was removed.
        value: ValueId,
        /// The proven abstract type of the value.
        value_type: TypeId,
        /// The type accepted by the removed check.
        accepted_type: TypeId,
        /// The complete source type-specifier constant.
        expected_type: TypeSpecConstId,
        /// An equivalent earlier guard enforces the same condition.
        equivalent_guard: Option<NodeId>,
    },
}

/// Identity of one fact in a form-local cache.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FactId {
    pub(super) form: FormId,
    pub(super) index: u32,
}

impl FactId {
    /// Returns the form that owns this fact identity.
    #[must_use]
    pub const fn form(self) -> FormId {
        self.form
    }

    /// Returns this fact's index in its form-local store.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }
}

/// A cached analysis result with source epoch and provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FactRecord {
    pub(super) domain: FactDomain,
    pub(super) epoch: u64,
    pub(super) source_origin: SourceOriginId,
    pub(super) provenance: FactProvenance,
    pub(super) certainty: FactCertainty,
    pub(super) authorization: FactAuthorization,
}

impl FactRecord {
    /// Creates a fact that the verifier checks against the current epoch.
    #[must_use]
    pub const fn new(
        domain: FactDomain,
        epoch: u64,
        source_origin: SourceOriginId,
        provenance: FactProvenance,
        certainty: FactCertainty,
        authorization: FactAuthorization,
    ) -> Self {
        Self {
            domain,
            epoch,
            source_origin,
            provenance,
            certainty,
            authorization,
        }
    }

    /// Returns the IR domain this fact describes.
    #[must_use]
    pub const fn domain(self) -> FactDomain {
        self.domain
    }

    /// Returns the source epoch against which the fact was computed.
    #[must_use]
    pub const fn epoch(self) -> u64 {
        self.epoch
    }

    /// Returns the source origin that contributed the fact.
    #[must_use]
    pub const fn source_origin(self) -> SourceOriginId {
        self.source_origin
    }

    /// Returns whether the fact came from source or analysis.
    #[must_use]
    pub const fn provenance(self) -> FactProvenance {
        self.provenance
    }

    /// Returns the analysis certainty.
    #[must_use]
    pub const fn certainty(self) -> FactCertainty {
        self.certainty
    }

    /// Returns the operation this fact authorizes, if any.
    #[must_use]
    pub const fn authorization(self) -> FactAuthorization {
        self.authorization
    }
}

/// Form-local cache of facts that optimizations may consume.
#[derive(Debug)]
pub struct FactStore {
    pub(super) form: FormId,
    records: Vec<FactRecord>,
}

impl FactStore {
    /// Creates an empty fact store for one form.
    #[must_use]
    pub const fn new(form: FormId) -> Self {
        Self {
            form,
            records: Vec::new(),
        }
    }

    /// Adds a fact and returns its checked identity.
    ///
    /// # Errors
    ///
    /// Returns VerifyError::Id when the fact index space is exhausted.
    pub fn add(&mut self, fact: FactRecord) -> Result<FactId, VerifyError> {
        let index = u32::try_from(self.records.len()).map_err(|_| {
            VerifyError::Id(IdError::IndexOverflow {
                index: self.records.len(),
            })
        })?;
        let id = FactId {
            form: self.form,
            index,
        };
        self.records.push(fact);
        Ok(id)
    }

    /// Returns a fact when the ID belongs to this store.
    #[must_use]
    pub fn get(&self, id: FactId) -> Option<&FactRecord> {
        if id.form != self.form {
            return None;
        }
        self.records.get(usize::try_from(id.index).ok()?)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (FactId, FactRecord)> + '_ {
        self.records
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(index, fact)| {
                u32::try_from(index).ok().map(|index| {
                    (
                        FactId {
                            form: self.form,
                            index,
                        },
                        fact,
                    )
                })
            })
    }
}

/// Current epoch for each HIR analysis domain.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FactEpochs {
    type_epoch: u64,
    effect_epoch: u64,
    escape_epoch: u64,
    dominance_epoch: u64,
    control_flow_epoch: u64,
}

impl FactEpochs {
    /// Returns the current epoch for a source domain.
    #[must_use]
    pub const fn current(self, domain: FactDomain) -> u64 {
        match domain {
            FactDomain::Type => self.type_epoch,
            FactDomain::Effect => self.effect_epoch,
            FactDomain::Escape => self.escape_epoch,
            FactDomain::Dominance => self.dominance_epoch,
            FactDomain::ControlFlow => self.control_flow_epoch,
        }
    }

    /// Applies a pass's declared invalidation set.
    ///
    /// # Errors
    ///
    /// Returns EpochOverflow if a domain cannot be advanced.
    pub fn after_pass(self, invalidates: &PassInvalidations) -> Result<Self, VerifyError> {
        let mut next = self;
        for domain in invalidates.domains.iter().copied() {
            let epoch = match domain {
                FactDomain::Type => &mut next.type_epoch,
                FactDomain::Effect => &mut next.effect_epoch,
                FactDomain::Escape => &mut next.escape_epoch,
                FactDomain::Dominance => &mut next.dominance_epoch,
                FactDomain::ControlFlow => &mut next.control_flow_epoch,
            };
            *epoch = epoch
                .checked_add(1)
                .ok_or(VerifyError::EpochOverflow(domain))?;
        }
        Ok(next)
    }
}

/// The source domains a compiler pass declares it may mutate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassInvalidations {
    domains: Vec<FactDomain>,
}

impl PassInvalidations {
    /// Creates a deduplicated invalidation declaration.
    #[must_use]
    pub fn new(domains: impl IntoIterator<Item = FactDomain>) -> Self {
        let mut unique = Vec::new();
        for domain in domains {
            if !unique.contains(&domain) {
                unique.push(domain);
            }
        }
        Self { domains: unique }
    }
}

/// Compiler build profile used for D-10 verifier defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationBuild {
    /// Unit and integration tests always verify.
    Test,
    /// CI always verifies.
    Ci,
    /// Release verification can default off after both gates pass.
    Release,
}

/// Release-gate state and explicit verifier opt-in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerificationConfig {
    build: VerificationBuild,
    v2_passed: bool,
    v3_passed: bool,
    force_enabled: bool,
}

impl VerificationConfig {
    /// Creates verifier settings for a build and its V2/V3 gate state.
    #[must_use]
    pub const fn new(
        build: VerificationBuild,
        v2_passed: bool,
        v3_passed: bool,
        force_enabled: bool,
    ) -> Self {
        Self {
            build,
            v2_passed,
            v3_passed,
            force_enabled,
        }
    }

    /// Whether this build runs the verifier by default or explicit opt-in.
    #[must_use]
    pub const fn enabled(self) -> bool {
        match self.build {
            VerificationBuild::Test | VerificationBuild::Ci => true,
            VerificationBuild::Release => self.force_enabled || !(self.v2_passed && self.v3_passed),
        }
    }
}

use crate::cl_type::SubtypeRelation;
use crate::policy::OptimizationPolicy;

impl Verifier<'_> {
    pub(super) fn verify_facts(&self) -> Result<(), VerifyError> {
        for (id, fact) in self.input.facts.iter() {
            self.origin(fact.source_origin)
                .map_err(|_| VerifyError::InvalidFactOrigin(id))?;
            if fact.epoch != self.input.epochs.current(fact.domain) {
                return Err(VerifyError::StaleFact(id));
            }
            if let FactAuthorization::ElideTypeCheck {
                value,
                value_type,
                accepted_type,
                expected_type,
                equivalent_guard,
            } = fact.authorization
            {
                self.value(value)?;
                self.ty(value_type)?;
                self.ty(accepted_type)?;
                self.type_constant(expected_type)?;
                if fact.domain != FactDomain::Type || fact.certainty == FactCertainty::Unknown {
                    return Err(VerifyError::UnprovenCheckElision(id));
                }
                let relation = self
                    .input
                    .types
                    .subtype(value_type, accepted_type)
                    .map_err(|_| VerifyError::InvalidType(value_type))?;
                let has_equivalent_guard = if let Some(guard) = equivalent_guard {
                    self.check_edges.get(&guard) == Some(&1)
                        && matches!(
                            &self.node_ref(guard)?.kind,
                            crate::node::NodeKind::CheckType {
                                value: guard_value,
                                accepted_type: guard_type,
                                expected_type: guard_expected,
                                ..
                            } if *guard_value == value
                                && *guard_type == accepted_type
                                && *guard_expected == expected_type
                        )
                } else {
                    false
                };
                if equivalent_guard.is_some() && !has_equivalent_guard {
                    return Err(VerifyError::InvalidCheckElision(id));
                }
                if relation != SubtypeRelation::Yes && !has_equivalent_guard {
                    return Err(VerifyError::UnprovenCheckElision(id));
                }
                if matches!(
                    fact.provenance,
                    FactProvenance::Declared | FactProvenance::Asserted
                ) || (fact.provenance == FactProvenance::Guard) != has_equivalent_guard
                {
                    return Err(VerifyError::InvalidCheckElision(id));
                }
                if self
                    .input
                    .policy
                    .requires_type_check(relation, has_equivalent_guard)
                {
                    return Err(VerifyError::InvalidCheckElision(id));
                }
            }
        }
        Ok(())
    }
}
