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
//! SO THIS TABLE IS THE AUTHORITY, copied from the registry item for item, label and href exactly as the registry writes
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
#[derive(Debug, Clone, Default)]
pub struct Actor {
    pub level: Option<Level>,
    pub authority_codes: Vec<String>,
    pub entitlement_codes: Vec<String>,
    pub account_type: String,
}

impl Actor {
    fn level(&self) -> Level {
        self.level.unwrap_or(Level::Guest)
    }

    fn internal(&self) -> bool {
        self.account_type == "internal"
    }

    fn is_root(&self) -> bool {
        self.level() == Level::Root
    }

    fn holds_authority(&self, code: &str) -> bool {
        code.is_empty() || self.authority_codes.iter().any(|held| held == code)
    }

    fn holds_entitlement(&self, code: &str) -> bool {
        // An empty requirement is satisfied. ROOT satisfies everything. An external account satisfies none.
        code.is_empty() || self.is_root() || self.entitlement_codes.iter().any(|held| held == code)
    }
}

/// One rail entry: label, href, and the registry's `authority` / `entitlement`.
pub struct Item {
    pub label: &'static str,
    pub href: &'static str,
    pub authority: &'static str,
    pub entitlement: &'static str,
}

/// One operating world, with the registry's surface-level rules.
pub struct SurfaceDef {
    pub surface: Surface,
    pub min_level: Option<Level>,
    pub access_authority: &'static str,
    pub items: &'static [Item],
}

const fn item(
    label: &'static str,
    href: &'static str,
    authority: &'static str,
    entitlement: &'static str,
) -> Item {
    Item {
        label,
        href,
        authority,
        entitlement,
    }
}

const NEXUS_ITEMS: &[Item] = &[
    item(
        "Cockpit",
        "/portal/dashboard",
        "portal.read",
        "cockpit.read",
    ),
    item("Clients", "/portal/clients", "portal.read", "person.read"),
    item(
        "Projects",
        "/portal/projects",
        "portal.read",
        "project.read",
    ),
    item("Contracts", "/portal/deals", "deal.read", "deal.read"),
    item("Cabinet", "/portal/documents", "deal.read", "vault.read"),
    item(
        "Workflows",
        "/portal/workflows",
        "portal.read",
        "portal.read",
    ),
    item("Forms", "/portal/forms", "deal.read", "form.read"),
    item(
        "Seller Strategy",
        "/portal/core/seller-strategy",
        "portal.read",
        "portal.read",
    ),
];

const ACCOUNTING_ITEMS: &[Item] = &[
    item(
        "Dashboard",
        "/portal/accounting",
        "portal.read",
        "accounting.read",
    ),
    item(
        "Receivables",
        "/portal/accounting/receivables",
        "portal.read",
        "accounting.read",
    ),
    item(
        "Expenses",
        "/portal/accounting/expenses",
        "portal.read",
        "accounting.read",
    ),
    item(
        "P&L Statement",
        "/portal/accounting/pnl",
        "portal.read",
        "accounting.read",
    ),
    item(
        "Receipt Scanner",
        "/portal/accounting/receipt-scanner",
        "portal.read",
        "accounting.read",
    ),
];

const MARKETING_ITEMS: &[Item] = &[
    item(
        "Dashboard",
        "/portal/marketing",
        "portal.read",
        "property.read",
    ),
    item(
        "Syndication",
        "/portal/marketing/syndication",
        "portal.read",
        "property.read",
    ),
];

const OPS_ITEMS: &[Item] = &[
    item(
        "Records",
        "/portal/property-admin",
        "portal.read",
        "property.read",
    ),
    item(
        "Listing Media",
        "/portal/property-media",
        "portal.read",
        "property.read",
    ),
];

const TECH_ITEMS: &[Item] = &[
    item("Cockpit", "/portal/tech", "tech.access", "tech.access"),
    item(
        "Story Board",
        "/portal/storyboard",
        "tech.access",
        "tech.access",
    ),
    item("UI Lab", "/portal/design-lab", "tech.access", "tech.access"),
];

const SUPPORT_ITEMS: &[Item] = &[
    item(
        "System Health",
        "/portal/system-health",
        "portal.read",
        "portal.read",
    ),
    item("DB Test", "/portal/db-test", "portal.read", "portal.read"),
    item(
        "WhatsApp Diagnostic",
        "/portal/admin/whatsapp-meta",
        "portal.read",
        "portal.read",
    ),
    item(
        "Security",
        "/portal/settings",
        "settings.read",
        "security.principal.read",
    ),
];

/// The six operating worlds, in `OPERATING_SURFACE_ORDER` — the registry's DISPLAY order, which is not the order its
/// object happens to be written in. SUPPORT PRECEDES TECH there, and the chrome follows the array, not the file.
pub const SURFACES: &[SurfaceDef] = &[
    SurfaceDef {
        surface: Surface::Core,
        min_level: None,
        access_authority: "",
        items: NEXUS_ITEMS,
    },
    SurfaceDef {
        surface: Surface::Accounting,
        min_level: None,
        access_authority: "",
        items: ACCOUNTING_ITEMS,
    },
    SurfaceDef {
        surface: Surface::Marketing,
        min_level: None,
        access_authority: "",
        items: MARKETING_ITEMS,
    },
    SurfaceDef {
        surface: Surface::Ops,
        min_level: None,
        access_authority: "",
        items: OPS_ITEMS,
    },
    SurfaceDef {
        surface: Surface::Support,
        min_level: None,
        access_authority: "",
        items: SUPPORT_ITEMS,
    },
    // The registry: `minSecurityLevel: 'ROOT'`, `accessAuthority: 'tech.access'`.
    SurfaceDef {
        surface: Surface::Tech,
        min_level: Some(Level::Root),
        access_authority: "tech.access",
        items: TECH_ITEMS,
    },
];

fn definition(surface: Surface) -> Option<&'static SurfaceDef> {
    SURFACES.iter().find(|def| def.surface == surface)
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

/// Whether a surface's capsule shows at all.
pub fn surface_visible(surface: Surface, actor: &Actor) -> bool {
    let Some(def) = definition(surface) else {
        return false;
    };
    if !actor.holds_authority(def.access_authority) {
        return false;
    }
    if let Some(required) = def.min_level {
        if actor.level() < required {
            return false;
        }
    }
    // The surface's own entitlement gate, from `operating-shell.tsx`: an internal account, and either ROOT or at least
    // one of the surface's items reachable — a world whose every door is shut is not offered.
    actor.internal()
        && (actor.is_root()
            || def.items.iter().any(|item| {
                !item.entitlement.is_empty()
                    && actor
                        .entitlement_codes
                        .iter()
                        .any(|held| held == item.entitlement)
            }))
}

/// Whether one rail item shows.
pub fn item_visible(item: &Item, actor: &Actor) -> bool {
    actor.holds_authority(item.authority)
        && actor.internal()
        && actor.holds_entitlement(item.entitlement)
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

/// The operating worlds whose capsule shows, in registry order.
pub fn visible_surfaces(actor: &Actor) -> Vec<Surface> {
    SURFACES
        .iter()
        .filter(|def| surface_visible(def.surface, actor))
        .map(|def| def.surface)
        .collect()
}

/// The rail for a surface, filtered for this actor, in registry order.
pub fn visible_items(surface: Surface, actor: &Actor) -> Vec<&'static Item> {
    definition(surface)
        .map(|def| {
            def.items
                .iter()
                .filter(|item| item_visible(item, actor))
                .collect()
        })
        .unwrap_or_default()
}
