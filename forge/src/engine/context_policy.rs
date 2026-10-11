//! Context pressure policy: which parts of an execution's context may be removed, in what order, and when.
//!
//! This is the DECISION layer only. It takes the context items of one execution and answers what must happen to them at
//! the current pressure. It does not own a conversation: the vendor builds each prompt, so applying these decisions to a
//! live turn is a separate step that depends on what the vendor exposes. Until then nothing here changes behavior.
//!
//! THE RULES:
//! - AUTHORITATIVE items are never removed or summarized away. They are the work order, acceptance criteria, the
//!   workflow node, candidate identity, role permissions, the execution generation and durable FACTS and DECISIONS.
//! - ACTIVE items (the current cycle, recent compiler output) are kept verbatim.
//! - HISTORICAL_OBSERVATION items (old logs, superseded diffs, repetitive tool output) are elided first, and the elision
//!   keeps a placeholder that records what was removed, so the evidence trail is never silently lost.
//! - DURABLE_KNOWLEDGE is kept; it is what survives cleanup.
//!
//! PRESSURE. Below the soft threshold nothing happens. At the soft threshold historical observations are elided. At the
//! hard threshold, if eliding is not enough, the oldest non-authoritative active context is summarized.

use serde::{Deserialize, Serialize};

/// What kind of context an item is. Classified by the caller, never guessed from its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextClass {
    Authoritative,
    Active,
    HistoricalObservation,
    DurableKnowledge,
}

/// One item of an execution's context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextItem {
    /// A stable id for the item (its position in the transcript, or an evidence id).
    pub id: String,
    pub class: ContextClass,
    pub tokens: usize,
    /// A one-line description kept in the placeholder, so the removed observation can still be identified.
    pub label: String,
}

/// The configured thresholds. Ratios of the context capacity; they live here, never in the execution engine.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextPolicy {
    pub soft_limit_ratio: f32,
    pub hard_limit_ratio: f32,
    /// The most recent active items kept verbatim even under pressure.
    pub preserve_recent_active: usize,
}

impl Default for ContextPolicy {
    fn default() -> Self {
        Self {
            soft_limit_ratio: 0.60,
            hard_limit_ratio: 0.85,
            preserve_recent_active: 4,
        }
    }
}

/// What must happen to the context at the current pressure, as ids of items.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContextAction {
    /// Historical observations to replace with a placeholder.
    pub elide: Vec<String>,
    /// Older active items to summarize (hard pressure only, and only after elision was not enough).
    pub summarize: Vec<String>,
    /// The tokens the plan frees, so a caller can see whether it reached the target.
    pub freed_tokens: usize,
}

impl ContextAction {
    pub fn is_empty(&self) -> bool {
        self.elide.is_empty() && self.summarize.is_empty()
    }
}

/// Decide what to do with `items` given the context `capacity` in tokens. Deterministic: the same items, capacity and
/// policy always give the same action.
pub fn plan(items: &[ContextItem], capacity: usize, policy: &ContextPolicy) -> ContextAction {
    let total: usize = items.iter().map(|item| item.tokens).sum();
    let soft = (capacity as f32 * policy.soft_limit_ratio) as usize;
    let hard = (capacity as f32 * policy.hard_limit_ratio) as usize;
    if capacity == 0 || total <= soft {
        return ContextAction::default();
    }

    let mut action = ContextAction::default();
    let mut remaining = total;

    // Soft pressure: historical observations go first, oldest first (the order they appear in `items`).
    for item in items
        .iter()
        .filter(|item| item.class == ContextClass::HistoricalObservation)
    {
        if remaining <= soft {
            break;
        }
        action.elide.push(item.id.clone());
        action.freed_tokens += item.tokens;
        remaining -= item.tokens;
    }

    // Hard pressure: still over the hard line, summarize the oldest active items that are not the most recent ones.
    if remaining > hard {
        let active: Vec<&ContextItem> = items
            .iter()
            .filter(|item| item.class == ContextClass::Active)
            .collect();
        let summarizable = active.len().saturating_sub(policy.preserve_recent_active);
        for item in active.into_iter().take(summarizable) {
            if remaining <= hard {
                break;
            }
            action.summarize.push(item.id.clone());
            action.freed_tokens += item.tokens;
            remaining -= item.tokens;
        }
    }

    action
}

/// The placeholder that replaces an elided observation. It keeps what was removed and where, so the record of the
/// execution stays traceable without the bytes being injected into the model's context.
pub fn elision_placeholder(item: &ContextItem) -> String {
    format!(
        "[{label} omitted: {tokens} tokens of historical observation, elided under context pressure; the raw output remains in the execution evidence (item {id})]",
        label = item.label,
        tokens = item.tokens,
        id = item.id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, class: ContextClass, tokens: usize) -> ContextItem {
        ContextItem {
            id: id.into(),
            class,
            tokens,
            label: format!("item {id}"),
        }
    }

    #[test]
    fn nothing_happens_below_the_soft_threshold() {
        let items = [item("a", ContextClass::HistoricalObservation, 100)];
        assert!(plan(&items, 1000, &ContextPolicy::default()).is_empty());
    }

    #[test]
    fn historical_observations_are_elided_before_anything_else_at_soft_pressure() {
        let items = [
            item("auth", ContextClass::Authoritative, 300),
            item("log", ContextClass::HistoricalObservation, 350),
            item("recent", ContextClass::Active, 100),
        ];
        // 750 of 1000 is above the 60% soft line; eliding the log brings it to 400, below it.
        let action = plan(&items, 1000, &ContextPolicy::default());
        assert_eq!(action.elide, vec!["log".to_string()]);
        assert!(action.summarize.is_empty());
        assert_eq!(action.freed_tokens, 350);
    }

    #[test]
    fn authoritative_and_durable_items_are_never_planned_for_removal() {
        let items = [
            item("auth", ContextClass::Authoritative, 500),
            item("fact", ContextClass::DurableKnowledge, 500),
        ];
        let action = plan(&items, 1000, &ContextPolicy::default());
        assert!(action.elide.is_empty());
        assert!(action.summarize.is_empty());
    }

    #[test]
    fn at_hard_pressure_the_oldest_active_items_are_summarized_but_the_most_recent_are_kept() {
        let policy = ContextPolicy {
            preserve_recent_active: 1,
            ..ContextPolicy::default()
        };
        let items = [
            item("auth", ContextClass::Authoritative, 200),
            item("old1", ContextClass::Active, 300),
            item("old2", ContextClass::Active, 300),
            item("newest", ContextClass::Active, 300),
        ];
        // 1100 of 1000 is over the hard line (850). Summarizing the oldest active item brings it to 800, under the line,
        // so the plan stops there: the second older item is not touched, and the newest is never summarized.
        let action = plan(&items, 1000, &policy);
        assert!(action.elide.is_empty());
        assert_eq!(action.summarize, vec!["old1".to_string()]);
        assert!(!action.summarize.contains(&"newest".to_string()));
    }

    #[test]
    fn the_same_inputs_always_give_the_same_plan() {
        let items = [
            item("log1", ContextClass::HistoricalObservation, 200),
            item("log2", ContextClass::HistoricalObservation, 200),
            item("auth", ContextClass::Authoritative, 400),
        ];
        let policy = ContextPolicy::default();
        assert_eq!(plan(&items, 1000, &policy), plan(&items, 1000, &policy));
    }

    #[test]
    fn an_elided_observation_leaves_a_placeholder_that_names_what_was_removed() {
        let log = item("cargo-7", ContextClass::HistoricalObservation, 4200);
        let placeholder = elision_placeholder(&log);
        assert!(placeholder.contains("item cargo-7 omitted"));
        assert!(placeholder.contains("4200 tokens"));
        assert!(placeholder.contains("raw output remains in the execution evidence"));
    }
}
