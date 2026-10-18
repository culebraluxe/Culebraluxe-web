//! Experience — the per-domain library that lets Harness² learn across experiments.
//!
//! Harness²'s second recursion: after an experiment is adopted, the adopted harness and the
//! evidence-supported lesson are written into a library that later experiments in the same
//! conditions read before proposing. The library is deliberately narrow:
//!
//! * **scope, not just domain.** Lessons are separated by domain *and* executor, recursion mode,
//!   budget and seed. A lesson learned with `k = 1` under parallel recursion is not offered to a
//!   sequential `k = 3` run — the conditions that produced it are part of its identity.
//! * **no fabricated lessons.** An insert needs the behavioral contrast that produced it and a
//!   *complete* Forge validation (every gate decided) whose embedded evidence matches the record.
//!   An unfinished experiment updates nothing.
//! * **task order.** A record is only visible at or after its creation time, so a task reads only
//!   earlier entries in its stream; expired records stop guiding retrieval.
//! * **bounded retrieval.** Retrieval is relevance-filtered against the target's applicability
//!   terms, ranked by relevance then confidence then recency, and hard-capped at
//!   [`EXPERIENCE_RETRIEVAL_CAP`].
//! * **counterevidence survives.** A lesson keeps the evidence against it, so retrieval can surface
//!   a caveat rather than a confident half-truth.
//!
//! v1 is an in-memory store. The durable mirror is the Lead's migration/table work; this module
//! holds the rules, not the connection.

use chrono::{DateTime, Utc};

use super::contrast::{self, ContrastVerdict};
use super::model::{
    ContrastReport, EvolutionError, ExperienceRecord, ExperienceScope, ValidationEvidence,
};

/// The hard cap on lessons one retrieval may return. A task should be guided by a handful of
/// relevant lessons, not by the whole library.
pub const EXPERIENCE_RETRIEVAL_CAP: usize = 5;

/// The most applicability terms one lesson may carry. Bounds matching cost and keeps a lesson a
/// lesson rather than a document.
pub const EXPERIENCE_MAX_APPLICABILITY: usize = 16;

/// A lesson must believe something. Confidence zero is a placeholder, not a lesson.
pub const EXPERIENCE_MIN_CONFIDENCE: u8 = 1;

/// Result of an insert. Duplicate ids are idempotent, not silently overwritten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertOutcome {
    Inserted,
    AlreadyPresent,
}

/// One retrieval query: the exact scope to read, the target's terms, and a requested limit that is
/// clamped to [`EXPERIENCE_RETRIEVAL_CAP`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperienceQuery {
    pub scope: ExperienceScope,
    pub terms: Vec<String>,
    pub limit: usize,
}

impl ExperienceQuery {
    pub fn new(scope: ExperienceScope, terms: Vec<String>) -> Self {
        Self {
            scope,
            terms,
            limit: EXPERIENCE_RETRIEVAL_CAP,
        }
    }

    /// The effective, bounded limit. Never zero: a caller asking for nothing still gets the most
    /// relevant lesson rather than the library silently answering with an empty success.
    pub fn capped_limit(&self) -> usize {
        self.limit.clamp(1, EXPERIENCE_RETRIEVAL_CAP)
    }
}

/// An in-memory, multi-domain experience library.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExperienceLibrary {
    entries: Vec<ExperienceRecord>,
}

impl ExperienceLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a library from already-validated records (used when rehydrating the durable mirror).
    /// Duplicates are kept in first-seen order; callers rehydrating a table should pass distinct
    /// ids.
    pub fn from_records(records: Vec<ExperienceRecord>) -> Self {
        Self { entries: records }
    }

    pub fn records(&self) -> &[ExperienceRecord] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Insert one lesson, but only when the experiment behind it is complete and evidence-linked.
    ///
    /// Refused unless:
    /// * the record itself is well formed ([`ExperienceRecord::validate`]);
    /// * the supporting contrast is valid and comparable (identity pins match);
    /// * the record cites that contrast (`contrast_id`) and the revision that contrast measured;
    /// * the embedded validation equals the supplied validation, is complete (no `NotRun` gate) and
    ///   has actually run;
    /// * the lesson links at least one evidence ref of the supporting contrast;
    /// * confidence is at least [`EXPERIENCE_MIN_CONFIDENCE`], applicability is non-empty and
    ///   bounded, and any expiry is after creation.
    pub fn insert(
        &mut self,
        record: ExperienceRecord,
        contrast: &ContrastReport,
        validation: &ValidationEvidence,
    ) -> Result<InsertOutcome, EvolutionError> {
        record.validate()?;
        contrast.validate()?;

        if record.contrast_id != contrast.id {
            return Err(EvolutionError::InvalidExperience(format!(
                "lesson cites contrast {:?} but the supporting contrast is {:?}",
                record.contrast_id, contrast.id
            )));
        }
        // The lesson may be about an adopted successor composed from several candidates, so its
        // revision need not be the single candidate revision this contrast measured. The linkage
        // that must hold is the contrast id and the shared evidence refs below.
        if record.validation != *validation {
            return Err(EvolutionError::InvalidExperience(
                "embedded validation differs from the supporting validation".into(),
            ));
        }
        if !validation.is_complete() {
            return Err(EvolutionError::InvalidExperience(format!(
                "supporting validation is incomplete: {:?}",
                validation.missing_gates()
            )));
        }
        if !validation.has_run() {
            return Err(EvolutionError::InvalidExperience(
                "supporting validation never ran".into(),
            ));
        }
        if validation.evidence_refs.is_empty() {
            return Err(EvolutionError::InvalidExperience(
                "supporting validation has no durable evidence refs".into(),
            ));
        }
        if !matches!(
            contrast::verdict(contrast),
            ContrastVerdict::Improved | ContrastVerdict::Regressed
        ) {
            return Err(EvolutionError::InvalidExperience(
                "lesson needs a behavioral contrast that actually improved or regressed something"
                    .into(),
            ));
        }
        if record.confidence < EXPERIENCE_MIN_CONFIDENCE {
            return Err(EvolutionError::InvalidExperience(
                "a lesson must carry a non-zero confidence".into(),
            ));
        }
        if record.applicability.is_empty() {
            return Err(EvolutionError::InvalidExperience(
                "a lesson must say where it applies".into(),
            ));
        }
        if record.applicability.len() > EXPERIENCE_MAX_APPLICABILITY {
            return Err(EvolutionError::InvalidExperience(format!(
                "lesson has {} applicability terms; the cap is {}",
                record.applicability.len(),
                EXPERIENCE_MAX_APPLICABILITY
            )));
        }
        if record
            .applicability
            .iter()
            .any(|term| term.trim().is_empty())
        {
            return Err(EvolutionError::InvalidExperience(
                "lesson has a blank applicability term".into(),
            ));
        }
        if !record
            .evidence_refs
            .iter()
            .any(|reference| contrast.evidence_refs.contains(reference))
        {
            return Err(EvolutionError::InvalidExperience(
                "lesson does not link any evidence ref of its supporting contrast".into(),
            ));
        }
        if let Some(expires_at) = record.expires_at {
            if expires_at <= record.created_at {
                return Err(EvolutionError::InvalidExperience(
                    "lesson expires at or before its creation".into(),
                ));
            }
        }

        if self
            .entries
            .iter()
            .any(|existing| existing.id == record.id)
        {
            return Ok(InsertOutcome::AlreadyPresent);
        }
        self.entries.push(record);
        Ok(InsertOutcome::Inserted)
    }

    /// Retrieval for one task: only records in the **exact same scope** that are visible at `now`,
    /// relevant to the target's terms, ranked and capped.
    ///
    /// An empty term list means "everything in this scope" (still capped and visibility-filtered);
    /// a non-empty list keeps only lessons whose applicability contains at least one term.
    pub fn retrieve(
        &self,
        query: &ExperienceQuery,
        now: DateTime<Utc>,
    ) -> Vec<&ExperienceRecord> {
        let terms = canonical_terms(&query.terms);
        let mut matches: Vec<(&ExperienceRecord, usize)> = self
            .entries
            .iter()
            .filter(|record| record.scope == query.scope)
            .filter(|record| record.is_visible_at(now))
            .filter_map(|record| {
                let score = relevance(record, &terms);
                if terms.is_empty() || score > 0 {
                    Some((record, score))
                } else {
                    None
                }
            })
            .collect();

        matches.sort_by(|(left_record, left_score), (right_record, right_score)| {
            right_score
                .cmp(left_score)
                .then(right_record.confidence.cmp(&left_record.confidence))
                .then(right_record.created_at.cmp(&left_record.created_at))
                .then(left_record.id.cmp(&right_record.id))
        });

        matches
            .into_iter()
            .take(query.capped_limit())
            .map(|(record, _)| record)
            .collect()
    }

    /// Every visible record in one scope, newest first. Diagnostic surface; retrieval should use
    /// [`ExperienceLibrary::retrieve`].
    pub fn visible_in_scope(
        &self,
        scope: &ExperienceScope,
        now: DateTime<Utc>,
    ) -> Vec<&ExperienceRecord> {
        let mut records: Vec<&ExperienceRecord> = self
            .entries
            .iter()
            .filter(|record| &record.scope == scope && record.is_visible_at(now))
            .collect();
        records.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then(left.id.cmp(&right.id))
        });
        records
    }

    pub fn count_in_scope(&self, scope: &ExperienceScope) -> usize {
        self.entries
            .iter()
            .filter(|record| &record.scope == scope)
            .count()
    }

    /// Drop expired records. Returns how many were removed. Expiry is also enforced at retrieval
    /// time; this is for a durable store's housekeeping.
    pub fn expire(&mut self, now: DateTime<Utc>) -> usize {
        let before = self.entries.len();
        self.entries.retain(|record| !record.is_expired(now));
        before - self.entries.len()
    }
}

fn canonical_terms(terms: &[String]) -> Vec<String> {
    let mut canonical: Vec<String> = terms
        .iter()
        .map(|term| term.trim().to_ascii_lowercase())
        .filter(|term| !term.is_empty())
        .collect();
    canonical.sort();
    canonical.dedup();
    canonical
}

/// Relevance = how many of the target's terms the lesson's applicability mentions. Case-insensitive
/// substring matching is enough here: applicability terms are authored keywords, not prose.
fn relevance(record: &ExperienceRecord, terms: &[String]) -> usize {
    if terms.is_empty() {
        return 0;
    }
    let mut score = 0;
    for applicability in &record.applicability {
        let applicability = applicability.to_ascii_lowercase();
        for term in terms {
            if applicability.contains(term.as_str()) {
                score += 1;
            }
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::harness_evolution::contrast::contrast;
    use crate::engine::harness_evolution::model::{
        BehaviorEvidence, EnvironmentClass, EvolutionBudget, EvolutionMode, GateStatus,
        HarnessDomain, ProbeIdentity, QaVerdict,
    };

    fn identity() -> ProbeIdentity {
        ProbeIdentity {
            model: "flash".into(),
            model_policy: "default".into(),
            executor: "opencode".into(),
            tools: vec!["read".into()],
            environment_class: EnvironmentClass::Development,
            input_dataset: "task-6-fixture".into(),
        }
    }

    fn evidence(id: &str, success: bool, steps: u64) -> BehaviorEvidence {
        let mut evidence = BehaviorEvidence::new(format!("evidence-{id}"), format!("probe-{id}"));
        evidence.success = success;
        evidence.tests_passed = 5;
        evidence.qa = QaVerdict::Pass;
        evidence.steps = steps;
        evidence.evidence_refs = vec![format!("receipt:{id}")];
        evidence
    }

    fn supporting_contrast() -> ContrastReport {
        contrast(
            "contrast-1",
            "base-1",
            "candidate-rev-1",
            &evidence("ref", true, 10),
            &evidence("cand", true, 5),
            &identity(),
            &identity(),
        )
        .unwrap()
    }

    fn scope(domain: HarnessDomain) -> ExperienceScope {
        ExperienceScope::k1(domain)
    }

    fn passed_validation() -> ValidationEvidence {
        ValidationEvidence::all_passed(vec!["rails:330".into(), "replay:42".into()])
    }

    fn record(
        id: &str,
        scope: ExperienceScope,
        contrast: &ContrastReport,
        validation: &ValidationEvidence,
    ) -> ExperienceRecord {
        ExperienceRecord {
            id: id.into(),
            scope,
            harness_revision: contrast.candidate_revision.clone(),
            manifest_hash: "manifest-hash-1".into(),
            lesson: "domain constraints stated before the first edit cut rework".into(),
            applicability: vec!["rust-backend".into(), "smith".into(), "rework".into()],
            counterevidence: vec!["one fixture was flaky".into()],
            confidence: 70,
            contrast_id: contrast.id.clone(),
            validation: validation.clone(),
            evidence_refs: vec!["receipt:cand".into()],
            created_at: Utc::now(),
            expires_at: None,
        }
    }

    fn library_with_one_lesson() -> (ExperienceLibrary, ExperienceScope) {
        let contrast = supporting_contrast();
        let validation = passed_validation();
        let scope = scope(HarnessDomain::RustBackend);
        let mut library = ExperienceLibrary::new();
        assert_eq!(
            library
                .insert(record("lesson-1", scope.clone(), &contrast, &validation), &contrast, &validation)
                .unwrap(),
            InsertOutcome::Inserted
        );
        (library, scope)
    }

    #[test]
    fn an_evidence_backed_lesson_is_stored() {
        let (library, _) = library_with_one_lesson();
        assert_eq!(library.len(), 1);
        let stored = &library.records()[0];
        assert_eq!(stored.counterevidence, vec!["one fixture was flaky".to_string()]);
        assert_eq!(stored.confidence, 70);
    }

    #[test]
    fn inserting_the_same_id_is_idempotent() {
        let contrast = supporting_contrast();
        let validation = passed_validation();
        let scope = scope(HarnessDomain::RustBackend);
        let mut library = ExperienceLibrary::new();
        library
            .insert(record("lesson-1", scope.clone(), &contrast, &validation), &contrast, &validation)
            .unwrap();
        assert_eq!(
            library
                .insert(record("lesson-1", scope, &contrast, &validation), &contrast, &validation)
                .unwrap(),
            InsertOutcome::AlreadyPresent
        );
        assert_eq!(library.len(), 1);
    }

    #[test]
    fn an_incomplete_experiment_cannot_update_the_library() {
        let contrast = supporting_contrast();
        let pending = ValidationEvidence::pending();
        let scope = scope(HarnessDomain::RustBackend);
        let mut library = ExperienceLibrary::new();
        let error = library
            .insert(record("lesson-1", scope, &contrast, &pending), &contrast, &pending)
            .unwrap_err();
        assert_eq!(error.code(), "invalid_experience");
        assert!(library.is_empty());
    }

    #[test]
    fn incomplete_supporting_validation_is_refused_even_when_the_record_claims_complete() {
        let contrast = supporting_contrast();
        let complete = passed_validation();
        let scope = scope(HarnessDomain::RustBackend);
        let mut library = ExperienceLibrary::new();
        let mut lesson = record("lesson-1", scope, &contrast, &complete);
        // The record carries a complete validation, but the caller passes a partial one.
        let partial = ValidationEvidence {
            static_policy: GateStatus::Passed,
            ..ValidationEvidence::pending()
        };
        let error = library
            .insert(lesson.clone(), &contrast, &partial)
            .unwrap_err();
        assert!(error.to_string().contains("differs"));
        lesson.validation = ValidationEvidence::pending();
        assert_eq!(
            library
                .insert(lesson, &contrast, &ValidationEvidence::pending())
                .unwrap_err()
                .code(),
            "invalid_experience"
        );
    }

    #[test]
    fn a_lesson_must_cite_its_supporting_contrast() {
        let contrast = supporting_contrast();
        let validation = passed_validation();
        let scope = scope(HarnessDomain::RustBackend);
        let mut lesson = record("lesson-1", scope, &contrast, &validation);
        lesson.contrast_id = "some-other-contrast".into();
        let mut library = ExperienceLibrary::new();
        let error = library
            .insert(lesson, &contrast, &validation)
            .unwrap_err();
        assert!(error.to_string().contains("cites contrast"));
    }

    #[test]
    fn a_lesson_must_link_the_contrast_evidence() {
        let contrast = supporting_contrast();
        let validation = passed_validation();
        let scope = scope(HarnessDomain::RustBackend);
        let mut lesson = record("lesson-1", scope, &contrast, &validation);
        lesson.evidence_refs = vec!["unrelated:1".into()];
        let mut library = ExperienceLibrary::new();
        let error = library
            .insert(lesson, &contrast, &validation)
            .unwrap_err();
        assert!(error.to_string().contains("evidence ref"));
    }

    #[test]
    fn zero_confidence_or_blank_applicability_is_refused() {
        let contrast = supporting_contrast();
        let validation = passed_validation();
        let scope = scope(HarnessDomain::RustBackend);
        let mut library = ExperienceLibrary::new();
        let mut no_confidence = record("lesson-1", scope.clone(), &contrast, &validation);
        no_confidence.confidence = 0;
        assert_eq!(
            library
                .insert(no_confidence, &contrast, &validation)
                .unwrap_err()
                .code(),
            "invalid_experience"
        );
        let mut blank = record("lesson-2", scope, &contrast, &validation);
        blank.applicability = vec![" ".into()];
        assert_eq!(
            library
                .insert(blank, &contrast, &validation)
                .unwrap_err()
                .code(),
            "invalid_experience"
        );
    }

    #[test]
    fn expiry_before_creation_is_refused() {
        let contrast = supporting_contrast();
        let validation = passed_validation();
        let scope = scope(HarnessDomain::RustBackend);
        let mut lesson = record("lesson-1", scope, &contrast, &validation);
        lesson.expires_at = Some(lesson.created_at - chrono::Duration::seconds(1));
        let mut library = ExperienceLibrary::new();
        assert_eq!(
            library
                .insert(lesson, &contrast, &validation)
                .unwrap_err()
                .code(),
            "invalid_experience"
        );
    }

    #[test]
    fn an_invalid_contrast_cannot_support_a_lesson() {
        let mut candidate = evidence("cand", true, 5);
        candidate.evidence_refs.clear();
        let bad = contrast(
            "contrast-1",
            "base-1",
            "candidate-rev-1",
            &evidence("ref", true, 10),
            &candidate,
            &identity(),
            &identity(),
        );
        assert!(bad.is_err());
        let validation = passed_validation();
        let hand_built = supporting_contrast();
        let scope = scope(HarnessDomain::RustBackend);
        let mut library = ExperienceLibrary::new();
        // The library still refuses a lesson whose cited contrast is not the valid one supplied.
        let lesson = record("lesson-1", scope, &hand_built, &validation);
        let mut other = hand_built.clone();
        other.id = "contrast-2".into();
        assert!(library
            .insert(lesson, &other, &validation)
            .is_err());
    }

    #[test]
    fn retrieval_is_domain_and_scope_scoped() {
        let (library, rust_scope) = library_with_one_lesson();
        let now = Utc::now();
        let query = ExperienceQuery::new(rust_scope.clone(), vec!["rework".into()]);
        assert_eq!(library.retrieve(&query, now).len(), 1);

        // A different domain never sees this lesson.
        let other_domain = ExperienceQuery::new(scope(HarnessDomain::QaTesting), vec!["rework".into()]);
        assert!(library.retrieve(&other_domain, now).is_empty());

        // Same domain, different mode/budget/seed: not the same conditions, not the same library.
        let sequential = ExperienceQuery::new(
            ExperienceScope::new(
                HarnessDomain::RustBackend,
                "opencode",
                EvolutionMode::Sequential,
                EvolutionBudget::k1(),
                0,
            ),
            vec!["rework".into()],
        );
        assert!(library.retrieve(&sequential, now).is_empty());
        let bigger_budget = ExperienceQuery::new(
            ExperienceScope::new(
                HarnessDomain::RustBackend,
                "opencode",
                EvolutionMode::Parallel,
                EvolutionBudget {
                    k: 2,
                    max_steps: 2,
                    max_probes: 4,
                },
                0,
            ),
            vec!["rework".into()],
        );
        assert!(library.retrieve(&bigger_budget, now).is_empty());
        let other_seed = ExperienceQuery::new(
            ExperienceScope::new(
                HarnessDomain::RustBackend,
                "opencode",
                EvolutionMode::Parallel,
                EvolutionBudget::k1(),
                7,
            ),
            vec!["rework".into()],
        );
        assert!(library.retrieve(&other_seed, now).is_empty());
    }

    #[test]
    fn retrieval_is_relevance_filtered_and_capped() {
        let contrast = supporting_contrast();
        let validation = passed_validation();
        let scope = scope(HarnessDomain::RustBackend);
        let mut library = ExperienceLibrary::new();
        for index in 0..(EXPERIENCE_RETRIEVAL_CAP + 3) {
            let mut lesson = record(&format!("lesson-{index}"), scope.clone(), &contrast, &validation);
            lesson.applicability = vec!["rust-backend".into(), format!("topic-{index}")];
            library.insert(lesson, &contrast, &validation).unwrap();
        }
        let now = Utc::now();
        let capped = ExperienceQuery::new(scope.clone(), vec![]);
        assert_eq!(capped.capped_limit(), EXPERIENCE_RETRIEVAL_CAP);
        assert_eq!(library.retrieve(&capped, now).len(), EXPERIENCE_RETRIEVAL_CAP);

        let mut unlimited = capped.clone();
        unlimited.limit = 1_000;
        assert_eq!(
            library.retrieve(&unlimited, now).len(),
            EXPERIENCE_RETRIEVAL_CAP
        );

        let relevant = ExperienceQuery::new(scope.clone(), vec!["topic-7".into()]);
        let hits = library.retrieve(&relevant, now);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "lesson-7");

        let irrelevant = ExperienceQuery::new(scope, vec!["nothing-matches".into()]);
        assert!(library.retrieve(&irrelevant, now).is_empty());
    }

    #[test]
    fn retrieval_prefers_relevance_then_confidence() {
        let contrast = supporting_contrast();
        let validation = passed_validation();
        let scope = scope(HarnessDomain::RustBackend);
        let mut library = ExperienceLibrary::new();
        let mut weak = record("lesson-weak", scope.clone(), &contrast, &validation);
        weak.applicability = vec!["rework".into(), "rust-backend".into()];
        weak.confidence = 40;
        let mut strong = record("lesson-strong", scope.clone(), &contrast, &validation);
        strong.applicability = vec!["rework".into(), "rust-backend".into(), "smith".into()];
        strong.confidence = 90;
        library.insert(weak, &contrast, &validation).unwrap();
        library.insert(strong, &contrast, &validation).unwrap();
        let hits = library.retrieve(
            &ExperienceQuery::new(
                scope,
                vec!["rework".into(), "rust-backend".into(), "smith".into()],
            ),
            Utc::now(),
        );
        assert_eq!(hits[0].id, "lesson-strong");
    }

    #[test]
    fn expired_and_future_lessons_are_not_visible() {
        let contrast = supporting_contrast();
        let validation = passed_validation();
        let scope = scope(HarnessDomain::RustBackend);
        let mut library = ExperienceLibrary::new();
        let now = Utc::now();

        let mut expired = record("lesson-expired", scope.clone(), &contrast, &validation);
        expired.created_at = now - chrono::Duration::hours(2);
        expired.expires_at = Some(now - chrono::Duration::seconds(1));
        assert_eq!(
            library.insert(expired, &contrast, &validation).unwrap(),
            InsertOutcome::Inserted
        );

        let mut future = record("lesson-future", scope.clone(), &contrast, &validation);
        future.created_at = now + chrono::Duration::hours(1);
        assert_eq!(
            library.insert(future, &contrast, &validation).unwrap(),
            InsertOutcome::Inserted
        );

        let visible = library.retrieve(&ExperienceQuery::new(scope.clone(), vec![]), now);
        assert!(visible.is_empty());
        assert_eq!(library.count_in_scope(&scope), 2);
        assert_eq!(library.expire(now), 1);
    }

    #[test]
    fn a_neutral_contrast_cannot_support_a_lesson() {
        let mut reference = evidence("ref", true, 10);
        reference.evidence_id = "evidence-ref".into();
        reference.probe_id = "probe-ref".into();
        let mut candidate = evidence("cand", true, 5);
        candidate.steps = 10;
        candidate.tokens_in = reference.tokens_in;
        candidate.tokens_out = reference.tokens_out;
        candidate.cost_micros = reference.cost_micros;
        candidate.elapsed_ms = reference.elapsed_ms;
        let neutral = contrast(
            "contrast-1",
            "base-1",
            "candidate-rev-1",
            &reference,
            &candidate,
            &identity(),
            &identity(),
        )
        .unwrap();
        assert_eq!(contrast::verdict(&neutral), ContrastVerdict::Neutral);
        let validation = passed_validation();
        let scope = scope(HarnessDomain::RustBackend);
        let mut library = ExperienceLibrary::new();
        let error = library
            .insert(record("lesson-1", scope, &neutral, &validation), &neutral, &validation)
            .unwrap_err();
        assert_eq!(error.code(), "invalid_experience");
    }

    #[test]
    fn a_validation_without_evidence_cannot_support_a_lesson() {
        let contrast = supporting_contrast();
        let complete_but_unlinked = ValidationEvidence {
            evidence_refs: Vec::new(),
            ..ValidationEvidence::all_passed(Vec::new())
        };
        let scope = scope(HarnessDomain::RustBackend);
        let mut library = ExperienceLibrary::new();
        let error = library
            .insert(
                record("lesson-1", scope, &contrast, &complete_but_unlinked),
                &contrast,
                &complete_but_unlinked,
            )
            .unwrap_err();
        assert_eq!(error.code(), "invalid_experience");
        assert!(error.to_string().contains("evidence refs"));
    }

    #[test]
    fn empty_term_retrieval_still_honours_visibility_and_cap() {
        let (library, scope) = library_with_one_lesson();
        let now = Utc::now();
        assert_eq!(
            library
                .retrieve(&ExperienceQuery::new(scope, vec![]), now)
                .len(),
            1
        );
    }
}
