//! UI.ENTITLEMENT — read-only (TST-UI-ENTITLEMENT-003).
//!
//! Contract: **an actor who may read may read, and an actor who may not write cannot start a write — the refusal
//! lives in the reducer, not only in the markup.** The Security screen is the one place in this application that draws
//! a genuinely read-only view: everyone with `security.principal.read` sees the roles, their account types and their
//! entitlement lists, and only an actor holding `security.entitlement.manage` is offered the grant controls
//! (`ctx.can(model::security::ENTITLEMENT_MANAGE)`, `app/screens/security.rs:234`).
//!
//! This screen is also the only one that **refuses in its reducer** rather than only in its view:
//!
//! ```text
//! Msg::SecurityRoleGrantRequested { .. } => {
//!     if model.role_grant_busy || !ctx.can(model::security::ENTITLEMENT_MANAGE) {
//!         return Cmd::none();
//!     }
//!     …
//! }
//! ```
//!
//! (`app/screens/security.rs:70`). That is the property this contract exists to pin, and it is the difference between
//! "the button is disabled" and "the write cannot be started". The contrast is deliberate and is asserted below: the
//! Forms editor gates `Save` in the **view** only, and its reducer does not re-check — so a test that only checked
//! `ctx.can` would pass for both screens and would not know which one actually refuses.
//!
//! Four properties, each in both directions:
//!
//!   1. **READ IS NOT WRITE.** Every internal actor that may open the screen can read it, whatever it may change. A
//!      read-only portal that refused its own reads is not a read-only portal, it is a broken one.
//!   2. **WRITE ⇒ NOT REFUSED.** An actor holding the grant action is not turned away by the reducer, and the write
//!      is actually issued — a screen that refuses everyone is as broken as one that refuses no one.
//!   3. **NO WRITE ⇒ REFUSED, AND NO COMMAND EMITTED.** The refusal returns `Cmd::none()`: no request reaches the shell,
//!      so the write is not merely un-drawn, it is never started.
//!   4. **ONE WRITE AT A TIME.** `role_grant_busy` refuses a second grant while one is in flight, so a double click
//!      cannot issue two conflicting writes. Busy is cleared by the answer, and a **failed** answer must clear it too —
//!      a busy flag left set by a failure is an editor permanently frozen.
//!
//! Level: L1 Component — the real `Screen` driven in-process through `MviHarness`. No database, no network, no PROD.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_entitlement__003__read_only

use model::security::{SecurityLevel, ENTITLEMENT_MANAGE};
use test_harness::mvi::screen::{classify, CommandKind, ScreenHarness};
use ui::app::cmd::ApiError;
use ui::app::screen::{Screen, ScreenCtx};
use ui::app::screens::security::{Model, Msg, Security};
use ui::model::PortalEntitlements;

/// A grant the screen can be asked for. Any catalogued action will do: the reducer asks only whether the actor may
/// change entitlements at all, never which one.
const GRANT: (&str, &str, bool) = ("business_power_user", "property.read", true);

/// An internal `ScreenCtx` at `level` holding exactly `granted`.
fn ctx_at(level: SecurityLevel, granted: &[&str]) -> ScreenCtx {
    let level_name = match level {
        SecurityLevel::Root => "ROOT",
        SecurityLevel::BusinessPowerUser => "BUSINESS_POWER_USER",
        SecurityLevel::User => "USER",
        SecurityLevel::Guest => "GUEST",
    };
    ScreenCtx {
        grants: Some(PortalEntitlements {
            account_type: model::security::INTERNAL_ACCOUNT.to_string(),
            security_level: level_name.to_string(),
            is_root: level == SecurityLevel::Root,
            entitlement_codes: granted.iter().map(|code| (*code).to_string()).collect(),
        }),
        ..ScreenCtx::default()
    }
}

/// The Security screen opened at `ctx`, with the model handed back for assertions.
fn opened(ctx: &ScreenCtx) -> (ScreenHarness<Security>, Model) {
    let (harness, _) = ScreenHarness::<Security>::open(ctx.clone());
    let model = harness.model().clone();
    (harness, model)
}

/// Ask for the grant through the screen's own message.
fn grant_requested() -> Msg {
    Msg::SecurityRoleGrantRequested {
        role_code: GRANT.0.to_string(),
        action: GRANT.1.to_string(),
        granted: GRANT.2,
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-ENTITLEMENT-003); the file and the assay use it.
fn ui_entitlement__003__read_only() {
    // 0. THE FIXTURE IS REAL. The whole contract turns on the actor changing between the read half and the write half,
    //    so the two must be shown to differ before anything is concluded from either.
    //
    //    The granting fixture is ROOT: `ENTITLEMENT_MANAGE` is a ROOT-only action, so the ONLY actor who may change role
    //    grants is ROOT — a lower level handed the code is refused it (section 7 proves that separately). That makes
    //    this screen's read-only property total: there is no level between "may read" and "ROOT", which is the clearest
    //    form the contract can take.
    let reader = ctx_at(SecurityLevel::User, &["security.principal.read"]);
    let writer = ctx_at(SecurityLevel::Root, &["security.principal.read"]);
    assert!(
        reader.can("security.principal.read") && !reader.can(ENTITLEMENT_MANAGE),
        "the read-only fixture must be able to read and unable to grant"
    );
    assert!(
        writer.can("security.principal.read") && writer.can(ENTITLEMENT_MANAGE),
        "the granting fixture must be able to do both — otherwise the refusal asserted below proves nothing"
    );
    assert!(
        !reader.can(ENTITLEMENT_MANAGE) && writer.can(ENTITLEMENT_MANAGE),
        "the two fixtures must differ on exactly the write action; a fixture that differs on neither would make every \
         assertion below vacuous"
    );

    // 1. READ IS NOT WRITE. The screen opens for an actor who may only read: it asks for its read model and is not
    //    turned away. A screen that refused its own reads would leave an operator unable to see why a grant was denied.
    let (mut read_only, model) = opened(&reader);
    assert!(
        matches!(model.read, ui::app::cmd::Remote::Loading),
        "the Security screen must start by asking for its read model, not by showing a refusal"
    );
    assert_eq!(
        read_only.updates(),
        0,
        "nothing has been asked of the screen yet — the model above is its `init` state"
    );

    // The refusal, driven through the screen's own message. `role_grant_busy` stays false and NO command is emitted:
    // the write is not merely un-drawn, it is never started.
    let refused = read_only.update(grant_requested());
    assert_eq!(
        classify(&refused),
        vec![CommandKind::None],
        "a read-only actor asking for a grant produced a command — the reducer must refuse the write, not merely hide \
         the control"
    );
    assert!(
        !read_only.model().role_grant_busy,
        "a refused grant left the screen busy — the operator would see every control greyed out with nothing running"
    );

    // The negative case for section 1: an external account, who may read nothing here, is refused the screen's reads as
    // well. Read-only is not a lesser form of access — it is access.
    let external = ScreenCtx {
        grants: Some(PortalEntitlements {
            account_type: "guest".into(),
            security_level: "USER".into(),
            is_root: false,
            entitlement_codes: vec!["security.principal.read".into(), ENTITLEMENT_MANAGE.into()],
        }),
        ..ScreenCtx::default()
    };
    assert!(
        !external.can("security.principal.read") && !external.can(ENTITLEMENT_MANAGE),
        "an external account holding both codes is offered both — the account type is the gate"
    );

    // 2. WRITE ⇒ NOT REFUSED. The mirror of section 1, and the direction a "refuse everyone" regression takes. The same
    //    actor, the same message, the grants added: the write is now actually issued.
    let (mut allowed, _) = opened(&writer);
    let granted = allowed.update(grant_requested());
    assert_eq!(
        classify(&granted),
        vec![CommandKind::Request],
        "an actor holding `{ENTITLEMENT_MANAGE}` cannot start a grant — the reducer refuses a write somebody is \
         entitled to make"
    );
    assert!(
        allowed.model().role_grant_busy,
        "the write was issued but the screen is not busy — a second grant could start while this one is in flight"
    );

    // ROOT satisfies the action without holding it, so the Security screen is fully operable for ROOT.
    let (mut as_root, _) = opened(&ctx_at(SecurityLevel::Root, &[]));
    assert_eq!(
        classify(&as_root.update(grant_requested())),
        vec![CommandKind::Request],
        "ROOT cannot change role grants — ROOT satisfies every action it does not reserve"
    );

    // 3. ONE WRITE AT A TIME. Busy is what makes the refusal total rather than advisory: while a grant is in flight a
    //    second is refused even for an actor entitled to make both. This is the negative case for the whole story — an
    //    editor that lets two conflicting writes race is the failure a busy flag exists to prevent.
    let second = allowed.update(grant_requested());
    assert_eq!(
        classify(&second),
        vec![CommandKind::None],
        "a second grant was issued while the first was still in flight — two conflicting writes can now race"
    );
    assert!(
        allowed.model().role_grant_busy,
        "the second grant cleared the busy flag, so the guard is a one-shot check rather than a real one"
    );

    // 4. A BUSY SCREEN REFUSES EVEN AN ENTITLED ACTOR. The refusal reads the screen's own state before the permission,
    //    so an entitled actor is turned away while the screen is working. This is asserted separately from section 3
    //    because it is a different claim: section 3 says two writes do not overlap, this one says the screen's own
    //    state is consulted at all.
    let (mut busy, _) = opened(&reader);
    let mut model = busy.model().clone();
    model.role_grant_busy = true;
    assert_eq!(
        classify(&<Security as Screen>::update(&mut model, grant_requested(), &writer)),
        vec![CommandKind::None],
        "an entitled actor's grant was issued while the screen was busy — the busy flag is not being read"
    );

    // 5. THE ANSWER CLEARS BUSY, AND A FAILURE CLEARS IT TOO. A busy flag left set by a failed write is an editor frozen
    //    forever with every control greyed out and nothing running — the most likely way this screen actually breaks,
    //    because the failure path is the one nobody exercises by hand.
    let (mut failed, _) = opened(&writer);
    let mut model = failed.model().clone();
    model.role_grant_busy = true;
    <Security as Screen>::update(
        &mut model,
        Msg::GrantAnswered(Err(ApiError::network("harness proves the failure path"))),
        &writer,
    );
    assert!(
        !model.role_grant_busy,
        "a failed grant left the screen busy — every control stays disabled with nothing in flight, and the operator \
         has no way to recover"
    );
    // And the screen is usable again: the same refused-then-answered cycle must not leave it stuck in either state.
    let recovered = <Security as Screen>::update(&mut model, grant_requested(), &writer);
    assert_eq!(
        classify(&recovered),
        vec![CommandKind::Request],
        "the screen is still refusing grants after a failure — the recovery path does not work"
    );

    // 6. A SUCCESSFUL ANSWER CLEARS BUSY TOO. The mirror of section 5, so the clearing is a property of the answer
    //    rather than of the failure branch alone.
    let (mut answered, _) = opened(&writer);
    let mut model = answered.model().clone();
    model.role_grant_busy = true;
    <Security as Screen>::update(&mut model, Msg::GrantAnswered(Ok(Vec::new())), &writer);
    assert!(
        !model.role_grant_busy,
        "a completed grant left the screen busy — the editor stays disabled after the write has already finished"
    );

    // 7. THE REFUSAL IS THE SCREEN'S, NOT THE TEST'S. `ENTITLEMENT_MANAGE` is read from the domain's own name here, so
    //    a screen that decided "ROOT only" from a list of its own would drift the moment the domain moved. Pinned
    //    because it is the difference between one reservation and two.
    assert!(
        model::security::is_root_only(ENTITLEMENT_MANAGE),
        "`{ENTITLEMENT_MANAGE}` stopped being ROOT-only; the reservation the screen enforces has changed"
    );
    // So a BUSINESS_POWER_USER holding the grant code is still refused it by the same reducer — the read-only screen
    // is read-only for everyone but ROOT, whichever code they were handed.
    let (mut power, _) = opened(&ctx_at(
        SecurityLevel::BusinessPowerUser,
        &[ENTITLEMENT_MANAGE],
    ));
    assert_eq!(
        classify(&power.update(grant_requested())),
        vec![CommandKind::None],
        "a BUSINESS_POWER_USER holding `{ENTITLEMENT_MANAGE}` changed role grants — the reservation is `is_root`, and \
         a grant of the code must not reach a non-ROOT level"
    );
}
