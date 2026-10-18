//! The immutable revision catalog: the one place an experiment's lineage and the domain's
//! active harness pointer are kept.
//!
//! Two invariants this module exists to make mechanical:
//!
//! * **A revision is never edited.** [`RevisionStore`] stores whole [`HarnessRevision`] values;
//!   the only mutation is a lifecycle-status write applied by the catalog itself after a legal
//!   transition. A manifest, its hash and its parent never change.
//! * **The active harness changes by one pointer swap.** [`HarnessCatalog::activate`] validates
//!   everything first and only then moves the pointer, records the activation and writes the two
//!   statuses. A refused activation leaves the catalog exactly as it was. [`HarnessCatalog::rollback`]
//!   pops the recorded prior revision, so a rollback restores the exact revision that was active
//!   before — same id, same manifest hash — instead of rebuilding one.
//!
//! Persistence v1 is [`InMemoryRevisionStore`]. A database store can implement the same trait
//! (migration 294 is the table shape); nothing in this module assumes a process-local lifetime.

use std::collections::BTreeMap;
use std::fmt;

use super::model::{HarnessDomain, HarnessRevision, RevisionStatus};

/// One durable revision store. The catalog owns lifecycle policy; the store owns bytes.
pub trait RevisionStore {
    /// Insert a revision. Implementations must refuse a duplicate id rather than replace a
    /// revision: replacing one would make history mutable.
    fn insert(&mut self, revision: HarnessRevision) -> Result<(), CatalogError>;

    fn get(&self, revision_id: &str) -> Option<&HarnessRevision>;

    /// Every stored revision. Used for domain-scoped reads; order is the store's choice.
    fn all(&self) -> Vec<&HarnessRevision>;

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Write the lifecycle status of an already-stored revision. Only the catalog calls this,
    /// and only after [`RevisionStatus::can_transition_to`] has approved the move.
    fn set_status(
        &mut self,
        revision_id: &str,
        status: RevisionStatus,
    ) -> Result<(), CatalogError>;
}

/// The v1 store: a process-local map. Useful for tests and for one experiment process; a durable
/// store implementing the same trait is the production shape (migration 294).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InMemoryRevisionStore {
    revisions: BTreeMap<String, HarnessRevision>,
}

impl InMemoryRevisionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn revisions(&self) -> impl Iterator<Item = &HarnessRevision> {
        self.revisions.values()
    }
}

impl RevisionStore for InMemoryRevisionStore {
    fn insert(&mut self, revision: HarnessRevision) -> Result<(), CatalogError> {
        if revision.id.trim().is_empty() {
            return Err(CatalogError::EmptyRevisionId);
        }
        if self.revisions.contains_key(&revision.id) {
            return Err(CatalogError::DuplicateRevision {
                revision_id: revision.id,
            });
        }
        self.revisions.insert(revision.id.clone(), revision);
        Ok(())
    }

    fn get(&self, revision_id: &str) -> Option<&HarnessRevision> {
        self.revisions.get(revision_id)
    }

    fn all(&self) -> Vec<&HarnessRevision> {
        self.revisions.values().collect()
    }

    fn len(&self) -> usize {
        self.revisions.len()
    }

    fn set_status(
        &mut self,
        revision_id: &str,
        status: RevisionStatus,
    ) -> Result<(), CatalogError> {
        let revision = self.revisions.get_mut(revision_id).ok_or_else(|| {
            CatalogError::UnknownRevision {
                revision_id: revision_id.to_string(),
            }
        })?;
        revision.status = status;
        Ok(())
    }
}

/// Why a catalog operation was refused. Every variant is a refusal: no operation degrades into a
/// partially applied state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    EmptyRevisionId,
    UnknownRevision {
        revision_id: String,
    },
    DuplicateRevision {
        revision_id: String,
    },
    DomainMismatch {
        revision_id: String,
        expected: HarnessDomain,
        actual: HarnessDomain,
    },
    ParentMissing {
        revision_id: String,
        parent_revision_id: String,
    },
    ParentDomainMismatch {
        revision_id: String,
        parent_domain: HarnessDomain,
        child_domain: HarnessDomain,
    },
    /// The stored manifest does not hash to the stored `manifest_hash`.
    Integrity(String),
    RevisionNotActivable {
        revision_id: String,
        status: RevisionStatus,
    },
    AlreadyActive {
        domain: HarnessDomain,
        revision_id: String,
    },
    NoActiveRevision {
        domain: HarnessDomain,
    },
    NoPriorRevision {
        domain: HarnessDomain,
    },
    IllegalStatusTransition {
        revision_id: String,
        from: RevisionStatus,
        to: RevisionStatus,
    },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRevisionId => write!(f, "revision id is empty"),
            Self::UnknownRevision { revision_id } => {
                write!(f, "unknown harness revision: {revision_id}")
            }
            Self::DuplicateRevision { revision_id } => {
                write!(f, "harness revision {revision_id} already exists")
            }
            Self::DomainMismatch {
                revision_id,
                expected,
                actual,
            } => write!(
                f,
                "revision {revision_id} belongs to domain {actual}, not {expected}"
            ),
            Self::ParentMissing {
                revision_id,
                parent_revision_id,
            } => write!(
                f,
                "revision {revision_id} names parent {parent_revision_id}, which does not exist"
            ),
            Self::ParentDomainMismatch {
                revision_id,
                parent_domain,
                child_domain,
            } => write!(
                f,
                "revision {revision_id} is {child_domain} but its parent is {parent_domain}"
            ),
            Self::Integrity(message) => write!(f, "revision integrity failure: {message}"),
            Self::RevisionNotActivable {
                revision_id,
                status,
            } => write!(
                f,
                "revision {revision_id} is {status} and may not become active"
            ),
            Self::AlreadyActive {
                domain,
                revision_id,
            } => write!(
                f,
                "revision {revision_id} is already the active harness for domain {domain}"
            ),
            Self::NoActiveRevision { domain } => {
                write!(f, "domain {domain} has no active harness revision")
            }
            Self::NoPriorRevision { domain } => {
                write!(f, "domain {domain} has no prior revision to roll back to")
            }
            Self::IllegalStatusTransition {
                revision_id,
                from,
                to,
            } => write!(
                f,
                "revision {revision_id} may not move from {from} to {to}"
            ),
        }
    }
}

impl std::error::Error for CatalogError {}

impl From<super::model::EvolutionError> for CatalogError {
    fn from(error: super::model::EvolutionError) -> Self {
        Self::Integrity(error.to_string())
    }
}

/// What produced an activation pointer move. Both are explicit; there is no "passive" activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationKind {
    /// An adoption decision moved the pointer to a revision.
    Adopt,
    /// A rollback restored the revision that was active before.
    Rollback,
}

impl ActivationKind {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Adopt => "adopt",
            Self::Rollback => "rollback",
        }
    }
}

/// One recorded pointer swap. This is the audit row for "which harness was active, and why".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activation {
    pub domain: HarnessDomain,
    /// Monotonic per-catalog sequence (persist as the activation row order).
    pub sequence: u64,
    pub kind: ActivationKind,
    /// The revision that was active before; `None` for the first bootstrap activation.
    pub from_revision: Option<String>,
    pub to_revision: String,
    /// The adoption/rollback decision id that justified it.
    pub decision_id: String,
    pub activated_by: String,
}

/// The active pointer for one domain, plus the stack of revisions it displaced. A stack (not a
/// single value) is what makes more than one rollback possible while still restoring the exact
/// prior revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPointer {
    pub revision_id: String,
    pub prior: Vec<String>,
}

/// The revision catalog: immutable revisions, one active pointer per domain, recorded history.
///
/// Generic over [`RevisionStore`] with the in-memory store as the default, so tests are pure and
/// a durable store can be dropped in without changing orchestration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessCatalog<S = InMemoryRevisionStore> {
    store: S,
    active: BTreeMap<HarnessDomain, DomainPointer>,
    activations: Vec<Activation>,
    sequence: u64,
}

impl Default for HarnessCatalog<InMemoryRevisionStore> {
    fn default() -> Self {
        Self::new()
    }
}

impl HarnessCatalog<InMemoryRevisionStore> {
    pub fn new() -> Self {
        Self::with_store(InMemoryRevisionStore::new())
    }
}

impl<S: RevisionStore> HarnessCatalog<S> {
    pub fn with_store(store: S) -> Self {
        Self {
            store,
            active: BTreeMap::new(),
            activations: Vec::new(),
            sequence: 0,
        }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    /// Insert an immutable revision.
    ///
    /// Validation happens before the store is touched: a revision with a tampered hash, an empty
    /// id, a duplicate id, or a parent that does not exist (or belongs to another domain) is
    /// refused whole. The parent id is what records lineage; the stored manifest hash is what
    /// makes the revision content-addressed.
    pub fn insert(&mut self, revision: HarnessRevision) -> Result<(), CatalogError> {
        revision
            .verify_integrity()
            .map_err(|error| CatalogError::Integrity(error.to_string()))?;
        if revision.id.trim().is_empty() {
            return Err(CatalogError::EmptyRevisionId);
        }
        if let Some(parent_id) = revision.parent_revision_id.as_deref() {
            let parent = self
                .store
                .get(parent_id)
                .ok_or_else(|| CatalogError::ParentMissing {
                    revision_id: revision.id.clone(),
                    parent_revision_id: parent_id.to_string(),
                })?;
            if parent.domain != revision.domain {
                return Err(CatalogError::ParentDomainMismatch {
                    revision_id: revision.id.clone(),
                    parent_domain: parent.domain,
                    child_domain: revision.domain,
                });
            }
            if parent.manifest_hash.trim().is_empty() {
                return Err(CatalogError::Integrity(format!(
                    "parent {parent_id} has no manifest hash"
                )));
            }
        }
        self.store.insert(revision)
    }

    pub fn revision(&self, revision_id: &str) -> Option<&HarnessRevision> {
        self.store.get(revision_id)
    }

    /// Write a lifecycle status on an existing revision, refusing an illegal transition. This is
    /// the only status-write path: it cannot change a manifest or a hash, only the lifecycle.
    pub fn set_status(
        &mut self,
        revision_id: &str,
        next: RevisionStatus,
    ) -> Result<(), CatalogError> {
        let revision = self
            .store
            .get(revision_id)
            .cloned()
            .ok_or_else(|| CatalogError::UnknownRevision {
                revision_id: revision_id.to_string(),
            })?;
        if !revision.status.can_transition_to(next) {
            return Err(CatalogError::IllegalStatusTransition {
                revision_id: revision_id.to_string(),
                from: revision.status,
                to: next,
            });
        }
        self.store.set_status(revision_id, next)
    }

    /// All revisions of a domain, in id order (the in-memory store is a `BTreeMap`). A durable
    /// store should preserve insertion order; callers must not assume more than a stable order.
    pub fn revisions_in(&self, domain: HarnessDomain) -> Vec<&HarnessRevision> {
        let mut revisions: Vec<&HarnessRevision> = self
            .store
            .all()
            .into_iter()
            .filter(|revision| revision.domain == domain)
            .collect();
        revisions.sort_by(|left, right| left.id.cmp(&right.id));
        revisions
    }

    pub fn active_revision(&self, domain: HarnessDomain) -> Option<&HarnessRevision> {
        self.active
            .get(&domain)
            .and_then(|pointer| self.store.get(&pointer.revision_id))
    }

    pub fn active_revision_id(&self, domain: HarnessDomain) -> Option<&str> {
        self.active
            .get(&domain)
            .map(|pointer| pointer.revision_id.as_str())
    }

    pub fn pointer(&self, domain: HarnessDomain) -> Option<&DomainPointer> {
        self.active.get(&domain)
    }

    /// The revision a rollback would restore, if any.
    pub fn prior_revision(&self, domain: HarnessDomain) -> Option<&HarnessRevision> {
        let pointer = self.active.get(&domain)?;
        let prior_id = pointer.prior.last()?;
        self.store.get(prior_id)
    }

    pub fn activations(&self) -> &[Activation] {
        &self.activations
    }

    /// Make `revision_id` the active harness for `domain`.
    ///
    /// The whole method is validate-then-mutate: every refusal (unknown revision, wrong domain,
    /// rejected/failed revision, already-active revision, illegal status transition) happens
    /// before the pointer, the audit log or any status is touched. A rejected revision can never
    /// be activated; `Adoptable`, `Adopted` (bootstrap/import) and `Superseded` (re-adoption after
    /// a rollback) are the only admissible states.
    pub fn activate(
        &mut self,
        domain: HarnessDomain,
        revision_id: &str,
        decision_id: impl Into<String>,
        activated_by: impl Into<String>,
    ) -> Result<Activation, CatalogError> {
        let revision = self
            .store
            .get(revision_id)
            .cloned()
            .ok_or_else(|| CatalogError::UnknownRevision {
                revision_id: revision_id.to_string(),
            })?;
        if revision.domain != domain {
            return Err(CatalogError::DomainMismatch {
                revision_id: revision_id.to_string(),
                expected: domain,
                actual: revision.domain,
            });
        }
        if matches!(
            revision.status,
            RevisionStatus::Rejected | RevisionStatus::Failed | RevisionStatus::RolledBack
        ) {
            return Err(CatalogError::RevisionNotActivable {
                revision_id: revision_id.to_string(),
                status: revision.status,
            });
        }
        if !matches!(
            revision.status,
            RevisionStatus::Adoptable | RevisionStatus::Adopted | RevisionStatus::Superseded
        ) {
            return Err(CatalogError::RevisionNotActivable {
                revision_id: revision_id.to_string(),
                status: revision.status,
            });
        }

        let current = self.active.get(&domain).cloned();
        if current
            .as_ref()
            .is_some_and(|pointer| pointer.revision_id == revision_id)
        {
            return Err(CatalogError::AlreadyActive {
                domain,
                revision_id: revision_id.to_string(),
            });
        }

        // Validate the two status moves before writing either one.
        let current_status = current.as_ref().and_then(|pointer| {
            self.store
                .get(&pointer.revision_id)
                .map(|revision| revision.status)
        });
        if let Some(status) = current_status {
            if !status.can_transition_to(RevisionStatus::Superseded) {
                return Err(CatalogError::IllegalStatusTransition {
                    revision_id: current
                        .as_ref()
                        .map(|pointer| pointer.revision_id.clone())
                        .unwrap_or_default(),
                    from: status,
                    to: RevisionStatus::Superseded,
                });
            }
        }
        // A revision already `Adopted` is a bootstrap/import row: it is admissible as the
        // pointer target without a status transition (Adopted->Adopted is not in the lifecycle
        // table). Every other admissible state must legally advance to `Adopted`.
        if revision.status != RevisionStatus::Adopted
            && !revision.status.can_transition_to(RevisionStatus::Adopted)
        {
            return Err(CatalogError::IllegalStatusTransition {
                revision_id: revision_id.to_string(),
                from: revision.status,
                to: RevisionStatus::Adopted,
            });
        }

        // --- infallible from here: pointer, statuses and audit move together. ---
        if let Some(pointer) = current.clone() {
            self.store
                .set_status(&pointer.revision_id, RevisionStatus::Superseded)?;
        }
        self.store
            .set_status(revision_id, RevisionStatus::Adopted)?;

        self.sequence += 1;
        let activation = Activation {
            domain,
            sequence: self.sequence,
            kind: ActivationKind::Adopt,
            from_revision: current
                .as_ref()
                .map(|pointer| pointer.revision_id.clone()),
            to_revision: revision_id.to_string(),
            decision_id: decision_id.into(),
            activated_by: activated_by.into(),
        };
        let mut displaced = current
            .as_ref()
            .map(|pointer| pointer.prior.clone())
            .unwrap_or_default();
        if let Some(pointer) = &current {
            displaced.push(pointer.revision_id.clone());
        }
        self.active.insert(
            domain,
            DomainPointer {
                revision_id: revision_id.to_string(),
                prior: displaced,
            },
        );
        self.activations.push(activation.clone());
        Ok(activation)
    }

    /// Roll back to the exact revision that was active before the last activation.
    ///
    /// The prior revision is read from the pointer stack and re-activated by id: its manifest,
    /// hash and parent are the ones that were stored when it was active, so the rollback restores
    /// a revision, it does not synthesize one. The displaced revision becomes `RolledBack`.
    pub fn rollback(
        &mut self,
        domain: HarnessDomain,
        decision_id: impl Into<String>,
        activated_by: impl Into<String>,
    ) -> Result<Activation, CatalogError> {
        let pointer = self
            .active
            .get(&domain)
            .cloned()
            .ok_or(CatalogError::NoActiveRevision { domain })?;
        let prior_id = pointer
            .prior
            .last()
            .cloned()
            .ok_or(CatalogError::NoPriorRevision { domain })?;
        let prior = self
            .store
            .get(&prior_id)
            .cloned()
            .ok_or_else(|| CatalogError::UnknownRevision {
                revision_id: prior_id.clone(),
            })?;
        if prior.domain != domain {
            return Err(CatalogError::DomainMismatch {
                revision_id: prior_id.clone(),
                expected: domain,
                actual: prior.domain,
            });
        }
        // The prior revision was either displaced by an adoption (`Superseded`) or is a
        // bootstrap/import revision that was active (`Adopted`). Nothing else may be restored.
        if !matches!(
            prior.status,
            RevisionStatus::Superseded | RevisionStatus::Adopted
        ) {
            return Err(CatalogError::RevisionNotActivable {
                revision_id: prior_id,
                status: prior.status,
            });
        }

        let displaced = self
            .store
            .get(&pointer.revision_id)
            .cloned()
            .ok_or_else(|| CatalogError::UnknownRevision {
                revision_id: pointer.revision_id.clone(),
            })?;
        if !displaced.status.can_transition_to(RevisionStatus::RolledBack) {
            return Err(CatalogError::IllegalStatusTransition {
                revision_id: pointer.revision_id.clone(),
                from: displaced.status,
                to: RevisionStatus::RolledBack,
            });
        }

        // --- infallible from here. ---
        self.store
            .set_status(&pointer.revision_id, RevisionStatus::RolledBack)?;
        self.store
            .set_status(&prior_id, RevisionStatus::Adopted)?;
        self.sequence += 1;
        let activation = Activation {
            domain,
            sequence: self.sequence,
            kind: ActivationKind::Rollback,
            from_revision: Some(pointer.revision_id.clone()),
            to_revision: prior_id.clone(),
            decision_id: decision_id.into(),
            activated_by: activated_by.into(),
        };
        let mut remaining = pointer.prior;
        remaining.pop();
        self.active.insert(
            domain,
            DomainPointer {
                revision_id: prior_id,
                prior: remaining,
            },
        );
        self.activations.push(activation.clone());
        Ok(activation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::harness_evolution::model::{
        Component, ComponentKind, HarnessManifest, HarnessRevision,
    };

    fn prompt(content: &str) -> Component {
        Component::green(ComponentKind::SystemPrompt, content, "catalog test prompt")
    }

    fn revision(
        id: &str,
        parent: Option<&str>,
        domain: HarnessDomain,
        content: &str,
        status: RevisionStatus,
    ) -> HarnessRevision {
        let mut revision = HarnessRevision::draft(
            id,
            domain,
            HarnessManifest::from_components([prompt(content)]),
            "owner",
        );
        revision.parent_revision_id = parent.map(str::to_string);
        revision.with_status(status)
    }

    fn base() -> HarnessRevision {
        revision(
            "rev-1",
            None,
            HarnessDomain::RustBackend,
            "base prompt",
            RevisionStatus::Adopted,
        )
    }

    fn next(id: &str) -> HarnessRevision {
        revision(
            id,
            Some("rev-1"),
            HarnessDomain::RustBackend,
            &format!("prompt {id}"),
            RevisionStatus::Adoptable,
        )
    }

    #[test]
    fn insert_records_parent_and_keeps_manifest_hash_stable() {
        let mut catalog = HarnessCatalog::new();
        catalog.insert(base()).unwrap();
        let child = next("rev-2");
        let hash = child.manifest_hash.clone();
        catalog.insert(child).unwrap();

        let stored = catalog.revision("rev-2").expect("child stored");
        assert_eq!(stored.parent_revision_id.as_deref(), Some("rev-1"));
        assert_eq!(stored.manifest_hash, hash);
        assert_eq!(stored.manifest.manifest_hash(), hash);
        stored.verify_integrity().unwrap();
    }

    #[test]
    fn duplicate_and_tampered_revisions_are_refused_whole() {
        let mut catalog = HarnessCatalog::new();
        catalog.insert(base()).unwrap();
        assert!(matches!(
            catalog.insert(base()),
            Err(CatalogError::DuplicateRevision { .. })
        ));

        let mut tampered = next("rev-2");
        tampered.manifest = HarnessManifest::from_components([prompt("different")]);
        assert!(matches!(
            catalog.insert(tampered),
            Err(CatalogError::Integrity(_))
        ));
        assert_eq!(catalog.len(), 1);
    }

    #[test]
    fn a_missing_or_cross_domain_parent_is_refused() {
        let mut catalog = HarnessCatalog::new();
        catalog.insert(base()).unwrap();
        let orphan = revision(
            "rev-2",
            Some("nope"),
            HarnessDomain::RustBackend,
            "child",
            RevisionStatus::Draft,
        );
        assert!(matches!(
            catalog.insert(orphan),
            Err(CatalogError::ParentMissing { .. })
        ));

        let mut cross = revision(
            "rev-3",
            None,
            HarnessDomain::YewMvi,
            "other domain",
            RevisionStatus::Draft,
        );
        cross.parent_revision_id = Some("rev-1".to_string());
        assert!(matches!(
            catalog.insert(cross),
            Err(CatalogError::ParentDomainMismatch { .. })
        ));
    }

    #[test]
    fn activation_swaps_the_pointer_and_records_history() {
        let mut catalog = HarnessCatalog::new();
        catalog.insert(base()).unwrap();
        catalog.insert(next("rev-2")).unwrap();

        let first = catalog
            .activate(HarnessDomain::RustBackend, "rev-1", "bootstrap", "owner")
            .unwrap();
        assert_eq!(first.from_revision, None);
        assert_eq!(first.to_revision, "rev-1");
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("rev-1")
        );
        assert_eq!(
            catalog.revision("rev-1").unwrap().status,
            RevisionStatus::Adopted
        );

        let second = catalog
            .activate(HarnessDomain::RustBackend, "rev-2", "adopt-1", "policy")
            .unwrap();
        assert_eq!(second.from_revision.as_deref(), Some("rev-1"));
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("rev-2")
        );
        assert_eq!(
            catalog.revision("rev-1").unwrap().status,
            RevisionStatus::Superseded
        );
        assert_eq!(
            catalog.revision("rev-2").unwrap().status,
            RevisionStatus::Adopted
        );
        assert_eq!(catalog.activations().len(), 2);
    }

    #[test]
    fn a_rejected_revision_cannot_activate() {
        let mut catalog = HarnessCatalog::new();
        catalog.insert(base()).unwrap();
        let mut rejected = next("rev-2");
        rejected = rejected.with_status(RevisionStatus::Rejected);
        catalog.insert(rejected).unwrap();
        let error = catalog
            .activate(HarnessDomain::RustBackend, "rev-2", "adopt-1", "policy")
            .unwrap_err();
        assert!(matches!(
            error,
            CatalogError::RevisionNotActivable {
                status: RevisionStatus::Rejected,
                ..
            }
        ));
        assert_eq!(catalog.active_revision_id(HarnessDomain::RustBackend), None);
    }

    #[test]
    fn a_refused_activation_leaves_the_pointer_untouched() {
        let mut catalog = HarnessCatalog::new();
        catalog.insert(base()).unwrap();
        catalog
            .activate(HarnessDomain::RustBackend, "rev-1", "bootstrap", "owner")
            .unwrap();
        let before = catalog.pointer(HarnessDomain::RustBackend).cloned();
        let activations_before = catalog.activations().len();

        assert!(matches!(
            catalog.activate(
                HarnessDomain::RustBackend,
                "missing",
                "adopt-x",
                "policy"
            ),
            Err(CatalogError::UnknownRevision { .. })
        ));
        assert_eq!(catalog.activate(
            HarnessDomain::RustBackend,
            "rev-1",
            "adopt-x",
            "policy"
        ), Err(CatalogError::AlreadyActive {
            domain: HarnessDomain::RustBackend,
            revision_id: "rev-1".to_string(),
        }));
        assert_eq!(before, catalog.pointer(HarnessDomain::RustBackend).cloned());
        assert_eq!(activations_before, catalog.activations().len());
    }

    #[test]
    fn rollback_restores_the_exact_prior_revision() {
        let mut catalog = HarnessCatalog::new();
        catalog.insert(base()).unwrap();
        catalog.insert(next("rev-2")).unwrap();
        catalog.insert(next("rev-3")).unwrap();
        catalog
            .activate(HarnessDomain::RustBackend, "rev-1", "bootstrap", "owner")
            .unwrap();
        catalog
            .activate(HarnessDomain::RustBackend, "rev-2", "adopt-2", "policy")
            .unwrap();
        catalog
            .activate(HarnessDomain::RustBackend, "rev-3", "adopt-3", "policy")
            .unwrap();

        let exact = catalog.revision("rev-2").unwrap().clone();
        let rollback = catalog
            .rollback(HarnessDomain::RustBackend, "rollback-1", "owner")
            .unwrap();
        assert_eq!(rollback.kind, ActivationKind::Rollback);
        assert_eq!(rollback.from_revision.as_deref(), Some("rev-3"));
        assert_eq!(rollback.to_revision, "rev-2");

        let restored = catalog
            .active_revision(HarnessDomain::RustBackend)
            .expect("active after rollback");
        assert_eq!(restored.id, exact.id);
        assert_eq!(restored.manifest_hash, exact.manifest_hash);
        assert_eq!(restored.manifest, exact.manifest);
        assert_eq!(restored.parent_revision_id, exact.parent_revision_id);
        assert_eq!(restored.status, RevisionStatus::Adopted);
        assert_eq!(
            catalog.revision("rev-3").unwrap().status,
            RevisionStatus::RolledBack
        );

        // A second rollback walks one further back — the stack is real.
        catalog
            .rollback(HarnessDomain::RustBackend, "rollback-2", "owner")
            .unwrap();
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("rev-1")
        );
        assert_eq!(
            catalog.revision("rev-2").unwrap().status,
            RevisionStatus::RolledBack
        );
    }

    #[test]
    fn rollback_without_history_is_refused() {
        let mut catalog = HarnessCatalog::new();
        catalog.insert(base()).unwrap();
        assert_eq!(
            catalog.rollback(HarnessDomain::RustBackend, "rollback-0", "owner"),
            Err(CatalogError::NoActiveRevision {
                domain: HarnessDomain::RustBackend
            })
        );
        catalog
            .activate(HarnessDomain::RustBackend, "rev-1", "bootstrap", "owner")
            .unwrap();
        assert_eq!(
            catalog.rollback(HarnessDomain::RustBackend, "rollback-1", "owner"),
            Err(CatalogError::NoPriorRevision {
                domain: HarnessDomain::RustBackend
            })
        );
    }

    #[test]
    fn a_rolled_back_revision_cannot_be_reactivated_directly() {
        let mut catalog = HarnessCatalog::new();
        catalog.insert(base()).unwrap();
        catalog.insert(next("rev-2")).unwrap();
        catalog
            .activate(HarnessDomain::RustBackend, "rev-1", "bootstrap", "owner")
            .unwrap();
        catalog
            .activate(HarnessDomain::RustBackend, "rev-2", "adopt-2", "policy")
            .unwrap();
        catalog
            .rollback(HarnessDomain::RustBackend, "rollback-1", "owner")
            .unwrap();
        let error = catalog
            .activate(HarnessDomain::RustBackend, "rev-2", "adopt-3", "policy")
            .unwrap_err();
        assert!(matches!(
            error,
            CatalogError::RevisionNotActivable {
                status: RevisionStatus::RolledBack,
                ..
            }
        ));
    }

    #[test]
    fn each_domain_has_its_own_pointer() {
        let mut catalog = HarnessCatalog::new();
        catalog.insert(base()).unwrap();
        let yew = revision(
            "yew-1",
            None,
            HarnessDomain::YewMvi,
            "yew prompt",
            RevisionStatus::Adopted,
        );
        catalog.insert(yew).unwrap();
        catalog
            .activate(HarnessDomain::RustBackend, "rev-1", "bootstrap", "owner")
            .unwrap();
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::YewMvi),
            None,
            "activating one domain must not activate another"
        );
        catalog
            .activate(HarnessDomain::YewMvi, "yew-1", "bootstrap", "owner")
            .unwrap();
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("rev-1")
        );
    }

    #[test]
    fn revisions_in_filters_by_domain() {
        let mut catalog = HarnessCatalog::new();
        catalog.insert(base()).unwrap();
        catalog.insert(next("rev-2")).unwrap();
        catalog
            .insert(revision(
                "yew-1",
                None,
                HarnessDomain::YewMvi,
                "yew",
                RevisionStatus::Draft,
            ))
            .unwrap();
        assert_eq!(catalog.revisions_in(HarnessDomain::RustBackend).len(), 2);
        assert_eq!(catalog.revisions_in(HarnessDomain::YewMvi).len(), 1);
    }
}
