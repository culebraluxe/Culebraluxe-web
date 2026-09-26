//! The operating navigation, ported from `lib/navigation/registry.ts` — items, words, order AND the rules that decide
//! who sees what.
//!
//! WHY THIS IS NOT IN `model.rs`. The model answers "what screens exist, and which surface owns each" — routing. This
//! answers "what does the nav show, to WHOM, in what order" — the designed menu. They are different questions and they
//! had different answers: the chrome was built from the model's screen titles and its `Listed` flag, which produced a
//! rail the operator did not recognise. Concretely, the model says OPPS has three items (Issue Queue, Needs Review, Data
//! Workbench); the registry shows two (Records, Listing Media). It calls the fourth NEXUS item `Deals`; the registry
//! calls it `Contracts`, at `Contracts`' own route.
//!
//! THE MENU ITSELF (items, labels, order, visibility) NOW LIVES IN THE REGISTRY (`app/registry.rs`); this module keeps the
//! actor, the levels and each surface's home. Historical note on where the menu came from:
//!
//! SO THIS TABLE WAS THE AUTHORITY, copied from the registry item for item, label and href exactly as the registry writes
//! them — including `OPPS` rather than `OPS`, which is the registry's spelling.
//!
//! THE VISIBILITY RULES ARE THE REGISTRY'S TOO, from `operating-shell.tsx`, which is where they actually ran:
//!
//!   item:    the authority must be held, and the entitlement must be held unless the level is ROOT.
//!   surface: the same, plus `minSecurityLevel` and `accessAuthority` where the registry sets them — TECH is ROOT-only
//!            and requires `tech.access`.
//!
//! THIS IS UI VISIBILITY, NOT AUTHORIZATION. Hiding a link is not a gate: `app/portal/layout.tsx` still enforces
//! `portal.read` on the server, and every route re-checks. If a link shows that should not, that is a bug in this table,
//! not a hole — but a link that HIDES is also not protection.

use crate::model::Surface;

/// The four broad levels, ranked as `lib/auth/security-level.ts` ranks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Guest,
    User,
    BusinessPowerUser,
    Root,
}

impl Level {
    /// Fail CLOSED: an unknown or missing level is `Guest`, never something higher. The TypeScript resolver did the
    /// same, and for the same reason — a string nobody recognises must not read as access.
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_uppercase().as_str() {
            "ROOT" => Self::Root,
            "BUSINESS_POWER_USER" => Self::BusinessPowerUser,
            "USER" => Self::User,
            _ => Self::Guest,
        }
    }
}

/// The actor as the page hands it over: the fields `toPortalActorSnapshot` projects. Enough to decide visibility and
/// nothing more — no session, no token, no identity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Actor {
    pub level: Option<Level>,
    pub authority_codes: Vec<String>,
    pub entitlement_codes: Vec<String>,
    pub account_type: String,
}

impl Actor {
    pub(crate) fn level(&self) -> Level {
        self.level.unwrap_or(Level::Guest)
    }

    pub(crate) fn internal(&self) -> bool {
        self.account_type == "internal"
    }

    pub(crate) fn is_root(&self) -> bool {
        self.level() == Level::Root
    }

    pub(crate) fn holds_authority(&self, code: &str) -> bool {
        code.is_empty() || self.authority_codes.iter().any(|held| held == code)
    }

    pub(crate) fn holds_entitlement(&self, code: &str) -> bool {
        // An empty requirement is satisfied. ROOT satisfies everything. An external account satisfies none.
        code.is_empty() || self.is_root() || self.entitlement_codes.iter().any(|held| held == code)
    }
}

/// Where a surface's top-nav capsule leads. The registry's `home`, NOT its first item — those differ for OPPS, whose
/// home is the Data Workbench while its first item is Records.
pub fn home_path(surface: Surface) -> &'static str {
    match surface {
        Surface::Core => "/portal/dashboard",
        Surface::Accounting => "/portal/accounting",
        Surface::Marketing => "/portal/marketing",
        Surface::Ops => "/portal/property-admin",
        Surface::Support => "/portal/system-health",
        Surface::Tech => "/portal/tech",
        Surface::Site => "/",
    }
}

thread_local! {
    /// The actor for this document. WASM is single-threaded here, and the page hands the snapshot over once.
    static ACTOR: std::cell::RefCell<Actor> = std::cell::RefCell::new(Actor::default());
}

/// Read the actor's projection from the JSON the page wrote.
///
/// MISSING FIELDS ARE NOT AN ERROR and they do not default to access: an unparsable payload leaves the actor empty,
/// which hides the entitlement-gated nav rather than showing it. That is the fail-closed direction on purpose.
pub fn actor_from_json(json: &str) -> Actor {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Actor::default();
    };
    let strings = |key: &str| -> Vec<String> {
        value
            .get(key)
            .and_then(|field| field.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    Actor {
        level: value
            .get("securityLevel")
            .and_then(|field| field.as_str())
            .map(Level::parse),
        authority_codes: strings("authorityCodes"),
        entitlement_codes: strings("entitlementCodes"),
        account_type: value
            .get("accountType")
            .and_then(|field| field.as_str())
            .unwrap_or_default()
            .to_owned(),
    }
}

/// Store the actor the page handed over.
pub fn set_actor(actor: Actor) {
    ACTOR.with(|slot| *slot.borrow_mut() = actor);
}

/// The actor for this document, or an empty one when the page never provided it.
pub fn actor() -> Actor {
    ACTOR.with(|slot| slot.borrow().clone())
}
