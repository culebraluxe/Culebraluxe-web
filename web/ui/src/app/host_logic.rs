//! The host's pure rules: staleness suppression and context-change classification.
//!
//! WHY THIS FILE EXISTS APART FROM `host.rs`. The staleness rule is the most valuable unit in the
//! shell — it is what keeps an answer to the previous record's request from landing on the new one —
//! and it is pure: generations are numbers, contexts are comparable data. It used to live in `host.rs`,
//! which is cfg-gated on the browser stack, so it could not build or
//! test on the host. It lives here instead, dependency-free (no browser-framework import: `std`, `crate::model` and
//! `crate::navigation` only), compiled unconditionally, with its own unit tests. `host.rs` keeps only
//! the `Component` impl and adapts `ScreenCtx` into a [`CtxView`] at the call site.

use std::collections::BTreeMap;

use crate::model::PortalEntitlements;
use crate::navigation::Actor;

/// Whether an answer asked for under `asked` may land while the host is on `current`.
///
/// This is the whole staleness rule, and it lives here rather than in a screen because a screen cannot forget to
/// apply it: a message carries the generation it was created under, the host moves on when another record is opened
/// ([`classify_change`]), and an answer to the previous record's request is dropped at `ScreenHost::update`.
pub fn answer_lands(current: u64, asked: u64) -> bool {
    current == asked
}

/// What a changed URL means for the mounted screen. `ScreenHost::changed` acts on this and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UrlChange {
    /// The context is identical: nothing changed, and nothing is re-drawn.
    Same,
    /// Another record — path, id or actor: start the screen over and retire the generation, so answers to the
    /// previous record's requests are dropped.
    AnotherRecord,
    /// Only the query changed (a tab, a selection): keep the state and let the screen decide (`Screen::url_changed`).
    Query,
    /// Only the grants arrived (or changed): re-draw what the view offers; nothing to read.
    Grants,
}

/// A borrow of the five fields of `ScreenCtx` that decide a context change, without naming that type.
///
/// `ScreenCtx` itself lives in the browser-gated `app::screen`, so this module cannot import it and stay
/// dependency-free. The caller (`host.rs`, which has the browser stack) builds this view; the decision stays here.
/// The field set mirrors `ScreenCtx` exactly, so comparing views decides exactly what comparing contexts decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CtxView<'a> {
    pub path: &'a str,
    pub id: Option<&'a str>,
    pub actor: &'a Actor,
    pub query: &'a BTreeMap<String, String>,
    pub grants: Option<&'a PortalEntitlements>,
}

/// Classify a context change. One decision, made here, so no screen re-decides what "another record" means.
pub fn classify_change(old: &CtxView<'_>, new: &CtxView<'_>) -> UrlChange {
    if new == old {
        UrlChange::Same
    } else if new.path != old.path || new.id != old.id || new.actor != old.actor {
        UrlChange::AnotherRecord
    } else if new.query != old.query {
        UrlChange::Query
    } else {
        UrlChange::Grants
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Owned {
        actor: Actor,
        query: BTreeMap<String, String>,
        grants: Option<PortalEntitlements>,
    }

    fn view<'a>(owned: &'a Owned, path: &'a str, id: Option<&'a str>) -> CtxView<'a> {
        CtxView {
            path,
            id,
            actor: &owned.actor,
            query: &owned.query,
            grants: owned.grants.as_ref(),
        }
    }

    fn base() -> Owned {
        Owned {
            actor: Actor {
                level: None,
                authority_codes: vec![],
                entitlement_codes: vec![],
                account_type: "internal".into(),
            },
            query: BTreeMap::new(),
            grants: None,
        }
    }

    #[test]
    fn a_stale_answer_is_dropped_and_a_current_one_lands() {
        assert!(
            !answer_lands(2, 1),
            "the previous record's answer must not land"
        );
        assert!(answer_lands(2, 2), "the current generation's answer lands");
        assert!(answer_lands(1, 1), "the first generation lands on itself");
    }

    #[test]
    fn an_identical_context_changes_nothing() {
        let ctx = base();
        let old = view(&ctx, "/portal/clients/abc", Some("abc"));
        let new = view(&ctx, "/portal/clients/abc", Some("abc"));
        assert_eq!(classify_change(&old, &new), UrlChange::Same);
    }

    #[test]
    fn another_record_restarts_the_screen() {
        let ctx = base();
        let old = view(&ctx, "/portal/clients/abc", Some("abc"));
        // Another id on the same path.
        let new_id = view(&ctx, "/portal/clients/abc", Some("xyz"));
        assert_eq!(classify_change(&old, &new_id), UrlChange::AnotherRecord);
        // Another path.
        let new_path = view(&ctx, "/portal/deals", None);
        assert_eq!(classify_change(&old, &new_path), UrlChange::AnotherRecord);
    }

    #[test]
    fn another_actor_restarts_the_screen() {
        let old_owned = base();
        let mut new_owned = base();
        new_owned.actor.account_type = "external".into();
        let old = view(&old_owned, "/portal/clients/abc", Some("abc"));
        let new = view(&new_owned, "/portal/clients/abc", Some("abc"));
        assert_eq!(classify_change(&old, &new), UrlChange::AnotherRecord);
    }

    #[test]
    fn only_the_query_changing_keeps_the_state() {
        let old_owned = base();
        let mut new_owned = base();
        new_owned.query.insert("tab".into(), "activity".into());
        let old = view(&old_owned, "/portal/clients/abc", Some("abc"));
        let new = view(&new_owned, "/portal/clients/abc", Some("abc"));
        assert_eq!(classify_change(&old, &new), UrlChange::Query);
    }

    #[test]
    fn only_the_grants_arriving_redraws_without_a_read() {
        let old_owned = base();
        let mut new_owned = base();
        new_owned.grants = Some(PortalEntitlements {
            account_type: "internal".into(),
            security_level: "ROOT".into(),
            is_root: true,
            entitlement_codes: vec![],
        });
        let old = view(&old_owned, "/portal/clients/abc", Some("abc"));
        let new = view(&new_owned, "/portal/clients/abc", Some("abc"));
        assert_eq!(classify_change(&old, &new), UrlChange::Grants);
    }
}
