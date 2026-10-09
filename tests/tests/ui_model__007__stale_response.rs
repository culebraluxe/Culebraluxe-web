//! UI.MODEL — stale response (TST-UI-MODEL-007).
//!
//! Contract: **an answer to the previous record's request never lands on the new record.** The rule lives in exactly
//! one place — the host's pure logic (`web/ui/src/app/host_logic.rs`), called by the one component that mounts every
//! screen in the portal (`ScreenHost`, `web/ui/src/app/host.rs`) — and it is two decisions, both pure:
//!
//! - every message carries the host's *generation* (`HostMsg.generation`), and a message is delivered only while that
//!   generation is still the host's (`answer_lands`, `web/ui/src/app/host_logic.rs:21`), so an answer asked for under the
//!   old record is dropped instead of applied to the new one;
//! - the generation moves on when the screen is reopened for **another record** — a different path, id or actor
//!   (`classify_change`, `web/ui/src/app/host_logic.rs:54`), while a query-only change keeps the state and asks the screen
//!   what to do (`Screen::url_changed`), and a grants-only change merely re-draws.
//!
//! Screens do not guard against this themselves, because they cannot forget to. That is why both decisions are
//! asserted against the host's own functions rather than one screen's behaviour: a screen that forgot would still be
//! safe, and the host is what makes it safe.
//!
//! The negative case is the subject: an answer created under generation 1 must **not** land once the host is on
//! generation 2, and a change of actor at the same path and id must retire the generation (the actor decides what the
//! request *is*). Both would silently render one record's data over another's if the rule were dropped, which is the
//! defect this test exists to catch: it fails if the drop is removed, not merely if it is reworded.
//!
//! The wiring is pinned too, because a predicate nobody calls protects nothing: `host.rs` must reach the decision
//! through `answer_lands` and `classify_change`, must keep exactly one place that retires a generation, and must not
//! go back to comparing generations inline.
//!
//! Level: L0 Pure — no database, no network, no browser: the host's own rule functions, plus a read of the one file
//! that calls them. The rule functions compile without the browser stack (`ui::host_logic`), while the caller pins
//! the wiring from the component side (`web/ui/src/app/host.rs`).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_model__007__stale_response

use std::collections::BTreeMap;

use test_harness::source;
use ui::app::screen::ScreenCtx;
use ui::host_logic::{answer_lands, classify_change, CtxView, UrlChange};
use ui::model::PortalEntitlements;
use ui::navigation::{Actor, Level};

const HARNESS: &str = "MviHarness/L0 Pure";

/// The host's own source: the only caller of the rule under test.
fn host_source() -> String {
    source::read(&source::repo_root().join("web/ui/src/app/host.rs"))
}

/// One actor shape, as `ScreenHost` receives it from the shell.
fn actor(level: Level, account_type: &str) -> Actor {
    Actor {
        level: Some(level),
        account_type: account_type.to_owned(),
        ..Actor::default()
    }
}

/// A screen context opened at one record.
fn ctx(path: &str, id: Option<&str>, actor: Actor) -> ScreenCtx {
    ScreenCtx {
        actor,
        id: id.map(str::to_owned),
        path: path.to_owned(),
        ..ScreenCtx::default()
    }
}

/// The host's pure view of a context: the five fields the change decision reads. Mirrors the
/// adapter in `web/ui/src/app/host.rs`, which is the only production caller of `classify_change`.
fn view(ctx: &ScreenCtx) -> CtxView<'_> {
    CtxView {
        path: &ctx.path,
        id: ctx.id.as_deref(),
        actor: &ctx.actor,
        query: &ctx.query,
        grants: ctx.grants.as_ref(),
    }
}

/// The change decision for two screen contexts, through the host's pure rule.
fn change(old: &ScreenCtx, new: &ScreenCtx) -> UrlChange {
    classify_change(&view(old), &view(new))
}

/// The grants the shell hands over when the portal's entitlement answer arrives.
fn grants() -> PortalEntitlements {
    PortalEntitlements {
        account_type: "internal".to_owned(),
        security_level: "USER".to_owned(),
        is_root: false,
        entitlement_codes: vec!["person.read".to_owned()],
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-MODEL-007); the file and the assay use it.
fn ui_model_007__stale_response() {
    // 1. The truth table of the drop, complete: a message lands exactly when the host is still on its generation.
    for current in 1..=3u64 {
        for asked in 1..=3u64 {
            assert_eq!(
                answer_lands(current, asked),
                current == asked,
                "{HARNESS}: generation {asked} against host {current} must land only when they agree"
            );
        }
    }

    // 2. The negative case, stated as the defect it prevents: the previous record's answer is dropped.
    assert!(
        !answer_lands(2, 1),
        "{HARNESS}: an answer asked for under generation 1 must not land once the host is on generation 2 — \
         dropping it is the whole contract"
    );
    assert!(
        answer_lands(1, 1),
        "{HARNESS}: the current generation's own answer must land, or no screen would ever update"
    );

    // 3. What retires the generation: another record. Another *actor* is another record at the same URL.
    let record_a = ctx(
        "/portal/clients/one",
        Some("one"),
        actor(Level::User, "internal"),
    );
    let record_b = ctx(
        "/portal/clients/two",
        Some("two"),
        actor(Level::User, "internal"),
    );
    let same_url_other_actor = ctx(
        "/portal/clients/one",
        Some("one"),
        actor(Level::Root, "internal"),
    );

    assert_eq!(
        change(&record_a, &record_a),
        UrlChange::Same,
        "{HARNESS}: an unchanged context must not re-init the screen"
    );
    assert_eq!(
        change(&record_a, &record_b),
        UrlChange::AnotherRecord,
        "{HARNESS}: another id at another path is another record"
    );

    assert_eq!(
        change(&record_a, &same_url_other_actor),
        UrlChange::AnotherRecord,
        "{HARNESS}: the same path and id under another actor is another record — the request is not the same one"
    );
    assert_eq!(
        change(
            &record_a,
            &ctx(
                "/portal/clients/one",
                Some("one"),
                actor(Level::User, "internal")
            )
        ),
        UrlChange::Same,
        "{HARNESS}: an equal context is equal, fields and all"
    );

    // 4. What does not: a query-only change is the screen's business, and grants arriving only re-draw.
    let tab = {
        let mut ctx = ctx(
            "/portal/clients/one",
            Some("one"),
            actor(Level::User, "internal"),
        );
        ctx.query = BTreeMap::from([("tab".to_owned(), "history".to_owned())]);
        ctx
    };
    assert_eq!(
        change(&record_a, &tab),
        UrlChange::Query,
        "{HARNESS}: a query-only change keeps the state and asks the screen"
    );
    assert_eq!(
        change(
            &record_a,
            &ScreenCtx {
                grants: Some(grants()),
                ..record_a.clone()
            }
        ),
        UrlChange::Grants,
        "{HARNESS}: the entitlement answer arriving changes what is offered, not what is read"
    );
    assert_eq!(
        change(&tab, &record_b),
        UrlChange::AnotherRecord,
        "{HARNESS}: another record outranks a query change — the new record starts the screen over"
    );

    // 5. The wiring: a predicate nobody calls protects nothing. One call site each, one place retires a generation,
    //    and the retired inline comparison must not come back.
    let source = host_source();
    assert!(
        source.contains("answer_lands(self.generation, message.generation)"),
        "{HARNESS}: `update` must drop a stale message through `answer_lands`, not by comparing generations inline"
    );
    assert!(
        source.contains("classify_change(&view_of(old), &view_of(new))"),
        "{HARNESS}: `changed` must act on `classify_change`, the one place that decides what another record is"
    );
    assert_eq!(
        source.matches("generation += 1").count(),
        1,
        "{HARNESS}: exactly one place retires a generation; a second is a second adjudicator of staleness"
    );
    assert!(
        !source.contains("message.generation != self.generation"),
        "{HARNESS}: the inline generation comparison was replaced by `answer_lands` and must not return"
    );
}
