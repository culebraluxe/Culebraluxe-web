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
    fn set_status(&mut self, revision_id: &str, status: RevisionStatus)
        -> Result<(), CatalogError>;

    /// Persist one pointer move **atomically**: the activation rows and the lifecycle status
    /// writes the plan carries must commit together or not at all.
    ///
    /// This is the D4 seam. The catalog validates first, builds the plan, and calls this method
    /// before touching the in-memory pointer: a store that returns `Err` (a refused transaction,
    /// a stale pointer, a lost connection) leaves the active pointer and every revision status
    /// exactly as they were. In-memory v1 validates all writes before applying any; the durable
    /// store runs one PostgreSQL transaction.
    fn apply_activation(&mut self, plan: &ActivationPlan) -> Result<(), CatalogError>;

    /// The revision recorded as active in the durable store, if the store keeps one. The
    /// in-memory store starts empty; [`HarnessCatalog::restore_active_pointer`] uses this to
    /// rehydrate a catalog from a durable store.
    fn current_activation(&self, _domain: HarnessDomain) -> Result<Option<String>, CatalogError> {
        Ok(None)
    }

    /// The highest activation sequence the store already holds for a domain. A rehydrated catalog
    /// resumes from here so the next activation keeps advancing; the in-memory store starts at 0.
    fn last_activation_sequence(&self, _domain: HarnessDomain) -> Result<u64, CatalogError> {
        Ok(0)
    }

    /// The domain's activation rows, OLDEST FIRST, for rebuilding the pointer stack after a restart.
    ///
    /// [`HarnessCatalog::restore_active_pointer`] replays this history to recover which revisions
    /// were displaced, so `rollback` restores the exact prior revision even in a fresh process. A
    /// store with no durable history returns the empty default.
    fn activation_history(&self, _domain: HarnessDomain) -> Result<Vec<Activation>, CatalogError> {
        Ok(Vec::new())
    }
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
        let revision =
            self.revisions
                .get_mut(revision_id)
                .ok_or_else(|| CatalogError::UnknownRevision {
                    revision_id: revision_id.to_string(),
                })?;
        revision.status = status;
        Ok(())
    }

    /// Validate every write first, then apply all of them. The in-memory analogue of the database
    /// transaction: a plan that names an unknown revision or an illegal transition writes nothing.
    fn apply_activation(&mut self, plan: &ActivationPlan) -> Result<(), CatalogError> {
        for (revision_id, next) in &plan.status_writes {
            let revision =
                self.revisions
                    .get(revision_id)
                    .ok_or_else(|| CatalogError::UnknownRevision {
                        revision_id: revision_id.clone(),
                    })?;
            if revision.status != *next && !revision.status.can_transition_to(*next) {
                return Err(CatalogError::IllegalStatusTransition {
                    revision_id: revision_id.clone(),
                    from: revision.status,
                    to: *next,
                });
            }
        }
        for (revision_id, next) in &plan.status_writes {
            if let Some(revision) = self.revisions.get_mut(revision_id) {
                revision.status = *next;
            }
        }
        Ok(())
    }

    fn current_activation(&self, _domain: HarnessDomain) -> Result<Option<String>, CatalogError> {
        Ok(None)
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
    /// The revision store refused the write (durable transaction failure, stale pointer, ...).
    /// The catalog has not moved anything when this is returned.
    Store(String),
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
            } => write!(f, "revision {revision_id} may not move from {from} to {to}"),
            Self::Store(message) => write!(f, "revision store refused the write: {message}"),
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

/// One transactional pointer move handed to a [`RevisionStore`].
///
/// It carries everything the durable store needs to commit the activation and its two lifecycle
/// writes together: the audit row (domain, sequence, decision, actor) and the status writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationPlan {
    pub domain: HarnessDomain,
    pub sequence: u64,
    pub kind: ActivationKind,
    pub from_revision: Option<String>,
    pub to_revision: String,
    pub decision_id: String,
    pub activated_by: String,
    /// Lifecycle writes applied in the same transaction as the pointer move:
    /// `(revision_id, next_status)`.
    pub status_writes: Vec<(String, RevisionStatus)>,
}

impl ActivationPlan {
    pub fn activation(&self) -> Activation {
        Activation {
            domain: self.domain,
            sequence: self.sequence,
            kind: self.kind,
            from_revision: self.from_revision.clone(),
            to_revision: self.to_revision.clone(),
            decision_id: self.decision_id.clone(),
            activated_by: self.activated_by.clone(),
        }
    }
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
    /// One monotonic sequence per domain: activating another domain does not advance (or rewind)
    /// this domain's sequence, and a restart resumes each domain independently.
    sequences: BTreeMap<HarnessDomain, u64>,
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
            sequences: BTreeMap::new(),
        }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    pub fn into_store(self) -> S {
        self.store
    }

    /// Rehydrate the active pointer for one domain from the durable store. The prior stack starts
    /// empty (the pointer history that matters for rollback is the store's own activation rows);
    /// the in-memory store returns `None` by default, so this is a no-op for the test store.
    pub fn restore_active_pointer(
        &mut self,
        domain: HarnessDomain,
    ) -> Result<Option<String>, CatalogError> {
        let current = self.store.current_activation(domain)?;
        let history = self.store.activation_history(domain)?;
        let mut sequence = self.store.last_activation_sequence(domain).unwrap_or(0);
        // Replay the audit rows to rebuild the displaced stack: an adoption pushes the previous
        // pointer onto the stack, a rollback pops it. Without this replay a restarted process
        // would know the active revision but not the one a rollback must restore.
        let mut prior: Vec<String> = Vec::new();
        let mut active: Option<String> = None;
        for activation in &history {
            sequence = sequence.max(activation.sequence);
            match activation.kind {
                ActivationKind::Adopt => {
                    if let Some(previous) = active.take() {
                        if !prior.contains(&previous) {
                            prior.push(previous);
                        }
                    }
                    active = Some(activation.to_revision.clone());
                }
                ActivationKind::Rollback => {
                    active = prior.pop().or_else(|| Some(activation.to_revision.clone()));
                }
            }
        }
        let Some(revision_id) = current else {
            self.active.remove(&domain);
            self.sequences.insert(domain, sequence);
            return Ok(None);
        };
        let revision =
            self.store
                .get(&revision_id)
                .ok_or_else(|| CatalogError::UnknownRevision {
                    revision_id: revision_id.clone(),
                })?;
        if revision.domain != domain {
            return Err(CatalogError::DomainMismatch {
                revision_id,
                expected: domain,
                actual: revision.domain,
            });
        }
        // Trust the durable current pointer but require the replayed history to agree with it.
        if !history.is_empty() && active.as_deref() != Some(revision_id.as_str()) {
            return Err(CatalogError::Store(format!(
                "activation history for {domain:?} replays to {active:?}, but the durable current pointer is {revision_id}; refusing to restore a divergent catalog"
            )));
        }
        if active.is_none() {
            prior.clear();
        }
        self.sequences.insert(domain, sequence);
        self.active.insert(
            domain,
            DomainPointer {
                revision_id: revision_id.clone(),
                prior,
            },
        );
        Ok(Some(revision_id))
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
        let revision =
            self.store
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
    pub(crate) fn activate(
        &mut self,
        domain: HarnessDomain,
        revision_id: &str,
        decision_id: impl Into<String>,
        activated_by: impl Into<String>,
    ) -> Result<Activation, CatalogError> {
        let revision =
            self.store
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

        // D4: persist the pointer move and its lifecycle writes FIRST, as one transaction. If the
        // store refuses (a failed commit, a stale pointer), nothing below runs and the in-memory
        // pointer, history and statuses stay exactly as they were.
        let sequence = self.sequences.get(&domain).copied().unwrap_or(0) + 1;
        let from_revision = current.as_ref().map(|pointer| pointer.revision_id.clone());
        let mut status_writes: Vec<(String, RevisionStatus)> = Vec::new();
        if let (Some(status), Some(pointer)) = (current_status, current.as_ref()) {
            if status != RevisionStatus::Superseded {
                status_writes.push((pointer.revision_id.clone(), RevisionStatus::Superseded));
            }
        }
        if revision.status != RevisionStatus::Adopted {
            status_writes.push((revision_id.to_string(), RevisionStatus::Adopted));
        }
        let plan = ActivationPlan {
            domain,
            sequence,
            kind: ActivationKind::Adopt,
            from_revision,
            to_revision: revision_id.to_string(),
            decision_id: decision_id.into(),
            activated_by: activated_by.into(),
            status_writes,
        };
        self.store.apply_activation(&plan)?;
        let activation = plan.activation();

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
        self.sequences.insert(domain, sequence);
        self.activations.push(activation.clone());
        Ok(activation)
    }

    /// Roll back to the exact revision that was active before the last activation.
    ///
    /// The prior revision is read from the pointer stack and re-activated by id: its manifest,
    /// hash and parent are the ones that were stored when it was active, so the rollback restores
    /// a revision, it does not synthesize one. The displaced revision becomes `RolledBack`.
    pub(crate) fn rollback(
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
        let prior =
            self.store
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
        if !displaced
            .status
            .can_transition_to(RevisionStatus::RolledBack)
        {
            return Err(CatalogError::IllegalStatusTransition {
                revision_id: pointer.revision_id.clone(),
                from: displaced.status,
                to: RevisionStatus::RolledBack,
            });
        }

        // D4: one transactional pointer move; a refused write leaves the pointer where it was.
        let sequence = self.sequences.get(&domain).copied().unwrap_or(0) + 1;
        let mut status_writes: Vec<(String, RevisionStatus)> = Vec::new();
        if displaced.status != RevisionStatus::RolledBack {
            status_writes.push((pointer.revision_id.clone(), RevisionStatus::RolledBack));
        }
        if prior.status != RevisionStatus::Adopted {
            status_writes.push((prior_id.clone(), RevisionStatus::Adopted));
        }
        let plan = ActivationPlan {
            domain,
            sequence,
            kind: ActivationKind::Rollback,
            from_revision: Some(pointer.revision_id.clone()),
            to_revision: prior_id.clone(),
            decision_id: decision_id.into(),
            activated_by: activated_by.into(),
            status_writes,
        };
        self.store.apply_activation(&plan)?;
        let activation = plan.activation();

        let mut remaining = pointer.prior;
        remaining.pop();
        self.active.insert(
            domain,
            DomainPointer {
                revision_id: prior_id,
                prior: remaining,
            },
        );
        self.sequences.insert(domain, sequence);
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
            catalog.activate(HarnessDomain::RustBackend, "missing", "adopt-x", "policy"),
            Err(CatalogError::UnknownRevision { .. })
        ));
        assert_eq!(
            catalog.activate(HarnessDomain::RustBackend, "rev-1", "adopt-x", "policy"),
            Err(CatalogError::AlreadyActive {
                domain: HarnessDomain::RustBackend,
                revision_id: "rev-1".to_string(),
            })
        );
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

    /// A store double that records the activation plans it was asked to persist and can be told
    /// to fail the next write. This is how the D4 contract is asserted without a database.
    #[derive(Default)]
    struct FlakyStore {
        inner: InMemoryRevisionStore,
        plans: Vec<ActivationPlan>,
        fail_next: bool,
    }

    impl RevisionStore for FlakyStore {
        fn insert(&mut self, revision: HarnessRevision) -> Result<(), CatalogError> {
            self.inner.insert(revision)
        }

        fn get(&self, revision_id: &str) -> Option<&HarnessRevision> {
            self.inner.get(revision_id)
        }

        fn all(&self) -> Vec<&HarnessRevision> {
            self.inner.all()
        }

        fn len(&self) -> usize {
            self.inner.len()
        }

        fn set_status(
            &mut self,
            revision_id: &str,
            status: RevisionStatus,
        ) -> Result<(), CatalogError> {
            self.inner.set_status(revision_id, status)
        }

        fn apply_activation(&mut self, plan: &ActivationPlan) -> Result<(), CatalogError> {
            self.plans.push(plan.clone());
            if self.fail_next {
                return Err(CatalogError::Store(
                    "simulated transactional write failure".into(),
                ));
            }
            self.inner.apply_activation(plan)
        }
    }

    fn catalog_with_store(store: FlakyStore) -> HarnessCatalog<FlakyStore> {
        HarnessCatalog::with_store(store)
    }

    #[test]
    fn activation_is_persisted_through_the_store_seam() {
        let mut catalog = catalog_with_store(FlakyStore::default());
        catalog.insert(base()).unwrap();
        catalog.insert(next("rev-2")).unwrap();
        catalog
            .activate(HarnessDomain::RustBackend, "rev-1", "bootstrap", "owner")
            .unwrap();
        catalog
            .activate(HarnessDomain::RustBackend, "rev-2", "adopt-1", "policy")
            .unwrap();
        let plans = &catalog.store().plans;
        assert_eq!(plans.len(), 2, "both pointer moves go through the store");
        assert_eq!(plans[0].kind, ActivationKind::Adopt);
        assert_eq!(plans[0].to_revision, "rev-1");
        assert!(
            plans[0].status_writes.is_empty(),
            "a bootstrap revision already Adopted needs no lifecycle write: {:?}",
            plans[0].status_writes
        );
        assert_eq!(
            plans[1].status_writes,
            vec![
                ("rev-1".to_string(), RevisionStatus::Superseded),
                ("rev-2".to_string(), RevisionStatus::Adopted),
            ]
        );
        // The catalog's own view agrees with what it asked the store to write.
        assert_eq!(
            catalog.revision("rev-1").unwrap().status,
            RevisionStatus::Superseded
        );
        assert_eq!(
            catalog.revision("rev-2").unwrap().status,
            RevisionStatus::Adopted
        );
    }

    #[test]
    fn rollback_is_persisted_through_the_store_seam() {
        let mut catalog = catalog_with_store(FlakyStore::default());
        catalog.insert(base()).unwrap();
        catalog.insert(next("rev-2")).unwrap();
        catalog
            .activate(HarnessDomain::RustBackend, "rev-1", "bootstrap", "owner")
            .unwrap();
        catalog
            .activate(HarnessDomain::RustBackend, "rev-2", "adopt-1", "policy")
            .unwrap();
        catalog
            .rollback(HarnessDomain::RustBackend, "rollback-1", "owner")
            .unwrap();
        let plans = &catalog.store().plans;
        assert_eq!(plans.len(), 3);
        assert_eq!(plans[2].kind, ActivationKind::Rollback);
        assert_eq!(plans[2].to_revision, "rev-1");
        assert_eq!(
            plans[2].status_writes,
            vec![
                ("rev-2".to_string(), RevisionStatus::RolledBack),
                ("rev-1".to_string(), RevisionStatus::Adopted),
            ]
        );
    }

    #[test]
    fn a_failed_store_write_leaves_the_pointer_and_statuses_unchanged() {
        let mut catalog = catalog_with_store(FlakyStore::default());
        catalog.insert(base()).unwrap();
        catalog.insert(next("rev-2")).unwrap();
        catalog
            .activate(HarnessDomain::RustBackend, "rev-1", "bootstrap", "owner")
            .unwrap();
        let pointer_before = catalog.pointer(HarnessDomain::RustBackend).cloned();
        let activations_before = catalog.activations().len();

        catalog.store_mut().fail_next = true;
        let error = catalog
            .activate(HarnessDomain::RustBackend, "rev-2", "adopt-1", "policy")
            .expect_err("the store refuses the transaction");
        assert!(matches!(error, CatalogError::Store(_)), "{error}");

        // D4: the failed write moved nothing — pointer, history and both statuses are untouched.
        assert_eq!(
            pointer_before,
            catalog.pointer(HarnessDomain::RustBackend).cloned()
        );
        assert_eq!(activations_before, catalog.activations().len());
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("rev-1")
        );
        assert_eq!(
            catalog.revision("rev-1").unwrap().status,
            RevisionStatus::Adopted
        );
        assert_eq!(
            catalog.revision("rev-2").unwrap().status,
            RevisionStatus::Adoptable
        );
    }

    #[test]
    fn a_failed_store_write_does_not_advance_the_activation_sequence() {
        let mut catalog = catalog_with_store(FlakyStore::default());
        catalog.insert(base()).unwrap();
        catalog.insert(next("rev-2")).unwrap();
        catalog
            .activate(HarnessDomain::RustBackend, "rev-1", "bootstrap", "owner")
            .unwrap();
        catalog.store_mut().fail_next = true;
        catalog
            .activate(HarnessDomain::RustBackend, "rev-2", "adopt-1", "policy")
            .expect_err("refused");
        catalog.store_mut().fail_next = false;
        let activation = catalog
            .activate(HarnessDomain::RustBackend, "rev-2", "adopt-2", "policy")
            .expect("retry succeeds");
        assert_eq!(activation.sequence, 2);
        assert_eq!(catalog.activations().len(), 2);
    }

    #[test]
    fn the_in_memory_transactional_write_is_all_or_nothing() {
        let mut store = InMemoryRevisionStore::new();
        store
            .insert(revision(
                "rev-1",
                None,
                HarnessDomain::RustBackend,
                "base",
                RevisionStatus::Draft,
            ))
            .unwrap();
        let plan = ActivationPlan {
            domain: HarnessDomain::RustBackend,
            sequence: 1,
            kind: ActivationKind::Adopt,
            from_revision: None,
            to_revision: "rev-1".to_string(),
            decision_id: "adopt".to_string(),
            activated_by: "policy".to_string(),
            status_writes: vec![
                ("rev-1".to_string(), RevisionStatus::Proposed),
                ("missing-revision".to_string(), RevisionStatus::Adopted),
            ],
        };
        let error = store.apply_activation(&plan).expect_err("plan is invalid");
        assert!(
            matches!(error, CatalogError::UnknownRevision { .. }),
            "{error}"
        );
        assert_eq!(
            store.get("rev-1").unwrap().status,
            RevisionStatus::Draft,
            "the first (legal) write must not survive a failed transaction"
        );
    }

    #[test]
    fn restoring_the_active_pointer_from_the_store_uses_the_durable_pointer() {
        #[derive(Default)]
        struct PointerStore {
            inner: InMemoryRevisionStore,
            current: Option<String>,
        }

        impl RevisionStore for PointerStore {
            fn insert(&mut self, revision: HarnessRevision) -> Result<(), CatalogError> {
                self.inner.insert(revision)
            }

            fn get(&self, revision_id: &str) -> Option<&HarnessRevision> {
                self.inner.get(revision_id)
            }

            fn all(&self) -> Vec<&HarnessRevision> {
                self.inner.all()
            }

            fn len(&self) -> usize {
                self.inner.len()
            }

            fn set_status(
                &mut self,
                revision_id: &str,
                status: RevisionStatus,
            ) -> Result<(), CatalogError> {
                self.inner.set_status(revision_id, status)
            }

            fn apply_activation(&mut self, plan: &ActivationPlan) -> Result<(), CatalogError> {
                self.inner.apply_activation(plan)
            }

            fn current_activation(
                &self,
                _domain: HarnessDomain,
            ) -> Result<Option<String>, CatalogError> {
                Ok(self.current.clone())
            }
        }

        let mut catalog = HarnessCatalog::with_store(PointerStore {
            inner: InMemoryRevisionStore::new(),
            current: Some("rev-1".to_string()),
        });
        catalog.insert(base()).unwrap();
        let restored = catalog
            .restore_active_pointer(HarnessDomain::RustBackend)
            .unwrap();
        assert_eq!(restored.as_deref(), Some("rev-1"));
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

    /// A durable-ish fake: same in-memory revisions plus the activation rows a restart observes.
    #[derive(Default, Clone)]
    struct DurableFakeStore {
        inner: InMemoryRevisionStore,
        current: BTreeMap<HarnessDomain, String>,
        history: BTreeMap<HarnessDomain, Vec<Activation>>,
        sequences: BTreeMap<HarnessDomain, u64>,
    }

    impl RevisionStore for DurableFakeStore {
        fn insert(&mut self, revision: HarnessRevision) -> Result<(), CatalogError> {
            self.inner.insert(revision)
        }
        fn get(&self, revision_id: &str) -> Option<&HarnessRevision> {
            self.inner.get(revision_id)
        }
        fn all(&self) -> Vec<&HarnessRevision> {
            self.inner.all()
        }
        fn len(&self) -> usize {
            self.inner.len()
        }
        fn set_status(
            &mut self,
            revision_id: &str,
            status: RevisionStatus,
        ) -> Result<(), CatalogError> {
            self.inner.set_status(revision_id, status)
        }
        fn apply_activation(&mut self, plan: &ActivationPlan) -> Result<(), CatalogError> {
            for (revision_id, status) in &plan.status_writes {
                self.inner.set_status(revision_id, *status)?;
            }
            self.history
                .entry(plan.domain)
                .or_default()
                .push(plan.activation());
            self.current.insert(plan.domain, plan.to_revision.clone());
            self.sequences.insert(plan.domain, plan.sequence);
            Ok(())
        }
        fn current_activation(
            &self,
            domain: HarnessDomain,
        ) -> Result<Option<String>, CatalogError> {
            Ok(self.current.get(&domain).cloned())
        }
        fn last_activation_sequence(&self, domain: HarnessDomain) -> Result<u64, CatalogError> {
            Ok(self.sequences.get(&domain).copied().unwrap_or(0))
        }
        fn activation_history(
            &self,
            domain: HarnessDomain,
        ) -> Result<Vec<Activation>, CatalogError> {
            Ok(self.history.get(&domain).cloned().unwrap_or_default())
        }
    }

    fn rooted_revision(
        id: &str,
        parent: Option<&str>,
        domain: HarnessDomain,
        status: RevisionStatus,
    ) -> HarnessRevision {
        revision(id, parent, domain, &format!("prompt {id}"), status)
    }

    #[test]
    fn a_restarted_catalog_restores_the_rollback_stack_from_activation_history() {
        let mut store = DurableFakeStore::default();
        store.insert(base()).unwrap();
        store.insert(next("rev-2")).unwrap();
        store
            .current
            .insert(HarnessDomain::RustBackend, "rev-1".into());
        store
            .history
            .entry(HarnessDomain::RustBackend)
            .or_default()
            .push(Activation {
                domain: HarnessDomain::RustBackend,
                sequence: 1,
                kind: ActivationKind::Adopt,
                from_revision: None,
                to_revision: "rev-1".into(),
                decision_id: "bootstrap".into(),
                activated_by: "owner".into(),
            });
        store.sequences.insert(HarnessDomain::RustBackend, 1);

        let mut catalog = HarnessCatalog::with_store(store);
        assert_eq!(
            catalog
                .restore_active_pointer(HarnessDomain::RustBackend)
                .unwrap(),
            Some("rev-1".to_string())
        );
        // Move to rev-2 in this process, then simulate a restart with the same durable store.
        catalog
            .activate(HarnessDomain::RustBackend, "rev-2", "adopt-1", "policy")
            .unwrap();
        let durable = catalog.into_store();
        let mut restarted = HarnessCatalog::with_store(durable);
        assert_eq!(
            restarted
                .restore_active_pointer(HarnessDomain::RustBackend)
                .unwrap(),
            Some("rev-2".to_string())
        );
        assert_eq!(
            restarted.active_revision_id(HarnessDomain::RustBackend),
            Some("rev-2")
        );
        assert_eq!(
            restarted
                .prior_revision(HarnessDomain::RustBackend)
                .unwrap()
                .id,
            "rev-1"
        );
        // Rollback works after the restart: the exact prior revision, not a synthesized one.
        let activation = restarted
            .rollback(HarnessDomain::RustBackend, "rollback-1", "owner")
            .expect("rollback after restart");
        assert_eq!(activation.to_revision, "rev-1");
        assert_eq!(
            activation.sequence, 3,
            "the restored sequence keeps advancing"
        );
        assert_eq!(activation.from_revision.as_deref(), Some("rev-2"));
    }

    #[test]
    fn restores_each_domain_with_its_own_sequence_without_a_global_rewind() {
        let rust_rev1 = rooted_revision(
            "rust-1",
            None,
            HarnessDomain::RustBackend,
            RevisionStatus::Adopted,
        );
        let rust_rev2 = rooted_revision(
            "rust-2",
            Some("rust-1"),
            HarnessDomain::RustBackend,
            RevisionStatus::Adopted,
        );
        let yew_rev1 = rooted_revision(
            "yew-1",
            None,
            HarnessDomain::YewMvi,
            RevisionStatus::Adopted,
        );
        let mut store = DurableFakeStore::default();
        for revision in [rust_rev1, rust_rev2, yew_rev1] {
            store.insert(revision).unwrap();
        }
        store
            .current
            .insert(HarnessDomain::RustBackend, "rust-2".into());
        store
            .history
            .entry(HarnessDomain::RustBackend)
            .or_default()
            .extend([
                Activation {
                    domain: HarnessDomain::RustBackend,
                    sequence: 1,
                    kind: ActivationKind::Adopt,
                    from_revision: None,
                    to_revision: "rust-1".into(),
                    decision_id: "bootstrap".into(),
                    activated_by: "owner".into(),
                },
                Activation {
                    domain: HarnessDomain::RustBackend,
                    sequence: 2,
                    kind: ActivationKind::Adopt,
                    from_revision: Some("rust-1".into()),
                    to_revision: "rust-2".into(),
                    decision_id: "adopt-1".into(),
                    activated_by: "policy".into(),
                },
            ]);
        store.sequences.insert(HarnessDomain::RustBackend, 2);
        store.current.insert(HarnessDomain::YewMvi, "yew-1".into());
        store
            .history
            .entry(HarnessDomain::YewMvi)
            .or_default()
            .push(Activation {
                domain: HarnessDomain::YewMvi,
                sequence: 1,
                kind: ActivationKind::Adopt,
                from_revision: None,
                to_revision: "yew-1".into(),
                decision_id: "bootstrap".into(),
                activated_by: "owner".into(),
            });
        store.sequences.insert(HarnessDomain::YewMvi, 1);

        let mut catalog = HarnessCatalog::with_store(store);
        catalog
            .restore_active_pointer(HarnessDomain::RustBackend)
            .unwrap();
        catalog
            .restore_active_pointer(HarnessDomain::YewMvi)
            .unwrap();
        // rust is at sequence 2, yew at sequence 1: each domain advances independently.
        catalog
            .insert(rooted_revision(
                "yew-2",
                Some("yew-1"),
                HarnessDomain::YewMvi,
                RevisionStatus::Adoptable,
            ))
            .unwrap();
        let yew_activation = catalog
            .activate(HarnessDomain::YewMvi, "yew-2", "adopt-yew", "policy")
            .unwrap();
        assert_eq!(yew_activation.sequence, 2, "yew resumes its own sequence");
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("rust-2"),
            "another domain's activation does not move or rewind this domain"
        );
        // rust's next activation is 3, not 2 (no global sequence rewind).
        catalog
            .insert(rooted_revision(
                "rust-3",
                Some("rust-2"),
                HarnessDomain::RustBackend,
                RevisionStatus::Adoptable,
            ))
            .unwrap();
        let rust_activation = catalog
            .activate(HarnessDomain::RustBackend, "rust-3", "adopt-rust", "policy")
            .unwrap();
        assert_eq!(rust_activation.sequence, 3);
    }
}
