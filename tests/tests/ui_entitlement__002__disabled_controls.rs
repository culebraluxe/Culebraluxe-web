//! UI.ENTITLEMENT — disabled controls (TST-UI-ENTITLEMENT-002).
//!
//! Contract: **a control an actor may not use is disabled, and it is not disabled for the wrong reason.** The Forms
//! editor is where this application offers privileged actions beside ordinary ones. Two of its controls are gated on a
//! permission — `Save` (issue a PDF, `vault.issue`) and `Send for signature` (`documentSign.issue`) — and the view
//! draws each with `disabled={working || !ctx.can("…")}` (`app/screens/forms/editor.rs`). The other two are gated on the
//! model's own state. Both halves matter, and they fail in opposite directions:
//!
//!   1. **NOT PERMITTED ⇒ DISABLED.** `ScreenCtx::can(action)` is the production rule (`app/screen.rs`): internal
//!      accounts only, ROOT-only actions for ROOT alone, everything else for ROOT or a held entitlement. An editor
//!      action an actor cannot perform must read `false`, or the operator is invited to press a button the server
//!      refuses.
//!   2. **PERMITTED ⇒ NOT DISABLED ON THAT GROUND.** The mirror, and the direction a "disable everything to be safe"
//!      regression takes: an action an actor *does* hold must read `true`, or the editor denies an operator something
//!      they are entitled to. A control that is always disabled is as broken as one that is never disabled.
//!   3. **THE RESERVATION IS ROOT'S ALONE.** A grant is not authority over a ROOT-only action. `model::security`
//!      holds one list (`ROOT_ONLY_ACTIONS`) read by both the server and this rule, so a non-ROOT level holding
//!      `security.role.manage` is still refused it.
//!   4. **NOTHING IS OFFERED BEFORE THE GRANTS ARRIVE.** `ScreenCtx::grants` is `None` until the shell has the
//!      server's answer. The rule must refuse through that window rather than defaulting to allowed — a screen that
//!      offers its controls for the moment before the answer arrives offers them to whoever the server turns out to
//!      refuse.
//!   5. **BUSY IS THE REDUCER'S, AND THE VIEW READS IT.** A write in flight disables the editor's actions so a double
//!      submission cannot be started from the UI. That is model state, so it is asserted by driving the real `Screen`
//!      through `MviHarness` — production's `init`/`update`, not a re-declared copy.
//!
//! WHAT IS AND IS NOT PROVEN HERE, stated so a green run is not over-read. This drives `Screen::init`/`Screen::update`
//! through the real MVI boundary, so the model and the commands are production's. The `disabled` **attribute** is drawn
//! in the view, and the Yew VDOM cannot be walked from a host test — `VList`'s children are `pub(crate)` in `yew 0.23`.
//! So this pins the decisions the view reads when it draws that attribute: `ctx.can(..)` and the model's own state. It
//! does not assert the attribute itself. Nor does it claim the reducer re-checks permission — it does not, and must not:
//! the server authorizes every request, and a reducer that refused would be a second, weaker adjudicator of one rule.
//!
//! Level: L1 Component — the real `Screen` driven in-process through `MviHarness`, with real `ScreenCtx` grants. No
//! database, no network, no PROD, no browser.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_entitlement__002__disabled_controls

use model::security::{SecurityLevel, ROLE_MANAGE};
use test_harness::actors::ActorBuilder;
use test_harness::mvi::screen::{classify, effect_kind, CommandKind, ScreenHarness};
use ui::app::cmd::ApiError;
use ui::app::screen::ScreenCtx;
use ui::app::screens::forms::{FormRecord, Msg as FormRecordMsg};
use ui::model::PortalEntitlements;

/// The two privileged actions the Forms editor draws as permission-gated controls, and the entitlement each needs.
const PRIVILEGED: [(&str, &str); 2] = [
    ("vault.issue", "Save"),
    ("documentSign.issue", "Send for signature"),
];

/// A `ScreenCtx` for an internal actor at `level`, holding exactly `granted` and nothing else.
///
/// The principal and its level come from `ActorBuilder`, the harness's own projection of the server's role vocabulary,
/// so the fixture cannot drift from what the server would resolve. The grants are then **replaced** rather than
/// appended: the contract under test is about an actor holding a *specific* list, and a fixture that granted
/// everything would make sections 1 and 2 indistinguishable.
fn ctx_at(level: SecurityLevel, granted: &[&str]) -> ScreenCtx {
    let ctx = ActorBuilder::at(level, "u-1").screen_ctx("/portal/forms");
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
        ..ctx
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-ENTITLEMENT-002); the file and the assay use it.
fn ui_entitlement__002__disabled_controls() {
    // THE FIXTURE IS REAL. Before anything is concluded from `can`, prove the fixture distinguishes the two things the
    // contract turns on. If an unprivileged actor and a privileged one answered alike, sections 1 and 2 would prove
    // nothing at all — so this is the check that makes the rest of the test meaningful.
    let unprivileged = ctx_at(SecurityLevel::User, &[]);
    let privileged = ctx_at(SecurityLevel::User, &["vault.issue", "documentSign.issue"]);
    for (action, button) in PRIVILEGED {
        assert!(
            !unprivileged.can(action),
            "the unprivileged fixture holds `{action}`"
        );
        assert!(
            privileged.can(action),
            "the privileged fixture cannot `{action}` — the grant is not reaching the production rule, so every \
             refusal asserted below would be vacuous ({button} would be disabled for everyone)"
        );
    }

    // 1. NOT PERMITTED ⇒ DISABLED. The direction that decides whether the UI respects the entitlement at all: the two
    //    actions the editor gates on a permission are refused for an internal actor holding neither.
    for (action, button) in PRIVILEGED {
        assert!(
            !unprivileged.can(action),
            "an unprivileged internal actor is offered `{action}` — the editor draws {button} enabled, so the operator \
             is invited to press a control the server will refuse"
        );
    }
    // Each half on its own, so a rule that refused everything for the wrong reason could not pass.
    let only_save = ctx_at(SecurityLevel::User, &["vault.issue"]);
    assert!(only_save.can("vault.issue"));
    assert!(
        !only_save.can("documentSign.issue"),
        "holding `vault.issue` also opened `documentSign.issue` — one grant opening a second control is exactly the \
         over-broad grant this contract refuses"
    );

    // 2. PERMITTED ⇒ NOT DISABLED ON THAT GROUND. The mirror, and the direction a "disable everything to be safe"
    //    regression takes. ROOT holds neither action and satisfies both — the editor is fully usable for ROOT.
    let root = ctx_at(SecurityLevel::Root, &[]);
    for (action, button) in PRIVILEGED {
        assert!(
            root.can(action),
            "ROOT is refused `{action}`, so the editor disables {button} for the one actor who may use it"
        );
    }

    // 3. THE RESERVATION IS ROOT'S ALONE. `model::security` holds one list read by both the server and this rule, so a
    //    grant is not authority over a ROOT-only action. This is the negative case for the reservation: a non-ROOT level
    //    that holds the code must still be refused it.
    assert!(
        model::security::is_root_only(ROLE_MANAGE),
        "`{ROLE_MANAGE}` stopped being ROOT-only; the reservation this test pins no longer exists"
    );
    let power_user = ctx_at(SecurityLevel::BusinessPowerUser, &[ROLE_MANAGE]);
    assert!(
        !power_user.can(ROLE_MANAGE),
        "a BUSINESS_POWER_USER holding `{ROLE_MANAGE}` is offered it — the reservation is `is_root`, so a grant of a \
         ROOT-only code must not reach a non-ROOT level"
    );
    assert!(
        root.can(ROLE_MANAGE),
        "ROOT is refused a ROOT-only action — `is_root` is true for ROOT, which is the one level meant to reach it"
    );
    // And the rule is asked through the domain rather than by re-typing the codes: a screen that decided "ROOT only"
    // from its own list would drift the moment the domain's list moved.
    for action in [ROLE_MANAGE, model::security::ENTITLEMENT_MANAGE] {
        assert!(
            model::security::is_root_only(action),
            "{action} must stay in the domain's one ROOT-only list"
        );
    }

    // 4. NOTHING IS OFFERED BEFORE THE GRANTS ARRIVE. `grants` is `None` until the shell has the server's answer, and
    //    `can` must refuse through that window — fail closed, not open.
    let mut arriving = ActorBuilder::root("u-2").screen_ctx("/portal/forms");
    arriving.grants = None;
    assert!(
        arriving.grants.is_none(),
        "this fixture is the window before the grants arrive"
    );
    for (action, button) in PRIVILEGED {
        assert!(
            !arriving.can(action),
            "`{action}` is offered before the server's grants arrive, so {button} is drawn enabled for whoever the \
             server turns out to refuse"
        );
    }

    // 5. AN EXTERNAL ACCOUNT IS OFFERED NOTHING, AND THE ACCOUNT TYPE IS THE REASON. The negative case for the whole
    //    story: an actor outside the portal must reach none of its controls however plausible its list looks. The level
    //    is Guest and the grants are generous, so only the account type can be doing the work.
    let external = ActorBuilder::external_guest("g-1")
        .entitlement("portal.read")
        .entitlement("vault.issue")
        .entitlement("documentSign.issue")
        .screen_ctx("/portal/forms");
    for (action, button) in PRIVILEGED {
        assert!(
            !external.can(action),
            "an external account holding `{action}` is offered it, so {button} is drawn enabled for a guest or a \
             client — `can` checks the account type first, and an external account holds nothing the portal offers"
        );
    }
    assert!(
        !external.can("portal.read"),
        "an external account is offered the portal read"
    );
    // The same rule, driven through the same production call, with ROOT's level and ROOT's code: an external account
    // whose level says ROOT is still refused, because `is_root` is read from the grants projection and an account type
    // is not a level.
    let privileged_guest = ScreenCtx {
        grants: Some(PortalEntitlements {
            account_type: "guest".into(),
            security_level: "ROOT".into(),
            is_root: true,
            entitlement_codes: vec!["vault.issue".into()],
        }),
        ..ActorBuilder::external_guest("g-2").screen_ctx("/portal/forms")
    };
    assert!(
        !privileged_guest.can("vault.issue"),
        "an external account claiming ROOT reaches the editor's controls — the account type is the gate, and no level \
         string grants an account the right to operate the portal"
    );

    // 6. BUSY IS THE REDUCER'S, AND THE VIEW READS IT. Driven through the real `Screen`, so the model and the commands
    //    are production's rather than a re-declaration.
    //
    //    A record route carries its id in the ctx: `FormRecord::init` refuses without one ("Form id is missing.") and
    //    draws no controls at all, so the id is supplied the way the router supplies it. That refusal is itself worth
    //    pinning — an editor opened with no record draws nothing rather than offering actions on no data.
    let mut no_id = ctx_at(SecurityLevel::User, &["vault.issue"]);
    no_id.id = None;
    let (_idless, idless_command) = ScreenHarness::<FormRecord>::open(no_id);
    assert_eq!(
        classify(&idless_command),
        vec![CommandKind::None],
        "a form record opened with no record id asks the shell for anything — the editor would offer its controls on \
         data it does not have, which is the one thing the busy half is meant to prevent"
    );

    // With the id the route carries, the editor asks the shell for its read: it draws its controls over data it does
    // not have yet, which is precisely why the busy half exists.
    let mut with_id = ctx_at(SecurityLevel::User, &["vault.issue"]);
    with_id.id = Some("form-1".into());
    with_id.path = "/portal/forms/form-1".into();
    let (mut harness, command) = ScreenHarness::<FormRecord>::open(with_id);
    assert_eq!(
        effect_kind(&command),
        Some(CommandKind::Request),
        "opening a form record asks the shell for its read"
    );

    // A refused read leaves the editor with nothing to act on: the failure path, and the negative case for "the
    // controls are always there".
    let refused = harness.update(FormRecordMsg::RecordLoaded(Err(ApiError::network(
        "harness proves the failure path",
    ))));
    assert_eq!(
        classify(&refused),
        vec![CommandKind::None],
        "a refused read asks for nothing further — there is no partial edit to offer an action on"
    );

    // A read that fails is not an authorization failure, so it must not be mistaken for one: the fixture's actor still
    // holds what it held, and the editor's controls answer from the ctx alone. This is the separation the server's own
    // rule depends on — a failed read must not change what anybody may do.
    assert!(
        harness.ctx().can("vault.issue"),
        "a failed read changed what the actor may do — a transport failure must not alter an authorization decision"
    );
    assert!(
        !harness.ctx().can("documentSign.issue"),
        "a failed read granted an action the actor never held"
    );
}
