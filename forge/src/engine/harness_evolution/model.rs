//! Harness Evolution — immutable typed contracts (Batch A).
//!
//! A native-Rust model of the Harness² improvement loop (Propose → Probe → Contrast →
//! Compose → Forge Validate → Adopt/Reject → Reflect) for the Forge SDLC. The design notes
//! live in `docs/agent/HARNESS-EVOLUTION-ARCHITECTURE.md`; the safety policy lives in
//! [`super::policy`].
//!
//! The one rule this module exists to make mechanical: **a revision is an immutable,
//! content-addressed value**. There is deliberately no `&mut self` API on [`HarnessManifest`]
//! or on any durable record — a change is a new value with a new hash, and the old revision
//! stays readable. Orchestration (catalog, probe execution, experiments, adoption) lives in
//! the Lead/Smith modules; this file carries the contracts only.
//!
//! No Python sidecar, no second telemetry store: the evidence model carries *references* to
//! durable Forge evidence ids, never a copy of telemetry.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;

/// Domain-separation prefix mixed into every manifest hash. Bumping this invalidates every
/// stored hash on purpose; a silent hash-format change would make two different harnesses
/// look identical.
pub const MANIFEST_HASH_DOMAIN: &str = "forge.harness_evolution.manifest.v1";

/// The eight editable harness surfaces, in the order Harness² names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind {
    SystemPrompt,
    Guardrails,
    Memory,
    ToolGuidance,
    Subagents,
    Skills,
    Scripts,
    Plugins,
}

impl ComponentKind {
    /// Every component kind, in a stable order. Persistence and hashing iterate this order.
    pub const ALL: [Self; 8] = [
        Self::SystemPrompt,
        Self::Guardrails,
        Self::Memory,
        Self::ToolGuidance,
        Self::Subagents,
        Self::Skills,
        Self::Scripts,
        Self::Plugins,
    ];

    /// Stable external id (also the serde representation).
    pub const fn id(self) -> &'static str {
        match self {
            Self::SystemPrompt => "system_prompt",
            Self::Guardrails => "guardrails",
            Self::Memory => "memory",
            Self::ToolGuidance => "tool_guidance",
            Self::Subagents => "subagents",
            Self::Skills => "skills",
            Self::Scripts => "scripts",
            Self::Plugins => "plugins",
        }
    }

    /// The least authority a component of this kind may claim.
    ///
    /// A proposer cannot quietly downgrade a protected surface: the policy takes the maximum
    /// of the declared class and this floor. `Scripts` and `Plugins` are RED by construction —
    /// they are the tool/MCP and publication/DEV_OPS surfaces.
    pub const fn minimum_authority(self) -> AuthorityClass {
        match self {
            Self::SystemPrompt | Self::Memory => AuthorityClass::Green,
            Self::Skills | Self::ToolGuidance | Self::Subagents | Self::Guardrails => {
                AuthorityClass::Yellow
            }
            Self::Scripts | Self::Plugins => AuthorityClass::Red,
        }
    }
}

impl fmt::Display for ComponentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// How much protection a harness surface carries.
///
/// * `Green` — experimentation and automatic adoption are allowed when validation is clean.
/// * `Yellow` — experimentation is allowed; promotion is controlled (Council review).
/// * `Red` — owner-only. The optimizer may never change it, and no automatic path may adopt it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityClass {
    Green,
    Yellow,
    Red,
}

impl AuthorityClass {
    pub const ALL: [Self; 3] = [Self::Green, Self::Yellow, Self::Red];

    pub const fn rank(self) -> u8 {
        match self {
            Self::Green => 0,
            Self::Yellow => 1,
            Self::Red => 2,
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Red => "red",
        }
    }

    /// The more protective of two classes.
    pub const fn max(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    pub const fn is_red(self) -> bool {
        matches!(self, Self::Red)
    }

    pub const fn is_green(self) -> bool {
        matches!(self, Self::Green)
    }
}

impl fmt::Display for AuthorityClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// One component of a harness revision.
///
/// `authority` is the *declared* class. The policy never trusts it alone: the effective class
/// is [`Component::effective_authority`] = max(declared, kind floor), and a change to a RED
/// surface is refused before a probe can run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub kind: ComponentKind,
    pub authority: AuthorityClass,
    /// The actual harness text/config for this surface.
    pub content: String,
    /// What this component is for. Not behavior; used for review and diagnostics.
    pub description: String,
}

impl Component {
    pub fn new(
        kind: ComponentKind,
        authority: AuthorityClass,
        content: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            authority,
            content: content.into(),
            description: description.into(),
        }
    }

    pub fn green(
        kind: ComponentKind,
        content: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self::new(kind, AuthorityClass::Green, content, description)
    }

    pub fn yellow(
        kind: ComponentKind,
        content: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self::new(kind, AuthorityClass::Yellow, content, description)
    }

    pub fn red(
        kind: ComponentKind,
        content: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self::new(kind, AuthorityClass::Red, content, description)
    }

    /// The class that actually applies to a change of this component.
    pub fn effective_authority(&self) -> AuthorityClass {
        self.authority.max(self.kind.minimum_authority())
    }

    /// SHA-256 over the canonical encoding of this component.
    pub fn content_hash(&self) -> String {
        let mut out = Vec::new();
        push_field(&mut out, self.kind.id());
        push_field(&mut out, self.authority.id());
        push_field(&mut out, &self.content);
        push_field(&mut out, &self.description);
        sha256_hex(&out)
    }
}

/// An immutable, content-addressed harness manifest.
///
/// There is no mutation API on purpose. To change a manifest, build a new one with
/// [`HarnessManifest::with_component`], [`HarnessManifest::without_component`] or
/// [`HarnessManifest::apply_changes`]; the revision that owns the manifest then has a new hash.
///
/// Serialization is a sorted `Vec<Component>` (not a JSON map) so the bytes are canonical
/// regardless of how the manifest was constructed. Hash canonicalization is explicit and
/// versioned by [`MANIFEST_HASH_DOMAIN`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HarnessManifest {
    components: BTreeMap<ComponentKind, Component>,
}

#[derive(Serialize, Deserialize)]
struct ManifestWire {
    components: Vec<Component>,
}

impl Serialize for HarnessManifest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let wire = ManifestWire {
            components: self.components.values().cloned().collect(),
        };
        wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for HarnessManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ManifestWire::deserialize(deserializer)?;
        let mut manifest = Self::empty();
        for component in wire.components {
            if manifest
                .components
                .insert(component.kind, component)
                .is_some()
            {
                return Err(<D::Error as serde::de::Error>::custom(
                    "duplicate component kind in harness manifest",
                ));
            }
        }
        Ok(manifest)
    }
}

impl HarnessManifest {
    /// The empty manifest. A revision built from it has a well-defined hash.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build a manifest from components. Later entries win for a duplicate kind — use
    /// [`HarnessManifest::with_component`] in a builder chain for the same effect.
    pub fn from_components<I>(components: I) -> Self
    where
        I: IntoIterator<Item = Component>,
    {
        let mut manifest = Self::empty();
        for component in components {
            manifest = manifest.with_component(component);
        }
        manifest
    }

    /// Read-only view of the components. There is no `_mut` twin.
    pub fn components(&self) -> &BTreeMap<ComponentKind, Component> {
        &self.components
    }

    pub fn get(&self, kind: ComponentKind) -> Option<&Component> {
        self.components.get(&kind)
    }

    /// A component's declared authority, if the manifest carries it.
    pub fn component_authority(&self, kind: ComponentKind) -> Option<AuthorityClass> {
        self.components
            .get(&kind)
            .map(|component| component.authority)
    }

    /// A component's effective authority (declared vs. kind floor), if present.
    pub fn effective_authority(&self, kind: ComponentKind) -> Option<AuthorityClass> {
        self.components
            .get(&kind)
            .map(Component::effective_authority)
    }

    pub fn len(&self) -> usize {
        self.components.len()
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ComponentKind, &Component)> {
        self.components.iter()
    }

    pub fn kinds(&self) -> impl Iterator<Item = ComponentKind> + '_ {
        self.components.keys().copied()
    }

    /// The highest declared authority in this manifest.
    pub fn highest_authority(&self) -> Option<AuthorityClass> {
        self.components
            .values()
            .map(|component| component.authority)
            .max()
    }

    /// The most protective effective authority in this manifest.
    pub fn highest_effective_authority(&self) -> Option<AuthorityClass> {
        self.components
            .values()
            .map(Component::effective_authority)
            .max()
    }

    /// A new manifest with `component` set. The receiver is untouched.
    pub fn with_component(mut self, component: Component) -> Self {
        self.components.insert(component.kind, component);
        self
    }

    pub fn with_components<I>(mut self, components: I) -> Self
    where
        I: IntoIterator<Item = Component>,
    {
        for component in components {
            self.components.insert(component.kind, component);
        }
        self
    }

    /// A new manifest without `kind`. The receiver is untouched.
    pub fn without_component(mut self, kind: ComponentKind) -> Self {
        self.components.remove(&kind);
        self
    }

    /// Apply a change list to produce a new manifest. Order matters only for duplicate kinds;
    /// the policy refuses duplicates before a candidate is probed.
    pub fn apply_changes(&self, changes: &[ComponentChange]) -> Result<Self, EvolutionError> {
        let mut manifest = self.clone();
        for change in changes {
            change.validate_shape()?;
            match change.action {
                ChangeAction::Set => {
                    let component = change.component.clone().ok_or_else(|| {
                        EvolutionError::InvalidComponentChange(format!(
                            "SET {} has no component",
                            change.kind
                        ))
                    })?;
                    if component.kind != change.kind {
                        return Err(EvolutionError::InvalidComponentChange(format!(
                            "SET {} carries a {} component",
                            change.kind, component.kind
                        )));
                    }
                    manifest.components.insert(component.kind, component);
                }
                ChangeAction::Remove => {
                    manifest.components.remove(&change.kind);
                }
            }
        }
        Ok(manifest)
    }

    /// Canonical bytes hashed by [`HarnessManifest::manifest_hash`].
    ///
    /// Sorted by [`ComponentKind`], length-prefixed, domain-separated. Deterministic across
    /// insertion order, machines and process restarts.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MANIFEST_HASH_DOMAIN.as_bytes());
        out.extend_from_slice(&(self.components.len() as u64).to_le_bytes());
        for component in self.components.values() {
            push_field(&mut out, component.kind.id());
            push_field(&mut out, component.authority.id());
            push_field(&mut out, &component.content);
            push_field(&mut out, &component.description);
        }
        out
    }

    /// Stable content hash. Two manifests with the same components in any construction order
    /// hash identically; any content, authority, description or kind change changes the hash.
    pub fn manifest_hash(&self) -> String {
        sha256_hex(&self.canonical_bytes())
    }
}

fn push_field(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        // Writing to a String cannot fail; avoid `unwrap`/`expect` in library code.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// A harness domain. Experience libraries are scoped per domain; domains may run concurrently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarnessDomain {
    ForgeWorkflow,
    RustBackend,
    YewMvi,
    DatabaseNeon,
    SecurityEntitlements,
    QaTesting,
    DevopsRelease,
    Integrations,
    ExecutiveSwarm,
}

impl HarnessDomain {
    pub const ALL: [Self; 9] = [
        Self::ForgeWorkflow,
        Self::RustBackend,
        Self::YewMvi,
        Self::DatabaseNeon,
        Self::SecurityEntitlements,
        Self::QaTesting,
        Self::DevopsRelease,
        Self::Integrations,
        Self::ExecutiveSwarm,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::ForgeWorkflow => "forge-workflow",
            Self::RustBackend => "rust-backend",
            Self::YewMvi => "yew-mvi",
            Self::DatabaseNeon => "database-neon",
            Self::SecurityEntitlements => "security-entitlements",
            Self::QaTesting => "qa-testing",
            Self::DevopsRelease => "devops-release",
            Self::Integrations => "integrations",
            Self::ExecutiveSwarm => "executive-swarm",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|domain| domain.id() == id)
    }
}

impl fmt::Display for HarnessDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// Revision lifecycle. Transitions are a closed table — a revision advances or it is terminated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionStatus {
    Draft,
    Proposed,
    ProbeReady,
    Probing,
    Compared,
    Composed,
    Validating,
    Adoptable,
    Adopted,
    Rejected,
    Failed,
    Superseded,
    RolledBack,
}

impl RevisionStatus {
    pub const ALL: [Self; 13] = [
        Self::Draft,
        Self::Proposed,
        Self::ProbeReady,
        Self::Probing,
        Self::Compared,
        Self::Composed,
        Self::Validating,
        Self::Adoptable,
        Self::Adopted,
        Self::Rejected,
        Self::Failed,
        Self::Superseded,
        Self::RolledBack,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Proposed => "proposed",
            Self::ProbeReady => "probe_ready",
            Self::Probing => "probing",
            Self::Compared => "compared",
            Self::Composed => "composed",
            Self::Validating => "validating",
            Self::Adoptable => "adoptable",
            Self::Adopted => "adopted",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
            Self::Superseded => "superseded",
            Self::RolledBack => "rolled_back",
        }
    }

    /// Explicit lifecycle table. Anything not listed is refused by
    /// [`HarnessRevision::try_transition`].
    pub const fn can_transition_to(self, next: Self) -> bool {
        match self {
            Self::Draft => matches!(next, Self::Proposed | Self::Rejected | Self::Failed),
            Self::Proposed => matches!(next, Self::ProbeReady | Self::Rejected | Self::Failed),
            Self::ProbeReady => matches!(next, Self::Probing | Self::Rejected | Self::Failed),
            Self::Probing => matches!(next, Self::Compared | Self::Failed),
            Self::Compared => matches!(next, Self::Composed | Self::Rejected | Self::Failed),
            Self::Composed => matches!(next, Self::Validating | Self::Rejected | Self::Failed),
            Self::Validating => matches!(next, Self::Adoptable | Self::Rejected | Self::Failed),
            Self::Adoptable => matches!(next, Self::Adopted | Self::Rejected | Self::Failed),
            Self::Adopted => matches!(next, Self::Superseded | Self::RolledBack),
            // A rollback restores the prior revision; a superseded revision may become active again.
            Self::Superseded => matches!(next, Self::Adopted | Self::RolledBack),
            Self::Rejected | Self::Failed | Self::RolledBack => false,
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Rejected | Self::Failed | Self::RolledBack)
    }

    pub const fn is_active(self) -> bool {
        matches!(self, Self::Adopted)
    }
}

impl fmt::Display for RevisionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// One immutable harness revision. `manifest_hash` is derived from `manifest` at construction
/// and checked with [`HarnessRevision::verify_integrity`] before use.
///
/// The fields are public so persistence can rehydrate rows without a second parallel type; the
/// discipline is "construct a new value, never mutate one in place". `try_transition` returns a
/// new revision; `with_*` builders return a new revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessRevision {
    pub id: String,
    pub parent_revision_id: Option<String>,
    pub domain: HarnessDomain,
    pub status: RevisionStatus,
    pub manifest: HarnessManifest,
    pub manifest_hash: String,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
}

impl HarnessRevision {
    /// A root draft revision. The hash is computed from the manifest, never supplied by a caller.
    pub fn draft(
        id: impl Into<String>,
        domain: HarnessDomain,
        manifest: HarnessManifest,
        created_by: impl Into<String>,
    ) -> Self {
        let manifest_hash = manifest.manifest_hash();
        Self {
            id: id.into(),
            parent_revision_id: None,
            domain,
            status: RevisionStatus::Draft,
            manifest,
            manifest_hash,
            created_at: Utc::now(),
            created_by: created_by.into(),
        }
    }

    /// A child revision of `base` whose manifest is exactly `base.manifest + changes`. This is
    /// the only sanctioned way to turn a candidate change list into a revision: it makes
    /// "candidate manifest = base manifest + declared changes" structural, not a review note.
    pub fn from_changes(
        base: &HarnessRevision,
        id: impl Into<String>,
        changes: &[ComponentChange],
        created_by: impl Into<String>,
    ) -> Result<Self, EvolutionError> {
        let manifest = base.manifest.apply_changes(changes)?;
        Ok(Self::draft(id, base.domain, manifest, created_by).with_parent(Some(base.id.clone())))
    }

    pub fn with_parent(mut self, parent_revision_id: Option<String>) -> Self {
        self.parent_revision_id = parent_revision_id;
        self
    }

    pub fn with_status(mut self, status: RevisionStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_domain(mut self, domain: HarnessDomain) -> Self {
        self.domain = domain;
        self
    }

    pub fn with_created_at(mut self, created_at: DateTime<Utc>) -> Self {
        self.created_at = created_at;
        self
    }

    pub fn with_created_by(mut self, created_by: impl Into<String>) -> Self {
        self.created_by = created_by.into();
        self
    }

    /// A new manifest on this revision, with the hash recomputed. Returns a new revision.
    pub fn with_manifest(mut self, manifest: HarnessManifest) -> Self {
        self.manifest_hash = manifest.manifest_hash();
        self.manifest = manifest;
        self
    }

    /// Advance the lifecycle. Unknown transitions are a typed error, not a silent status write.
    pub fn try_transition(self, next: RevisionStatus) -> Result<Self, EvolutionError> {
        if self.status.can_transition_to(next) {
            Ok(self.with_status(next))
        } else {
            Err(EvolutionError::InvalidRevisionTransition {
                from: self.status,
                to: next,
            })
        }
    }

    /// Recompute the hash from the manifest and compare it with the stored value.
    pub fn verify_integrity(&self) -> Result<(), EvolutionError> {
        let computed = self.manifest.manifest_hash();
        if computed == self.manifest_hash {
            Ok(())
        } else {
            Err(EvolutionError::InvalidManifestHash {
                expected: self.manifest_hash.clone(),
                actual: computed,
            })
        }
    }

    /// Check the stored hash against a hash read from another source (e.g. a DB row) without
    /// trusting either side.
    pub fn verify_against(&self, stored_manifest_hash: &str) -> Result<(), EvolutionError> {
        let computed = self.manifest.manifest_hash();
        if computed == stored_manifest_hash && computed == self.manifest_hash {
            Ok(())
        } else {
            Err(EvolutionError::InvalidManifestHash {
                expected: stored_manifest_hash.to_string(),
                actual: computed,
            })
        }
    }
}

/// What a change does to a component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeAction {
    Set,
    Remove,
}

impl ChangeAction {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Set => "set",
            Self::Remove => "remove",
        }
    }
}

/// One proposed component change, with its rationale. A change without a rationale is malformed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentChange {
    pub kind: ComponentKind,
    pub action: ChangeAction,
    pub component: Option<Component>,
    pub rationale: String,
}

impl ComponentChange {
    pub fn set(component: Component, rationale: impl Into<String>) -> Self {
        Self {
            kind: component.kind,
            action: ChangeAction::Set,
            component: Some(component),
            rationale: rationale.into(),
        }
    }

    pub fn remove(kind: ComponentKind, rationale: impl Into<String>) -> Self {
        Self {
            kind,
            action: ChangeAction::Remove,
            component: None,
            rationale: rationale.into(),
        }
    }

    /// Structural validity: a Set carries a matching component, a Remove carries none, and
    /// every change says why. The policy calls this before it looks at authority.
    pub fn validate_shape(&self) -> Result<(), EvolutionError> {
        if self.rationale.trim().is_empty() {
            return Err(EvolutionError::InvalidComponentChange(format!(
                "{} change carries no rationale",
                self.kind
            )));
        }
        match self.action {
            ChangeAction::Set => match &self.component {
                Some(component) if component.kind == self.kind => Ok(()),
                Some(component) => Err(EvolutionError::InvalidComponentChange(format!(
                    "SET {} carries a {} component",
                    self.kind, component.kind
                ))),
                None => Err(EvolutionError::InvalidComponentChange(format!(
                    "SET {} has no component",
                    self.kind
                ))),
            },
            ChangeAction::Remove => {
                if self.component.is_some() {
                    Err(EvolutionError::InvalidComponentChange(format!(
                        "REMOVE {} must not carry a component",
                        self.kind
                    )))
                } else {
                    Ok(())
                }
            }
        }
    }

    /// The declared authority of a Set change, if any.
    pub fn target_authority(&self) -> Option<AuthorityClass> {
        self.component.as_ref().map(|component| component.authority)
    }
}

/// Invariant candidates improve the general harness; domain candidates add domain knowledge.
/// They are proposed separately and only combined at Compose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalKind {
    Invariant,
    Domain,
}

impl ProposalKind {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Invariant => "invariant",
            Self::Domain => "domain",
        }
    }
}

/// A candidate harness, expressed as changes against one base revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub base_revision: String,
    pub kind: ProposalKind,
    pub changes: Vec<ComponentChange>,
}

impl Candidate {
    pub fn new(
        id: impl Into<String>,
        base_revision: impl Into<String>,
        kind: ProposalKind,
        changes: Vec<ComponentChange>,
    ) -> Self {
        Self {
            id: id.into(),
            base_revision: base_revision.into(),
            kind,
            changes,
        }
    }

    /// Non-empty identity, at least one well-formed change.
    pub fn validate(&self) -> Result<(), EvolutionError> {
        if self.id.trim().is_empty() {
            return Err(EvolutionError::InvalidCandidate(
                "candidate id is empty".into(),
            ));
        }
        if self.base_revision.trim().is_empty() {
            return Err(EvolutionError::InvalidCandidate(
                "candidate base revision is empty".into(),
            ));
        }
        if self.changes.is_empty() {
            return Err(EvolutionError::InvalidCandidate(format!(
                "candidate {} has no changes",
                self.id
            )));
        }
        for change in &self.changes {
            change.validate_shape()?;
        }
        Ok(())
    }

    /// Distinct component kinds this candidate touches, in change order.
    pub fn target_kinds(&self) -> Vec<ComponentKind> {
        let mut kinds = Vec::new();
        for change in &self.changes {
            if !kinds.contains(&change.kind) {
                kinds.push(change.kind);
            }
        }
        kinds
    }
}

/// Recursion mode. `k = 1` is the default; k > 1 is implemented by
/// `experiment::run_recursion` (parallel: one shared evidence phase and k compositions;
/// sequential: k chained single passes), with the mode and budget carried here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvolutionMode {
    Parallel,
    Sequential,
}

impl EvolutionMode {
    pub const ALL: [Self; 2] = [Self::Parallel, Self::Sequential];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Parallel => "parallel",
            Self::Sequential => "sequential",
        }
    }
}

/// The Harness² recursion budget. [`EvolutionBudget::k1`] is the default; k > 1 is implemented by
/// `experiment::run_recursion`, which refuses a budget above the recursion ceilings before any
/// stage runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvolutionBudget {
    /// k: reference executions of the base harness, and candidates per scope.
    pub k: u32,
    /// Propose–Probe–Compose steps for sequential mode (sequential recursion requires
    /// `max_steps == k`). The default is 1.
    pub max_steps: u32,
    /// Hard cap on probe executions for one experiment.
    pub max_probes: u32,
}

impl EvolutionBudget {
    /// The v1 budget: one reference run, one invariant + one domain candidate, one step.
    pub const fn k1() -> Self {
        Self {
            k: 1,
            max_steps: 1,
            max_probes: 3,
        }
    }

    pub fn reference_runs(&self) -> u32 {
        self.k
    }

    pub fn candidates_per_scope(&self) -> u32 {
        self.k
    }

    pub fn is_k1(&self) -> bool {
        self.k == 1 && self.max_steps == 1
    }

    pub fn validate(&self) -> Result<(), EvolutionError> {
        if self.k == 0 {
            return Err(EvolutionError::InvalidBudget("k must be at least 1".into()));
        }
        if self.max_steps == 0 {
            return Err(EvolutionError::InvalidBudget(
                "max_steps must be at least 1".into(),
            ));
        }
        if self.max_probes < self.k.saturating_mul(2) {
            return Err(EvolutionError::InvalidBudget(
                "max_probes must cover one probe per candidate scope".into(),
            ));
        }
        Ok(())
    }
}

impl Default for EvolutionBudget {
    fn default() -> Self {
        Self::k1()
    }
}

/// The candidates proposed for one step: invariant and domain candidates stay separate until
/// Compose. The set is a working value, not a durable harness revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateSet {
    pub invariant: Vec<Candidate>,
    pub domain: Vec<Candidate>,
    pub mode: EvolutionMode,
    pub budget: EvolutionBudget,
}

impl CandidateSet {
    pub fn new(mode: EvolutionMode, budget: EvolutionBudget) -> Self {
        Self {
            invariant: Vec::new(),
            domain: Vec::new(),
            mode,
            budget,
        }
    }

    pub fn k1() -> Self {
        Self::new(EvolutionMode::Parallel, EvolutionBudget::k1())
    }

    pub fn all(&self) -> impl Iterator<Item = &Candidate> {
        self.invariant.iter().chain(self.domain.iter())
    }

    pub fn len(&self) -> usize {
        self.invariant.len() + self.domain.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn within_budget(&self) -> bool {
        let per_scope = self.budget.candidates_per_scope() as usize;
        self.invariant.len() <= per_scope && self.domain.len() <= per_scope
    }

    /// A set is usable when the budget is valid, both scopes are within k, at least one
    /// candidate exists, every candidate is well formed, and all share one base revision.
    pub fn validate(&self) -> Result<(), EvolutionError> {
        self.budget.validate()?;
        if self.is_empty() {
            return Err(EvolutionError::InvalidCandidate(
                "candidate set is empty".into(),
            ));
        }
        if !self.within_budget() {
            return Err(EvolutionError::InvalidCandidate(format!(
                "candidate set exceeds k={} per scope",
                self.budget.k
            )));
        }
        let mut base: Option<&str> = None;
        let mut ids: Vec<&str> = Vec::new();
        for candidate in self.all() {
            candidate.validate()?;
            match base {
                None => base = Some(candidate.base_revision.as_str()),
                Some(expected) if expected != candidate.base_revision => {
                    return Err(EvolutionError::InvalidCandidate(
                        "candidates in one set must target the same base revision".into(),
                    ));
                }
                Some(_) => {}
            }
            if ids.contains(&candidate.id.as_str()) {
                return Err(EvolutionError::InvalidCandidate(format!(
                    "duplicate candidate id {}",
                    candidate.id
                )));
            }
            ids.push(candidate.id.as_str());
        }
        Ok(())
    }
}

/// Where a probe may execute. A live production execution class is never a probe target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentClass {
    Sandbox,
    Development,
    Ci,
    Staging,
    Production,
}

impl EnvironmentClass {
    pub const ALL: [Self; 5] = [
        Self::Sandbox,
        Self::Development,
        Self::Ci,
        Self::Staging,
        Self::Production,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Sandbox => "sandbox",
            Self::Development => "development",
            Self::Ci => "ci",
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|class| class.id() == id)
    }

    /// Production is not a valid probe target — live traffic is not an experiment surface.
    pub const fn allows_probe(self) -> bool {
        !matches!(self, Self::Production)
    }

    pub const fn is_production(self) -> bool {
        matches!(self, Self::Production)
    }
}

/// The pins that must match before two executions can be compared behaviorally.
///
/// Harness² contrasts executions that differ only in harness revision. If any identity pin
/// (model, model policy, executor, tools, environment class, input dataset) differs, the
/// contrast is invalid and cannot justify composition or adoption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeIdentity {
    pub model: String,
    pub model_policy: String,
    pub executor: String,
    pub tools: Vec<String>,
    pub environment_class: EnvironmentClass,
    pub input_dataset: String,
}

impl ProbeIdentity {
    /// Compare identities with tools canonicalized (trim, drop empty, dedupe, sort).
    pub fn matches(&self, other: &Self) -> bool {
        self.model == other.model
            && self.model_policy == other.model_policy
            && self.executor == other.executor
            && self.environment_class == other.environment_class
            && self.input_dataset == other.input_dataset
            && canonical_tools(&self.tools) == canonical_tools(&other.tools)
    }

    /// Short human-readable summary for violation messages.
    pub fn summary(&self) -> String {
        format!(
            "model={} policy={} executor={} environment={} dataset={} tools={}",
            self.model,
            self.model_policy,
            self.executor,
            self.environment_class.id(),
            self.input_dataset,
            canonical_tools(&self.tools).join(",")
        )
    }
}

/// Everything that must be pinned before a probe executes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeSpec {
    pub task: String,
    pub base_revision: String,
    pub candidate_revision: String,
    pub model: String,
    pub model_policy: String,
    pub executor: String,
    pub tools: Vec<String>,
    pub environment_class: EnvironmentClass,
    pub input_dataset: String,
    pub test_plan: String,
    pub replay_id: String,
}

impl ProbeSpec {
    /// A minimally populated spec. Fill the pins with the `with_*` builders before validating.
    pub fn new(
        task: impl Into<String>,
        base_revision: impl Into<String>,
        candidate_revision: impl Into<String>,
    ) -> Self {
        Self {
            task: task.into(),
            base_revision: base_revision.into(),
            candidate_revision: candidate_revision.into(),
            model: String::new(),
            model_policy: "default".to_string(),
            executor: "opencode".to_string(),
            tools: Vec::new(),
            environment_class: EnvironmentClass::Development,
            input_dataset: String::new(),
            test_plan: String::new(),
            replay_id: String::new(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn with_model_policy(mut self, model_policy: impl Into<String>) -> Self {
        self.model_policy = model_policy.into();
        self
    }

    pub fn with_executor(mut self, executor: impl Into<String>) -> Self {
        self.executor = executor.into();
        self
    }

    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.tools = tools;
        self
    }

    pub fn with_environment_class(mut self, environment_class: EnvironmentClass) -> Self {
        self.environment_class = environment_class;
        self
    }

    pub fn with_input_dataset(mut self, input_dataset: impl Into<String>) -> Self {
        self.input_dataset = input_dataset.into();
        self
    }

    pub fn with_test_plan(mut self, test_plan: impl Into<String>) -> Self {
        self.test_plan = test_plan.into();
        self
    }

    pub fn with_replay_id(mut self, replay_id: impl Into<String>) -> Self {
        self.replay_id = replay_id.into();
        self
    }

    pub fn identity(&self) -> ProbeIdentity {
        ProbeIdentity {
            model: self.model.clone(),
            model_policy: self.model_policy.clone(),
            executor: self.executor.clone(),
            tools: canonical_tools(&self.tools),
            environment_class: self.environment_class,
            input_dataset: self.input_dataset.clone(),
        }
    }

    /// Hard safety checks: base ≠ candidate and no production execution.
    pub fn validate_identity(&self) -> Result<(), EvolutionError> {
        if self.base_revision.trim().is_empty() || self.candidate_revision.trim().is_empty() {
            return Err(EvolutionError::InvalidProbe(
                "probe must name both revisions".into(),
            ));
        }
        if self.base_revision == self.candidate_revision {
            return Err(EvolutionError::SameRevisionProbe {
                revision: self.base_revision.clone(),
            });
        }
        if self.environment_class.is_production() {
            return Err(EvolutionError::ProductionProbeForbidden {
                environment_class: self.environment_class,
            });
        }
        Ok(())
    }

    /// [`ProbeSpec::validate_identity`] plus a complete pin set. Fail closed before execution.
    pub fn validate_complete(&self) -> Result<(), EvolutionError> {
        self.validate_identity()?;
        let missing: Vec<&str> = [
            ("task", self.task.as_str()),
            ("model", self.model.as_str()),
            ("executor", self.executor.as_str()),
            ("input_dataset", self.input_dataset.as_str()),
            ("test_plan", self.test_plan.as_str()),
            ("replay_id", self.replay_id.as_str()),
        ]
        .into_iter()
        .filter(|(_, value)| value.trim().is_empty())
        .map(|(name, _)| name)
        .collect();
        if !missing.is_empty() {
            return Err(EvolutionError::InvalidProbe(format!(
                "probe is missing pins: {}",
                missing.join(", ")
            )));
        }
        Ok(())
    }
}

fn canonical_tools(tools: &[String]) -> Vec<String> {
    let mut canonical: Vec<String> = tools
        .iter()
        .map(|tool| tool.trim().to_string())
        .filter(|tool| !tool.is_empty())
        .collect();
    canonical.sort();
    canonical.dedup();
    canonical
}

/// QA's independent verdict for one probed execution.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum QaVerdict {
    #[default]
    NotRun,
    Pass,
    Fail,
    Inconclusive,
}

impl QaVerdict {
    pub const fn id(self) -> &'static str {
        match self {
            Self::NotRun => "not_run",
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Inconclusive => "inconclusive",
        }
    }
}

/// Behavioral evidence for one execution, keyed to durable evidence ids.
///
/// This is a *reference* model: `evidence_refs` points at Forge rows/receipts, it is not a
/// second telemetry capture. `cost_micros` is `Option` because "not priced" is not zero.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BehaviorEvidence {
    pub evidence_id: String,
    pub probe_id: String,
    pub success: bool,
    pub tests_passed: u32,
    pub tests_failed: u32,
    pub qa: QaVerdict,
    pub architecture_violations: u32,
    pub tool_failures: u32,
    pub unnecessary_calls: u32,
    pub steps: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    /// Vendor cost in micro-units, or `None` when the run is unpriced. Never imputed as zero.
    pub cost_micros: Option<u64>,
    pub elapsed_ms: u64,
    pub repairs: u32,
    pub escalations: u32,
    pub holds: u32,
    pub scope_drift_files: u32,
    pub forbidden_attempts: u32,
    pub evidence_refs: Vec<String>,
}

impl BehaviorEvidence {
    pub fn new(evidence_id: impl Into<String>, probe_id: impl Into<String>) -> Self {
        Self {
            evidence_id: evidence_id.into(),
            probe_id: probe_id.into(),
            ..Self::default()
        }
    }

    pub fn tokens_total(&self) -> u64 {
        self.tokens_in.saturating_add(self.tokens_out)
    }

    /// Strictly clean behavior: success, passing tests and QA, no violations or forbidden
    /// attempts. A missing QA verdict is not clean.
    pub fn is_behaviorally_clean(&self) -> bool {
        self.success
            && self.tests_failed == 0
            && self.qa == QaVerdict::Pass
            && self.architecture_violations == 0
            && self.tool_failures == 0
            && self.scope_drift_files == 0
            && self.forbidden_attempts == 0
    }

    /// At least one reference back to durable Forge evidence.
    pub fn has_evidence(&self) -> bool {
        !self.evidence_refs.is_empty()
    }

    pub fn validate(&self) -> Result<(), EvolutionError> {
        if self.evidence_id.trim().is_empty() {
            return Err(EvolutionError::MissingEvidence(
                "behavior evidence has no evidence id".into(),
            ));
        }
        if self.probe_id.trim().is_empty() {
            return Err(EvolutionError::MissingEvidence(
                "behavior evidence has no probe id".into(),
            ));
        }
        Ok(())
    }
}

/// One behavioral dimension Harness² contrasts. Every dimension is a count or a verdict —
/// there is no "looks nicer" dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorDimension {
    Success,
    TestsPassed,
    TestsFailed,
    Qa,
    ArchitectureViolations,
    ToolFailures,
    UnnecessaryCalls,
    Steps,
    Tokens,
    Cost,
    Elapsed,
    Repairs,
    Escalations,
    Holds,
    ScopeDrift,
    ForbiddenAttempts,
}

impl BehaviorDimension {
    pub const ALL: [Self; 16] = [
        Self::Success,
        Self::TestsPassed,
        Self::TestsFailed,
        Self::Qa,
        Self::ArchitectureViolations,
        Self::ToolFailures,
        Self::UnnecessaryCalls,
        Self::Steps,
        Self::Tokens,
        Self::Cost,
        Self::Elapsed,
        Self::Repairs,
        Self::Escalations,
        Self::Holds,
        Self::ScopeDrift,
        Self::ForbiddenAttempts,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::TestsPassed => "tests_passed",
            Self::TestsFailed => "tests_failed",
            Self::Qa => "qa",
            Self::ArchitectureViolations => "architecture_violations",
            Self::ToolFailures => "tool_failures",
            Self::UnnecessaryCalls => "unnecessary_calls",
            Self::Steps => "steps",
            Self::Tokens => "tokens",
            Self::Cost => "cost",
            Self::Elapsed => "elapsed",
            Self::Repairs => "repairs",
            Self::Escalations => "escalations",
            Self::Holds => "holds",
            Self::ScopeDrift => "scope_drift",
            Self::ForbiddenAttempts => "forbidden_attempts",
        }
    }

    /// `Some(value)` only when the dimension was actually captured; `None` is "not captured"
    /// and is never treated as zero.
    pub fn value(self, evidence: &BehaviorEvidence) -> Option<f64> {
        match self {
            Self::Success => Some(if evidence.success { 1.0 } else { 0.0 }),
            Self::TestsPassed => Some(f64::from(evidence.tests_passed)),
            Self::TestsFailed => Some(f64::from(evidence.tests_failed)),
            Self::Qa => match evidence.qa {
                QaVerdict::Pass => Some(1.0),
                QaVerdict::Fail => Some(0.0),
                QaVerdict::NotRun | QaVerdict::Inconclusive => None,
            },
            Self::ArchitectureViolations => Some(f64::from(evidence.architecture_violations)),
            Self::ToolFailures => Some(f64::from(evidence.tool_failures)),
            Self::UnnecessaryCalls => Some(f64::from(evidence.unnecessary_calls)),
            Self::Steps => Some(evidence.steps as f64),
            Self::Tokens => Some(evidence.tokens_total() as f64),
            Self::Cost => evidence.cost_micros.map(|micros| micros as f64),
            Self::Elapsed => Some(evidence.elapsed_ms as f64),
            Self::Repairs => Some(f64::from(evidence.repairs)),
            Self::Escalations => Some(f64::from(evidence.escalations)),
            Self::Holds => Some(f64::from(evidence.holds)),
            Self::ScopeDrift => Some(f64::from(evidence.scope_drift_files)),
            Self::ForbiddenAttempts => Some(f64::from(evidence.forbidden_attempts)),
        }
    }

    /// True when a lower value is better for this dimension.
    pub fn better_is_lower(self) -> bool {
        !matches!(self, Self::Success | Self::TestsPassed | Self::Qa)
    }

    /// Direction of a behavior change, or `Unknown` when either side is uncaptured.
    pub fn direction(
        self,
        reference: &BehaviorEvidence,
        candidate: &BehaviorEvidence,
    ) -> ContrastDirection {
        let (Some(reference_value), Some(candidate_value)) =
            (self.value(reference), self.value(candidate))
        else {
            return ContrastDirection::Unknown;
        };
        let Some(ordering) = candidate_value.partial_cmp(&reference_value) else {
            return ContrastDirection::Unknown;
        };
        match ordering {
            std::cmp::Ordering::Equal => ContrastDirection::Unchanged,
            std::cmp::Ordering::Less if self.better_is_lower() => ContrastDirection::Improved,
            std::cmp::Ordering::Less => ContrastDirection::Regressed,
            std::cmp::Ordering::Greater if self.better_is_lower() => ContrastDirection::Regressed,
            std::cmp::Ordering::Greater => ContrastDirection::Improved,
        }
    }
}

/// Direction of one behavioral dimension. `Unknown` is not "unchanged" — it means the evidence
/// for that dimension was not captured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContrastDirection {
    Improved,
    Regressed,
    Unchanged,
    Unknown,
}

impl ContrastDirection {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Improved => "improved",
            Self::Regressed => "regressed",
            Self::Unchanged => "unchanged",
            Self::Unknown => "unknown",
        }
    }
}

/// One dimension's contrast, with the raw values and its evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionContrast {
    pub dimension: BehaviorDimension,
    pub direction: ContrastDirection,
    pub reference_value: Option<f64>,
    pub candidate_value: Option<f64>,
    pub note: String,
    pub evidence_refs: Vec<String>,
}

impl DimensionContrast {
    pub fn from_evidence(
        dimension: BehaviorDimension,
        reference: &BehaviorEvidence,
        candidate: &BehaviorEvidence,
        note: impl Into<String>,
    ) -> Self {
        Self {
            dimension,
            direction: dimension.direction(reference, candidate),
            reference_value: dimension.value(reference),
            candidate_value: dimension.value(candidate),
            note: note.into(),
            evidence_refs: Vec::new(),
        }
    }

    pub fn with_evidence_refs(mut self, evidence_refs: Vec<String>) -> Self {
        self.evidence_refs = evidence_refs;
        self
    }
}

/// Behavioral contrast between a reference and a candidate execution. This is the only
/// legitimate basis for composing a successor; identity mismatch invalidates the report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContrastReport {
    pub id: String,
    pub base_revision: String,
    pub candidate_revision: String,
    pub reference_identity: ProbeIdentity,
    pub candidate_identity: ProbeIdentity,
    pub dimensions: Vec<DimensionContrast>,
    pub summary: String,
    pub evidence_refs: Vec<String>,
}

impl ContrastReport {
    pub fn identities_match(&self) -> bool {
        self.reference_identity.matches(&self.candidate_identity)
    }

    pub fn regressions(&self) -> Vec<BehaviorDimension> {
        self.dimensions
            .iter()
            .filter(|contrast| contrast.direction == ContrastDirection::Regressed)
            .map(|contrast| contrast.dimension)
            .collect()
    }

    pub fn improvements(&self) -> Vec<BehaviorDimension> {
        self.dimensions
            .iter()
            .filter(|contrast| contrast.direction == ContrastDirection::Improved)
            .map(|contrast| contrast.dimension)
            .collect()
    }

    /// A report is usable when it is complete and its executions are comparable. Environment
    /// or identity mismatch is a typed error, never a "close enough" contrast.
    pub fn validate(&self) -> Result<(), EvolutionError> {
        if self.id.trim().is_empty()
            || self.base_revision.trim().is_empty()
            || self.candidate_revision.trim().is_empty()
        {
            return Err(EvolutionError::MissingEvidence(
                "contrast report lacks its identity".into(),
            ));
        }
        if self.dimensions.is_empty() {
            return Err(EvolutionError::MissingEvidence(
                "contrast report has no behavioral dimensions".into(),
            ));
        }
        if !self.identities_match() {
            return Err(EvolutionError::ProbeIdentityMismatch {
                reference: self.reference_identity.summary(),
                candidate: self.candidate_identity.summary(),
            });
        }
        Ok(())
    }
}

/// One component-level decision in a composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentDecision {
    Keep,
    Adopt,
    Reject,
    Merge,
}

impl ComponentDecision {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Adopt => "adopt",
            Self::Reject => "reject",
            Self::Merge => "merge",
        }
    }

    /// Decisions that change the harness must be evidence-linked; Keep may stand on the base.
    pub const fn requires_evidence(self) -> bool {
        !matches!(self, Self::Keep)
    }
}

/// A component decision with the evidence for and against it and a confidence in percent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentDecisionRecord {
    pub kind: ComponentKind,
    pub decision: ComponentDecision,
    pub evidence_for: Vec<String>,
    pub evidence_against: Vec<String>,
    pub confidence: u8,
    pub rationale: String,
}

impl ComponentDecisionRecord {
    pub fn new(
        kind: ComponentKind,
        decision: ComponentDecision,
        rationale: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            decision,
            evidence_for: Vec::new(),
            evidence_against: Vec::new(),
            confidence: 0,
            rationale: rationale.into(),
        }
    }

    pub fn with_evidence_for(mut self, evidence_for: Vec<String>) -> Self {
        self.evidence_for = evidence_for;
        self
    }

    pub fn with_evidence_against(mut self, evidence_against: Vec<String>) -> Self {
        self.evidence_against = evidence_against;
        self
    }

    pub fn with_confidence(mut self, confidence: u8) -> Self {
        self.confidence = confidence;
        self
    }
}

/// The composition decision: how Behave evidence becomes a successor manifest. The successor
/// is a new manifest/hash; the base revision is never overwritten. Choosing the unchanged base
/// (all `Keep`) is legal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionDecision {
    pub id: String,
    pub base_revision: String,
    pub candidates: Vec<String>,
    pub contrast_ids: Vec<String>,
    pub decisions: Vec<ComponentDecisionRecord>,
    pub successor_manifest_hash: String,
    pub summary: String,
    pub evidence_refs: Vec<String>,
}

impl CompositionDecision {
    /// A composition is usable when it names its base and evidence, every changing decision has
    /// evidence, confidence is a percentage, and the successor hash is present.
    pub fn validate(&self) -> Result<(), EvolutionError> {
        if self.id.trim().is_empty() || self.base_revision.trim().is_empty() {
            return Err(EvolutionError::InvalidComposition(
                "composition lacks its identity".into(),
            ));
        }
        if self.candidates.is_empty() || self.contrast_ids.is_empty() {
            return Err(EvolutionError::InvalidComposition(
                "composition must name its candidates and contrasts".into(),
            ));
        }
        if self.decisions.is_empty() {
            return Err(EvolutionError::InvalidComposition(
                "composition has no component decisions".into(),
            ));
        }
        if self.successor_manifest_hash.trim().is_empty() {
            return Err(EvolutionError::InvalidComposition(
                "composition has no successor manifest hash".into(),
            ));
        }
        for record in &self.decisions {
            if record.confidence > 100 {
                return Err(EvolutionError::InvalidComposition(format!(
                    "confidence {} for {} is outside 0..=100",
                    record.confidence, record.kind
                )));
            }
            if record.decision.requires_evidence()
                && record.evidence_for.is_empty()
                && record.evidence_against.is_empty()
                && self.evidence_refs.is_empty()
            {
                return Err(EvolutionError::InvalidComposition(format!(
                    "{} decision for {} is not evidence-linked",
                    record.decision.id(),
                    record.kind
                )));
            }
        }
        Ok(())
    }
}

/// The four adoption outcomes plus the two escalation destinations. No other vocabulary is
/// allowed to mean "ship it".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdoptionVerdict {
    Adopt,
    Reject,
    MoreEvidence,
    Council,
    Chris,
}

impl AdoptionVerdict {
    pub const ALL: [Self; 5] = [
        Self::Adopt,
        Self::Reject,
        Self::MoreEvidence,
        Self::Council,
        Self::Chris,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Adopt => "adopt",
            Self::Reject => "reject",
            Self::MoreEvidence => "more_evidence",
            Self::Council => "council",
            Self::Chris => "chris",
        }
    }

    /// Only a clean Green adoption may be applied without a human gate.
    pub const fn is_automatic(self) -> bool {
        matches!(self, Self::Adopt)
    }
}

/// A durable adoption decision (the Lead's adoption module applies it through the catalog).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdoptionDecision {
    pub id: String,
    pub domain: HarnessDomain,
    pub revision_id: String,
    pub verdict: AdoptionVerdict,
    pub rationale: String,
    pub decided_by: String,
    pub evidence_refs: Vec<String>,
    pub decided_at: DateTime<Utc>,
}

impl AdoptionDecision {
    pub fn new(
        id: impl Into<String>,
        domain: HarnessDomain,
        revision_id: impl Into<String>,
        verdict: AdoptionVerdict,
        decided_by: impl Into<String>,
        rationale: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            domain,
            revision_id: revision_id.into(),
            verdict,
            rationale: rationale.into(),
            decided_by: decided_by.into(),
            evidence_refs: Vec::new(),
            decided_at: Utc::now(),
        }
    }

    pub fn with_evidence_refs(mut self, evidence_refs: Vec<String>) -> Self {
        self.evidence_refs = evidence_refs;
        self
    }

    /// An automatic decision is only valid when the verdict is Adopt and the policy (not a
    /// council/owner) made it.
    pub fn is_automatic(&self) -> bool {
        self.verdict.is_automatic() && self.decided_by.eq_ignore_ascii_case("policy")
    }

    pub fn requires_owner(&self) -> bool {
        matches!(self.verdict, AdoptionVerdict::Chris)
    }
}

/// Status of one Forge validation gate.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    #[default]
    NotRun,
    Passed,
    Failed,
}

impl GateStatus {
    pub const fn id(self) -> &'static str {
        match self {
            Self::NotRun => "not_run",
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

/// Forge validation evidence: a behavioral contrast alone is never sufficient to adopt.
///
/// The Smith `validate` stage produces this (or maps its outcome into it); the adoption policy
/// consumes it. `phase3_evidence` is the review of the Phase 3 signal that motivated the
/// candidate. Gate statuses distinguish "failed" from "not run": a failed gate rejects, an
/// unfinished gate asks for more evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ValidationEvidence {
    pub static_policy: GateStatus,
    pub regression_rails: GateStatus,
    pub replay: GateStatus,
    pub phase3_evidence: GateStatus,
    pub evidence_refs: Vec<String>,
}

impl ValidationEvidence {
    /// Evidence for a validation that passed every gate.
    pub fn all_passed(evidence_refs: Vec<String>) -> Self {
        Self {
            static_policy: GateStatus::Passed,
            regression_rails: GateStatus::Passed,
            replay: GateStatus::Passed,
            phase3_evidence: GateStatus::Passed,
            evidence_refs,
        }
    }

    /// No gate has run yet (the default).
    pub fn pending() -> Self {
        Self::default()
    }

    pub fn gates(&self) -> [(&'static str, GateStatus); 4] {
        [
            ("static_policy", self.static_policy),
            ("regression_rails", self.regression_rails),
            ("replay", self.replay),
            ("phase3_evidence", self.phase3_evidence),
        ]
    }

    pub fn failed_gates(&self) -> Vec<&'static str> {
        self.gates()
            .into_iter()
            .filter(|(_, status)| *status == GateStatus::Failed)
            .map(|(name, _)| name)
            .collect()
    }

    pub fn missing_gates(&self) -> Vec<&'static str> {
        self.gates()
            .into_iter()
            .filter(|(_, status)| *status == GateStatus::NotRun)
            .map(|(name, _)| name)
            .collect()
    }

    /// Every gate passed and the evidence is linked. This is the only state in which an
    /// automatic adoption may occur.
    pub fn is_clean(&self) -> bool {
        self.missing_gates().is_empty()
            && self.failed_gates().is_empty()
            && !self.evidence_refs.is_empty()
    }

    /// At least one validation gate has actually run.
    pub fn has_run(&self) -> bool {
        self.gates()
            .into_iter()
            .any(|(_, status)| status != GateStatus::NotRun)
    }

    /// Every gate reached a decision (passed or failed). An incomplete experiment must not
    /// update the experience library.
    pub fn is_complete(&self) -> bool {
        self.missing_gates().is_empty()
    }
}

/// The scope that separates experience libraries. Tasks within one scope read only earlier
/// entries; different domains may run concurrently. Harness² separates by benchmark, executor,
/// mode, budget and seed — here the benchmark role is played by the harness domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperienceScope {
    pub domain: HarnessDomain,
    pub executor: String,
    pub mode: EvolutionMode,
    pub budget: EvolutionBudget,
    pub seed: u64,
}

impl ExperienceScope {
    pub fn new(
        domain: HarnessDomain,
        executor: impl Into<String>,
        mode: EvolutionMode,
        budget: EvolutionBudget,
        seed: u64,
    ) -> Self {
        Self {
            domain,
            executor: executor.into(),
            mode,
            budget,
            seed,
        }
    }

    pub fn k1(domain: HarnessDomain) -> Self {
        Self::new(
            domain,
            "opencode",
            EvolutionMode::Parallel,
            EvolutionBudget::k1(),
            0,
        )
    }
}

/// An evidence-supported lesson stored after an experiment. `insert` requires behavioral
/// contrast + validation evidence; no fabricated lessons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperienceRecord {
    pub id: String,
    pub scope: ExperienceScope,
    pub harness_revision: String,
    pub manifest_hash: String,
    pub lesson: String,
    pub applicability: Vec<String>,
    pub counterevidence: Vec<String>,
    pub confidence: u8,
    pub contrast_id: String,
    pub validation: ValidationEvidence,
    pub evidence_refs: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl ExperienceRecord {
    /// An insertable lesson must say what was learned, cite the behavioral contrast, cite a
    /// completed validation (passed or failed gates — both are lessons), and link evidence.
    pub fn validate(&self) -> Result<(), EvolutionError> {
        if self.id.trim().is_empty() {
            return Err(EvolutionError::InvalidExperience(
                "experience has no id".into(),
            ));
        }
        if self.lesson.trim().is_empty() {
            return Err(EvolutionError::InvalidExperience(
                "experience has no lesson".into(),
            ));
        }
        if self.scope.executor.trim().is_empty() {
            return Err(EvolutionError::InvalidExperience(
                "experience scope has no executor".into(),
            ));
        }
        if self.contrast_id.trim().is_empty() {
            return Err(EvolutionError::InvalidExperience(
                "experience has no behavioral contrast".into(),
            ));
        }
        if !self.validation.is_complete() {
            return Err(EvolutionError::InvalidExperience(format!(
                "experience validation is incomplete: {:?}",
                self.validation.missing_gates()
            )));
        }
        if self.evidence_refs.is_empty() {
            return Err(EvolutionError::InvalidExperience(
                "experience has no evidence refs".into(),
            ));
        }
        if self.applicability.is_empty() {
            return Err(EvolutionError::InvalidExperience(
                "experience has no applicability".into(),
            ));
        }
        if self.confidence > 100 {
            return Err(EvolutionError::InvalidExperience(
                "confidence is outside 0..=100".into(),
            ));
        }
        if self.manifest_hash.trim().is_empty() {
            return Err(EvolutionError::InvalidExperience(
                "experience has no harness manifest hash".into(),
            ));
        }
        Ok(())
    }

    /// A lesson stops guiding retrieval after its expiry.
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.is_some_and(|expires_at| now >= expires_at)
    }

    /// A record is not visible to an earlier task: tasks read only earlier entries.
    pub fn is_visible_at(&self, now: DateTime<Utc>) -> bool {
        self.created_at <= now && !self.is_expired(now)
    }
}

/// Typed failure for the Harness Evolution contracts. Every variant is a refusal, not a
/// degraded success.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvolutionError {
    InvalidManifestHash {
        expected: String,
        actual: String,
    },
    InvalidComponentChange(String),
    InvalidBudget(String),
    InvalidProbe(String),
    SameRevisionProbe {
        revision: String,
    },
    ProductionProbeForbidden {
        environment_class: EnvironmentClass,
    },
    ProbeIdentityMismatch {
        reference: String,
        candidate: String,
    },
    InvalidRevisionTransition {
        from: RevisionStatus,
        to: RevisionStatus,
    },
    MissingEvidence(String),
    InvalidCandidate(String),
    InvalidComposition(String),
    InvalidExperience(String),
}

impl EvolutionError {
    /// Stable code for durable error rows / logs.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidManifestHash { .. } => "invalid_manifest_hash",
            Self::InvalidComponentChange(_) => "invalid_component_change",
            Self::InvalidBudget(_) => "invalid_budget",
            Self::InvalidProbe(_) => "invalid_probe",
            Self::SameRevisionProbe { .. } => "same_revision_probe",
            Self::ProductionProbeForbidden { .. } => "production_probe_forbidden",
            Self::ProbeIdentityMismatch { .. } => "probe_identity_mismatch",
            Self::InvalidRevisionTransition { .. } => "invalid_revision_transition",
            Self::MissingEvidence(_) => "missing_evidence",
            Self::InvalidCandidate(_) => "invalid_candidate",
            Self::InvalidComposition(_) => "invalid_composition",
            Self::InvalidExperience(_) => "invalid_experience",
        }
    }
}

impl fmt::Display for EvolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidManifestHash { expected, actual } => write!(
                f,
                "manifest hash mismatch: stored {expected}, computed {actual}"
            ),
            Self::InvalidComponentChange(message) => write!(f, "invalid component change: {message}"),
            Self::InvalidBudget(message) => write!(f, "invalid evolution budget: {message}"),
            Self::InvalidProbe(message) => write!(f, "invalid probe: {message}"),
            Self::SameRevisionProbe { revision } => write!(
                f,
                "probe base and candidate are the same revision: {revision}"
            ),
            Self::ProductionProbeForbidden { environment_class } => write!(
                f,
                "probe may not target the {} environment",
                environment_class.id()
            ),
            Self::ProbeIdentityMismatch {
                reference,
                candidate,
            } => write!(
                f,
                "probe identities differ; contrast invalid (reference: {reference}; candidate: {candidate})"
            ),
            Self::InvalidRevisionTransition { from, to } => {
                write!(f, "revision may not transition from {from} to {to}")
            }
            Self::MissingEvidence(message) => write!(f, "missing evidence: {message}"),
            Self::InvalidCandidate(message) => write!(f, "invalid candidate: {message}"),
            Self::InvalidComposition(message) => write!(f, "invalid composition: {message}"),
            Self::InvalidExperience(message) => write!(f, "invalid experience: {message}"),
        }
    }
}

impl std::error::Error for EvolutionError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt(content: &str) -> Component {
        Component::green(ComponentKind::SystemPrompt, content, "test prompt")
    }

    fn manifest_with(component: Component) -> HarnessManifest {
        HarnessManifest::empty().with_component(component)
    }

    fn base_revision(manifest: HarnessManifest) -> HarnessRevision {
        HarnessRevision::draft("base-1", HarnessDomain::RustBackend, manifest, "owner")
    }

    #[test]
    fn component_kind_ids_are_stable() {
        assert_eq!(ComponentKind::ALL.len(), 8);
        let ids: Vec<&str> = ComponentKind::ALL.iter().map(|kind| kind.id()).collect();
        assert_eq!(
            ids,
            vec![
                "system_prompt",
                "guardrails",
                "memory",
                "tool_guidance",
                "subagents",
                "skills",
                "scripts",
                "plugins",
            ]
        );
        assert!(ComponentKind::Scripts.minimum_authority().is_red());
        assert!(ComponentKind::Plugins.minimum_authority().is_red());
        assert!(ComponentKind::SystemPrompt.minimum_authority().is_green());
    }

    #[test]
    fn authority_classes_rank_and_max() {
        assert!(AuthorityClass::Green.rank() < AuthorityClass::Yellow.rank());
        assert!(AuthorityClass::Yellow.rank() < AuthorityClass::Red.rank());
        assert_eq!(
            AuthorityClass::Green.max(AuthorityClass::Yellow),
            AuthorityClass::Yellow
        );
        assert_eq!(
            AuthorityClass::Red.max(AuthorityClass::Green),
            AuthorityClass::Red
        );
    }

    #[test]
    fn manifest_hash_is_stable_across_insertion_order() {
        let first = Component::green(ComponentKind::SystemPrompt, "be useful", "prompt");
        let second = Component::yellow(ComponentKind::ToolGuidance, "use ripwire", "tools");
        let a = HarnessManifest::empty()
            .with_component(first.clone())
            .with_component(second.clone());
        let b = HarnessManifest::empty()
            .with_component(second)
            .with_component(first);
        assert_eq!(a.manifest_hash(), b.manifest_hash());
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    }

    #[test]
    fn manifest_hash_changes_with_content() {
        let before = manifest_with(prompt("be useful"));
        let after = manifest_with(prompt("be useful and concise"));
        assert_ne!(before.manifest_hash(), after.manifest_hash());
    }

    #[test]
    fn manifest_serialization_roundtrips() {
        let manifest = HarnessManifest::empty()
            .with_component(prompt("alpha"))
            .with_component(Component::yellow(
                ComponentKind::Skills,
                "load rust-testing",
                "skills",
            ));
        let json = serde_json::to_string(&manifest).expect("manifest serializes");
        let restored: HarnessManifest = serde_json::from_str(&json).expect("manifest restores");
        assert_eq!(restored, manifest);
        assert_eq!(restored.manifest_hash(), manifest.manifest_hash());
    }

    #[test]
    fn manifest_construction_never_mutates_the_original() {
        let original = manifest_with(prompt("baseline"));
        let original_hash = original.manifest_hash();
        let extended = original.clone().with_component(Component::green(
            ComponentKind::Memory,
            "note",
            "memory",
        ));
        assert_eq!(original.len(), 1);
        assert_eq!(original.manifest_hash(), original_hash);
        assert_eq!(extended.len(), 2);
        assert_ne!(extended.manifest_hash(), original_hash);

        assert_eq!(extended.len(), 2);
        let removed = extended.without_component(ComponentKind::Memory);
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn manifest_apply_changes_sets_and_removes() {
        let manifest = HarnessManifest::empty()
            .with_component(prompt("baseline"))
            .with_component(Component::green(ComponentKind::Memory, "old", "memory"));
        let changes = vec![
            ComponentChange::set(prompt("improved"), "clearer instructions"),
            ComponentChange::remove(ComponentKind::Memory, "stale memory"),
        ];
        let updated = manifest
            .apply_changes(&changes)
            .expect("well-formed changes apply");
        assert_eq!(
            updated
                .get(ComponentKind::SystemPrompt)
                .map(|c| c.content.as_str()),
            Some("improved")
        );
        assert!(updated.get(ComponentKind::Memory).is_none());
        assert_eq!(
            manifest
                .get(ComponentKind::SystemPrompt)
                .map(|c| c.content.as_str()),
            Some("baseline"),
            "apply_changes must not mutate the source manifest"
        );
    }

    #[test]
    fn manifest_apply_changes_rejects_malformed_change() {
        let malformed = ComponentChange {
            kind: ComponentKind::SystemPrompt,
            action: ChangeAction::Set,
            component: Some(Component::green(
                ComponentKind::Memory,
                "wrong kind",
                "test",
            )),
            rationale: "test".to_string(),
        };
        assert!(HarnessManifest::empty()
            .apply_changes(&[malformed])
            .is_err());
        let missing = ComponentChange {
            kind: ComponentKind::SystemPrompt,
            action: ChangeAction::Set,
            component: None,
            rationale: "test".to_string(),
        };
        assert!(HarnessManifest::empty().apply_changes(&[missing]).is_err());
    }

    #[test]
    fn manifest_component_authority_uses_kind_floor() {
        let manifest = HarnessManifest::empty()
            .with_component(Component::new(
                ComponentKind::Plugins,
                AuthorityClass::Green,
                "plugin config",
                "plugins",
            ))
            .with_component(Component::green(ComponentKind::Memory, "note", "memory"));
        assert_eq!(
            manifest.component_authority(ComponentKind::Plugins),
            Some(AuthorityClass::Green)
        );
        assert_eq!(
            manifest.effective_authority(ComponentKind::Plugins),
            Some(AuthorityClass::Red),
            "Plugins are RED by floor even if a proposer declares Green"
        );
        assert_eq!(
            manifest.highest_effective_authority(),
            Some(AuthorityClass::Red)
        );
    }

    #[test]
    fn harness_domain_ids_roundtrip() {
        assert_eq!(HarnessDomain::ALL.len(), 9);
        for domain in HarnessDomain::ALL {
            assert_eq!(HarnessDomain::from_id(domain.id()), Some(domain));
        }
        assert_eq!(HarnessDomain::from_id("nope"), None);
        assert_eq!(HarnessDomain::ForgeWorkflow.id(), "forge-workflow");
    }

    #[test]
    fn revision_status_transition_table_is_a_closed_lifecycle() {
        assert!(RevisionStatus::Draft.can_transition_to(RevisionStatus::Proposed));
        assert!(RevisionStatus::Adoptable.can_transition_to(RevisionStatus::Adopted));
        assert!(RevisionStatus::Adopted.can_transition_to(RevisionStatus::RolledBack));
        assert!(RevisionStatus::Superseded.can_transition_to(RevisionStatus::Adopted));
        assert!(!RevisionStatus::Draft.can_transition_to(RevisionStatus::Adopted));
        assert!(!RevisionStatus::Rejected.can_transition_to(RevisionStatus::Adopted));
        assert!(RevisionStatus::Rejected.is_terminal());
        assert!(RevisionStatus::Adopted.is_active());
    }

    #[test]
    fn revision_integrity_and_hash_are_enforced() {
        let manifest = manifest_with(prompt("baseline"));
        let revision = base_revision(manifest.clone());
        assert_eq!(revision.manifest_hash, manifest.manifest_hash());
        revision
            .verify_integrity()
            .expect("fresh revision is valid");

        let tampered = revision
            .clone()
            .with_manifest(manifest_with(prompt("silently changed")));
        tampered.verify_integrity().expect("with_manifest rehashes");
        assert!(revision.verify_against(&tampered.manifest_hash).is_err());

        let transitioned = revision
            .clone()
            .try_transition(RevisionStatus::Proposed)
            .expect("draft -> proposed is legal");
        assert_eq!(transitioned.status, RevisionStatus::Proposed);
        assert!(revision.try_transition(RevisionStatus::Adopted).is_err());
    }

    #[test]
    fn revision_successor_and_from_changes_keep_the_parent() {
        let base = base_revision(manifest_with(prompt("baseline")));
        let changes = vec![ComponentChange::set(prompt("improved"), "clearer")];
        let candidate = HarnessRevision::from_changes(&base, "cand-1", &changes, "architect")
            .expect("candidate is base + changes");
        assert_eq!(candidate.parent_revision_id.as_deref(), Some("base-1"));
        assert_eq!(candidate.domain, base.domain);
        assert_eq!(candidate.status, RevisionStatus::Draft);
        assert_eq!(
            candidate.manifest,
            base.manifest.apply_changes(&changes).expect("applies")
        );
        assert_ne!(candidate.manifest_hash, base.manifest_hash);
    }

    #[test]
    fn component_change_shape_validation() {
        let ok = ComponentChange::set(prompt("x"), "why");
        ok.validate_shape().expect("valid set");
        let ok_remove = ComponentChange::remove(ComponentKind::Memory, "why");
        ok_remove.validate_shape().expect("valid remove");
        assert!(ComponentChange::set(prompt("x"), "  ")
            .validate_shape()
            .is_err());

        let remove_with_component = ComponentChange {
            kind: ComponentKind::Memory,
            action: ChangeAction::Remove,
            component: Some(Component::green(ComponentKind::Memory, "x", "y")),
            rationale: "why".to_string(),
        };
        assert!(remove_with_component.validate_shape().is_err());
    }

    #[test]
    fn candidate_validation_requires_changes() {
        let base = base_revision(manifest_with(prompt("baseline")));
        let empty = Candidate::new("c1", base.id.clone(), ProposalKind::Invariant, Vec::new());
        assert!(empty.validate().is_err());
        let candidate = Candidate::new(
            "c1",
            base.id.clone(),
            ProposalKind::Invariant,
            vec![ComponentChange::set(prompt("x"), "why")],
        );
        candidate.validate().expect("well-formed candidate");
        assert_eq!(candidate.target_kinds(), vec![ComponentKind::SystemPrompt]);
    }

    #[test]
    fn budget_and_candidate_set_express_k_but_default_to_one() {
        let budget = EvolutionBudget::default();
        assert!(budget.is_k1());
        budget.validate().expect("k1 is valid");
        assert!(EvolutionBudget::k1().reference_runs() == 1);

        let mut set = CandidateSet::k1();
        assert!(set.is_empty());
        let base = base_revision(manifest_with(prompt("baseline")));
        set.invariant.push(Candidate::new(
            "i1",
            base.id.clone(),
            ProposalKind::Invariant,
            vec![ComponentChange::set(prompt("x"), "why")],
        ));
        set.domain.push(Candidate::new(
            "d1",
            base.id,
            ProposalKind::Domain,
            vec![ComponentChange::set(prompt("y"), "why")],
        ));
        set.validate()
            .expect("k1 set with one candidate per scope is valid");
        assert!(set.within_budget());

        let later = EvolutionBudget {
            k: 2,
            max_steps: 2,
            max_probes: 4,
        };
        later.validate().expect("k=2 budget is expressible");
        assert!(!later.is_k1());
    }

    #[test]
    fn probe_identity_matching_production_and_same_revision_refusals() {
        let spec = ProbeSpec::new("task-1", "base", "cand")
            .with_model("deepseek-flash")
            .with_executor("dsh")
            .with_tools(vec!["edit".into(), "read".into(), "read".into()])
            .with_input_dataset("phase3-scorecard")
            .with_test_plan("cargo test -p forge")
            .with_replay_id("replay-1");
        spec.validate_identity().expect("different revisions");
        spec.validate_complete().expect("complete pins");

        let mut other = spec.identity();
        other.tools = vec!["read".into(), "edit".into()];
        assert!(spec.identity().matches(&other), "tool pin is a set");

        other.model = "other-model".into();
        assert!(!spec.identity().matches(&other));

        let same = ProbeSpec::new("task-1", "base", "base");
        assert!(same.validate_identity().is_err());

        let production = spec
            .clone()
            .with_environment_class(EnvironmentClass::Production);
        assert!(matches!(
            production.validate_identity(),
            Err(EvolutionError::ProductionProbeForbidden { .. })
        ));

        let incomplete = ProbeSpec::new("task-1", "base", "cand");
        assert!(incomplete.validate_complete().is_err());
    }

    #[test]
    fn behavior_dimensions_are_honest_about_missing_cost() {
        let reference = BehaviorEvidence {
            success: true,
            tests_failed: 1,
            qa: QaVerdict::Pass,
            steps: 10,
            tokens_in: 100,
            tokens_out: 50,
            cost_micros: None,
            ..BehaviorEvidence::new("e1", "p1")
        };
        let candidate = BehaviorEvidence {
            success: true,
            tests_failed: 0,
            qa: QaVerdict::Pass,
            steps: 8,
            tokens_in: 90,
            tokens_out: 40,
            cost_micros: None,
            ..BehaviorEvidence::new("e2", "p2")
        };
        assert_eq!(
            BehaviorDimension::TestsFailed.direction(&reference, &candidate),
            ContrastDirection::Improved
        );
        assert_eq!(
            BehaviorDimension::Steps.direction(&reference, &candidate),
            ContrastDirection::Improved
        );
        assert_eq!(
            BehaviorDimension::Tokens.direction(&reference, &candidate),
            ContrastDirection::Improved
        );
        assert_eq!(
            BehaviorDimension::Cost.direction(&reference, &candidate),
            ContrastDirection::Unknown,
            "unpriced cost is not unchanged and not zero"
        );
        assert_eq!(candidate.tokens_total(), 130);
        assert!(candidate.is_behaviorally_clean());

        let mut failing = candidate.clone();
        failing.qa = QaVerdict::NotRun;
        assert!(!failing.is_behaviorally_clean());
    }

    #[test]
    fn contrast_report_rejects_identity_mismatch() {
        let reference = BehaviorEvidence::new("e1", "p1");
        let candidate = BehaviorEvidence::new("e2", "p2");
        let identity = ProbeSpec::new("task", "base", "cand")
            .with_model("m")
            .with_input_dataset("d")
            .with_test_plan("t")
            .with_replay_id("r")
            .identity();
        let mut other_identity = identity.clone();
        other_identity.executor = "codex".into();

        let contrast = DimensionContrast::from_evidence(
            BehaviorDimension::TestsFailed,
            &reference,
            &candidate,
            "tests",
        );
        let report = ContrastReport {
            id: "contrast-1".into(),
            base_revision: "base".into(),
            candidate_revision: "cand".into(),
            reference_identity: identity.clone(),
            candidate_identity: identity,
            dimensions: vec![contrast.clone()],
            summary: "same".into(),
            evidence_refs: vec!["ev-1".into()],
        };
        report.validate().expect("matched identities contrast");
        assert_eq!(
            report.improvements(),
            Vec::<BehaviorDimension>::new(),
            "equal values are unchanged"
        );

        let mismatched = ContrastReport {
            candidate_identity: other_identity,
            ..report
        };
        assert!(matches!(
            mismatched.validate(),
            Err(EvolutionError::ProbeIdentityMismatch { .. })
        ));
    }

    #[test]
    fn composition_may_keep_the_base_but_must_link_evidence() {
        let base = base_revision(manifest_with(prompt("baseline")));
        let keep = ComponentDecisionRecord::new(
            ComponentKind::SystemPrompt,
            ComponentDecision::Keep,
            "no measured improvement",
        );
        let keep_base = CompositionDecision {
            id: "comp-1".into(),
            base_revision: base.id.clone(),
            candidates: vec!["cand-1".into()],
            contrast_ids: vec!["contrast-1".into()],
            decisions: vec![keep],
            successor_manifest_hash: base.manifest_hash.clone(),
            summary: "keep base".into(),
            evidence_refs: vec!["ev-1".into()],
        };
        keep_base
            .validate()
            .expect("keeping the unchanged base is legal");

        let adopt_without_evidence = CompositionDecision {
            decisions: vec![ComponentDecisionRecord::new(
                ComponentKind::SystemPrompt,
                ComponentDecision::Adopt,
                "looks better",
            )],
            evidence_refs: Vec::new(),
            ..keep_base
        };
        assert!(adopt_without_evidence.validate().is_err());
    }

    #[test]
    fn validation_evidence_separates_failed_from_not_run() {
        let pending = ValidationEvidence::pending();
        assert!(!pending.is_clean());
        assert!(!pending.has_run());
        assert!(!pending.is_complete());

        let failed = ValidationEvidence {
            regression_rails: GateStatus::Failed,
            ..pending
        };
        assert_eq!(failed.failed_gates(), vec!["regression_rails"]);
        assert!(!failed.is_complete());

        let clean = ValidationEvidence::all_passed(vec!["val-1".into()]);
        assert!(clean.is_clean());
        assert!(clean.is_complete());
        assert!(clean.failed_gates().is_empty());

        let unlinked = ValidationEvidence::all_passed(Vec::new());
        assert!(!unlinked.is_clean());
    }

    #[test]
    fn adoption_decision_tracks_its_authority() {
        let policy = AdoptionDecision::new(
            "adopt-1",
            HarnessDomain::RustBackend,
            "rev-2",
            AdoptionVerdict::Adopt,
            "policy",
            "clean green",
        )
        .with_evidence_refs(vec!["ev-1".into()]);
        assert!(policy.is_automatic());

        let council = AdoptionDecision::new(
            "adopt-2",
            HarnessDomain::RustBackend,
            "rev-2",
            AdoptionVerdict::Council,
            "policy",
            "yellow surface",
        );
        assert!(!council.is_automatic());

        let owner = AdoptionDecision::new(
            "adopt-3",
            HarnessDomain::SecurityEntitlements,
            "rev-3",
            AdoptionVerdict::Chris,
            "policy",
            "red surface",
        );
        assert!(owner.requires_owner());
    }

    #[test]
    fn experience_is_scoped_and_requires_complete_validation() {
        let scope = ExperienceScope::k1(HarnessDomain::RustBackend);
        assert!(scope == ExperienceScope::k1(HarnessDomain::RustBackend));
        assert_ne!(scope, ExperienceScope::k1(HarnessDomain::DatabaseNeon));

        let now = Utc::now();
        let record = ExperienceRecord {
            id: "exp-1".into(),
            scope: scope.clone(),
            harness_revision: "rev-2".into(),
            manifest_hash: "abc".into(),
            lesson: "name the failing test before editing".into(),
            applicability: vec!["rust-backend".into()],
            counterevidence: Vec::new(),
            confidence: 70,
            contrast_id: "contrast-1".into(),
            validation: ValidationEvidence::all_passed(vec!["val-1".into()]),
            evidence_refs: vec!["ev-1".into()],
            created_at: now,
            expires_at: None,
        };
        record.validate().expect("complete evidence inserts");
        assert!(record.is_visible_at(now));
        assert!(!record.is_expired(now));

        let incomplete = ExperienceRecord {
            validation: ValidationEvidence::pending(),
            ..record.clone()
        };
        assert!(incomplete.validate().is_err());

        let no_contrast = ExperienceRecord {
            contrast_id: String::new(),
            ..record
        };
        assert!(no_contrast.validate().is_err());

        let expired = ExperienceRecord {
            id: "exp-2".into(),
            scope: ExperienceScope::k1(HarnessDomain::RustBackend),
            harness_revision: "rev".into(),
            manifest_hash: "abc".into(),
            lesson: "lesson".into(),
            applicability: vec!["rust-backend".into()],
            counterevidence: Vec::new(),
            confidence: 10,
            contrast_id: "contrast-2".into(),
            validation: ValidationEvidence::all_passed(vec!["val-2".into()]),
            evidence_refs: vec!["ev-2".into()],
            created_at: now,
            expires_at: Some(now),
        };
        assert!(expired.is_expired(now));
        expired
            .validate()
            .expect("an expired-but-complete lesson is still a valid record");
    }

    #[test]
    fn evolution_error_codes_are_stable() {
        let error = EvolutionError::SameRevisionProbe {
            revision: "rev-1".into(),
        };
        assert_eq!(error.code(), "same_revision_probe");
        assert!(error.to_string().contains("rev-1"));
        let mismatch = EvolutionError::ProbeIdentityMismatch {
            reference: "a".into(),
            candidate: "b".into(),
        };
        assert!(mismatch.to_string().contains("contrast invalid"));
    }
}
