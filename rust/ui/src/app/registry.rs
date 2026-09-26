//! THE REGISTRY — every screen the app serves, in one table. See docs/agent/UI-SCREEN-ARCHITECTURE.md.
//!
//! Routing, the site header, the portal rail, the headless navigation walk and the "every page has a screen" check are
//! all generated from `ENTRIES`. Nothing else lists screens. Adding a screen is: implement `Screen`, add one line here.
//!
//! THE KINDS, and why three of them are temporary:
//!   * `Screen(mount)` — a screen on the `Screen` trait. The only kind that survives the cutover.
//!   * `LegacyPortal(key)` / `LegacySite` — a screen still on the old global MVI loop, hosted inside this shell while it
//!     is ported. Each one is a debt: the count only goes down (`legacy_count` test), and at zero both kinds are deleted.
//!   * `External` — a page Next still renders itself (Auth.js sign-in, the React Forms editor). Links to it load the
//!     document.
//!
//! The nav order is the table order. The rail and header labels are the designed menu (ported from
//! `lib/navigation/registry.ts` item for item), not screen titles.

use yew::Html;

use crate::app::host::mount;
use crate::app::rows::RowsScreen;
use crate::app::screen::ScreenCtx;
use crate::app::screens::account::Account;
use crate::app::screens::accounting;
use crate::app::screens::clients::{ClientRecord, Clients};
use crate::app::screens::db_test::DbTest;
use crate::app::screens::security::Security;
use crate::app::screens::security_users::SecurityUsers;
use crate::app::screens::support_rows::{Authorities, Review, Roles, VideoTest};
use crate::app::screens::system_health::SystemHealth;
use crate::app::screens::whatsapp_meta::WhatsAppMeta;
use crate::app::screens::whatsapp_public::WhatsAppPublic;
use crate::model::Surface;
use crate::navigation::{Actor, Level};

/// Which application area a path belongs to. Moving between areas is a document load: the portal's server-side guard
/// and its actor snapshot only exist on a portal page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    Site,
    Portal,
}

/// Where a screen appears in the menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    None,
    /// The public site's header, with this label.
    Header(&'static str),
    /// Its surface's rail in the portal, with this label.
    Rail(&'static str),
}

#[derive(Clone, Copy)]
pub enum Kind {
    /// On the `Screen` trait. The function mounts `ScreenHost<S>`.
    Screen(fn(ScreenCtx) -> Html),
    /// Still on the old loop: the old portal app for this screen key. Temporary.
    LegacyPortal(&'static str),
    /// As `LegacyPortal`, and its PAGE mounts a React vendor island (`components/rust-ui/rust-ui.tsx` picks the island
    /// by the page's screen). Moving to or from it is therefore a document load: an in-app move would leave the island
    /// of the page you started on. Temporary — it goes when the island is behind the `Island` component.
    LegacyIsland(&'static str),
    /// Still on the old loop: the old site app. Temporary.
    LegacySite,
    /// Rendered by Next, not by this app.
    External,
}

impl std::fmt::Debug for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Kind::Screen(_) => write!(f, "Screen"),
            Kind::LegacyPortal(key) => write!(f, "LegacyPortal({key})"),
            Kind::LegacyIsland(key) => write!(f, "LegacyIsland({key})"),
            Kind::LegacySite => write!(f, "LegacySite"),
            Kind::External => write!(f, "External"),
        }
    }
}

/// One screen's identity.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub key: &'static str,
    /// `/portal/clients/:personId` — `:name` segments become `ScreenCtx` params.
    pub path: &'static str,
    pub surface: Surface,
    pub title: &'static str,
    pub menu: Menu,
    /// UI VISIBILITY ONLY. The server authorizes every request; hiding a link is not a gate.
    pub authority: &'static str,
    pub entitlement: &'static str,
    pub kind: Kind,
    /// The screen this one is reached FROM — a record from its list, a sub-page from its screen. The menus highlight the
    /// parent while you are here, and the template's back link returns to it.
    pub parent: Option<&'static str>,
}

impl Entry {
    /// Whether arriving here, or leaving from here, must load the document rather than move in-app.
    pub fn needs_document(&self) -> bool {
        matches!(self.kind, Kind::External | Kind::LegacyIsland(_))
    }

    /// The area is a fact about the URL, not the menu: a public page filed under SUPPORT (the WhatsApp page Meta
    /// needed) still renders in the site chrome, because it is served outside `/portal` and its guard.
    pub fn area(&self) -> Area {
        area_of(self.path)
    }
}

#[allow(clippy::too_many_arguments)]
const fn entry(
    key: &'static str,
    path: &'static str,
    surface: Surface,
    title: &'static str,
    menu: Menu,
    authority: &'static str,
    entitlement: &'static str,
    kind: Kind,
) -> Entry {
    Entry {
        key,
        path,
        surface,
        title,
        menu,
        authority,
        entitlement,
        kind,
        parent: None,
    }
}

impl Entry {
    /// Mark this entry as a drill-in of `parent` (a key in this table).
    const fn of(self, parent: &'static str) -> Self {
        Entry {
            parent: Some(parent),
            ..self
        }
    }
}

#[rustfmt::skip]
pub const ENTRIES: &[Entry] = &[
    entry("dashboard", "/portal/dashboard", Surface::Core, "Cockpit", Menu::Rail("Cockpit"), "portal.read", "cockpit.read", Kind::LegacyPortal("dashboard")),
    entry("clients", "/portal/clients", Surface::Core, "Clients", Menu::Rail("Clients"), "portal.read", "person.read", Kind::Screen(mount::<Clients>)),
    entry("projects", "/portal/projects", Surface::Core, "Projects", Menu::Rail("Projects"), "portal.read", "project.read", Kind::LegacyIsland("projects")),
    entry("deals", "/portal/deals", Surface::Core, "Contracts", Menu::Rail("Contracts"), "deal.read", "deal.read", Kind::LegacyPortal("deals")),
    entry("cabinet", "/portal/documents", Surface::Core, "Cabinet", Menu::Rail("Cabinet"), "deal.read", "vault.read", Kind::LegacyPortal("cabinet")),
    entry("workflows", "/portal/workflows", Surface::Core, "Workflows", Menu::Rail("Workflows"), "portal.read", "portal.read", Kind::LegacyPortal("workflows")),
    entry("forms", "/portal/forms", Surface::Core, "Forms", Menu::Rail("Forms"), "deal.read", "form.read", Kind::External),
    entry("seller-strategy", "/portal/core/seller-strategy", Surface::Core, "Seller Strategy", Menu::Rail("Seller Strategy"), "portal.read", "portal.read", Kind::LegacyPortal("seller-strategy")),
    entry("accounting", "/portal/accounting", Surface::Accounting, "Dashboard", Menu::Rail("Dashboard"), "portal.read", "accounting.read", Kind::Screen(mount::<accounting::Dashboard>)),
    entry("accounting-receivables", "/portal/accounting/receivables", Surface::Accounting, "Receivables", Menu::Rail("Receivables"), "portal.read", "accounting.read", Kind::Screen(mount::<accounting::Receivables>)),
    entry("accounting-expenses", "/portal/accounting/expenses", Surface::Accounting, "Expenses", Menu::Rail("Expenses"), "portal.read", "accounting.read", Kind::Screen(mount::<accounting::Expenses>)),
    entry("accounting-pnl", "/portal/accounting/pnl", Surface::Accounting, "P&L Statement", Menu::Rail("P&L Statement"), "portal.read", "accounting.read", Kind::Screen(mount::<accounting::Pnl>)),
    entry("accounting-receipt-scanner", "/portal/accounting/receipt-scanner", Surface::Accounting, "Receipt Scanner", Menu::Rail("Receipt Scanner"), "portal.read", "accounting.read", Kind::Screen(mount::<accounting::ReceiptScanner>)),
    entry("marketing", "/portal/marketing", Surface::Marketing, "Dashboard", Menu::Rail("Dashboard"), "portal.read", "property.read", Kind::LegacyPortal("marketing")),
    entry("marketing-syndication", "/portal/marketing/syndication", Surface::Marketing, "Syndication", Menu::Rail("Syndication"), "portal.read", "property.read", Kind::LegacyPortal("marketing-syndication")),
    entry("property-admin", "/portal/property-admin", Surface::Ops, "Data Workbench", Menu::Rail("Records"), "portal.read", "property.read", Kind::LegacyIsland("property-admin")),
    entry("property-media", "/portal/property-media", Surface::Ops, "Property Media", Menu::Rail("Listing Media"), "portal.read", "property.read", Kind::LegacyPortal("property-media")),
    entry("tech", "/portal/tech", Surface::Tech, "Cockpit", Menu::Rail("Cockpit"), "tech.access", "tech.access", Kind::LegacyIsland("tech")),
    entry("storyboard", "/portal/storyboard", Surface::Tech, "Story Board", Menu::Rail("Story Board"), "tech.access", "tech.access", Kind::LegacyPortal("storyboard")),
    entry("design-lab", "/portal/design-lab", Surface::Tech, "UI Lab", Menu::Rail("UI Lab"), "tech.access", "tech.access", Kind::LegacyIsland("design-lab")),
    entry("system-health", "/portal/system-health", Surface::Support, "System Health", Menu::Rail("System Health"), "portal.read", "portal.read", Kind::Screen(mount::<SystemHealth>)),
    entry("db-test", "/portal/db-test", Surface::Support, "DB Test", Menu::Rail("DB Test"), "portal.read", "portal.read", Kind::Screen(mount::<DbTest>)),
    entry("whatsapp-meta", "/portal/admin/whatsapp-meta", Surface::Support, "WhatsApp Diagnostic", Menu::Rail("WhatsApp Diagnostic"), "portal.read", "portal.read", Kind::Screen(mount::<WhatsAppMeta>)),
    entry("whatsapp-coexistence", "/portal/admin/whatsapp-coexistence", Surface::Support, "WhatsApp Activation", Menu::Rail("WhatsApp Activation"), "portal.read", "portal.read", Kind::External),
    entry("site-whatsapp", "/whatsapp", Surface::Support, "WhatsApp Public Page", Menu::Rail("WhatsApp Public Page"), "portal.read", "portal.read", Kind::Screen(mount::<WhatsAppPublic>)),
    entry("site-video", "/video", Surface::Support, "Mux Video Test", Menu::Rail("Mux Video Test"), "portal.read", "portal.read", Kind::Screen(mount::<RowsScreen<VideoTest>>)),
    entry("review", "/review/:token/:page", Surface::Support, "Review", Menu::None, "", "", Kind::Screen(mount::<RowsScreen<Review>>)),
    entry("security", "/portal/settings", Surface::Support, "Security", Menu::Rail("Security"), "settings.read", "security.principal.read", Kind::Screen(mount::<Security>)),
    entry("site-buyers", "/buyers", Surface::Site, "Buyers", Menu::Header("Buyers"), "", "", Kind::LegacySite),
    entry("site-sellers", "/sellers", Surface::Site, "Sellers", Menu::Header("Sellers"), "", "", Kind::LegacySite),
    entry("site-services", "/services", Surface::Site, "Services", Menu::Header("Services"), "", "", Kind::LegacySite),
    entry("site-guide", "/guide", Surface::Site, "Guide", Menu::Header("Guide"), "", "", Kind::LegacySite),
    entry("site-about", "/about", Surface::Site, "About", Menu::Header("About"), "", "", Kind::LegacySite),
    entry("site-faq", "/faq", Surface::Site, "FAQ", Menu::Header("FAQ"), "", "", Kind::LegacySite),
    entry("site-contact", "/contact", Surface::Site, "Contact", Menu::Header("Contact"), "", "", Kind::LegacySite),
    entry("settings-authorities", "/portal/settings/authorities", Surface::Support, "Authorities", Menu::None, "portal.read", "", Kind::Screen(mount::<RowsScreen<Authorities>>)).of("security"),
    entry("settings-roles", "/portal/settings/roles", Surface::Support, "Roles", Menu::None, "portal.read", "", Kind::Screen(mount::<RowsScreen<Roles>>)).of("security"),
    entry("settings-users", "/portal/settings/users", Surface::Support, "Users", Menu::None, "portal.read", "", Kind::Screen(mount::<SecurityUsers>)).of("security"),
    entry("attention", "/portal/attention", Surface::Core, "Attention", Menu::None, "portal.read", "", Kind::LegacyPortal("attention")).of("dashboard"),
    entry("activity", "/portal/activity", Surface::Core, "Activity", Menu::None, "portal.read", "", Kind::LegacyPortal("activity")).of("dashboard"),
    entry("client-record", "/portal/clients/:personId", Surface::Core, "Client", Menu::None, "portal.read", "", Kind::Screen(mount::<ClientRecord>)).of("clients"),
    entry("deal-record", "/portal/deals/:dealId", Surface::Core, "Deal", Menu::None, "portal.read", "", Kind::LegacyPortal("deal-record")).of("deals"),
    entry("form-record", "/portal/forms/:formId", Surface::Core, "Form", Menu::None, "portal.read", "", Kind::External).of("forms"),
    entry("workflow-record", "/portal/workflows/:instanceId", Surface::Core, "Workflow instance", Menu::None, "portal.read", "", Kind::LegacyPortal("workflow-record")).of("workflows"),
    entry("property-record", "/portal/property-admin/:propertyId", Surface::Ops, "Property record", Menu::None, "portal.read", "", Kind::LegacyPortal("property-record")).of("property-admin"),
    entry("story-record", "/portal/storyboard/:id", Surface::Tech, "Story", Menu::None, "portal.read", "", Kind::LegacyPortal("story-record")).of("storyboard"),
    entry("trace-record", "/portal/tech/flight-recorder/:instanceId", Surface::Tech, "Trace", Menu::None, "portal.read", "", Kind::LegacyIsland("trace-record")).of("tech"),
    entry("site-home", "/", Surface::Site, "Home", Menu::None, "", "", Kind::LegacySite),
    entry("site-properties", "/properties", Surface::Site, "Properties", Menu::None, "", "", Kind::LegacySite).of("site-buyers"),
    entry("site-property-detail", "/properties/:slug", Surface::Site, "Property", Menu::None, "", "", Kind::LegacySite).of("site-buyers"),
    entry("site-privacy", "/privacy", Surface::Site, "Privacy", Menu::None, "", "", Kind::LegacySite),
    entry("site-favorites", "/favorites", Surface::Site, "Favorites", Menu::None, "", "", Kind::LegacySite),
    entry("site-account", "/account", Surface::Site, "Account", Menu::None, "", "", Kind::Screen(mount::<Account>)),
    entry("login", "/login", Surface::Site, "Login", Menu::None, "", "", Kind::External),
    entry("login-recovery", "/login/recovery", Surface::Site, "Login recovery", Menu::None, "", "", Kind::LegacySite),
    entry("login-unauthorized", "/login/unauthorized", Surface::Site, "Login unauthorized", Menu::None, "", "", Kind::LegacySite),
    entry("auth-error", "/auth/error", Surface::Site, "Auth error", Menu::None, "", "", Kind::External),
    entry("portal-root", "/portal", Surface::Core, "Portal", Menu::None, "portal.read", "", Kind::External),
];

/// The screen a path is served by, with its `:param` values. Exact segments win over params, so
/// `/portal/property-admin/new` would beat `/portal/property-admin/:propertyId` if both existed.
pub fn resolve(path: &str) -> Option<(&'static Entry, Vec<(&'static str, String)>)> {
    let path = normalize(path);
    let segments: Vec<&str> = split(&path).collect();
    let mut best: Option<(usize, &'static Entry, Vec<(&'static str, String)>)> = None;
    for entry in ENTRIES {
        let pattern: Vec<&str> = split(entry.path).collect();
        if pattern.len() != segments.len() {
            continue;
        }
        let mut params = Vec::new();
        let mut exact = 0;
        let matched = pattern.iter().zip(&segments).all(|(want, got)| {
            if let Some(name) = want.strip_prefix(':') {
                params.push((name, decode_segment(got)));
                true
            } else if want == got {
                exact += 1;
                true
            } else {
                false
            }
        });
        if matched && best.as_ref().is_none_or(|(score, _, _)| exact > *score) {
            best = Some((exact, entry, params));
        }
    }
    best.map(|(_, entry, params)| (entry, params))
}

/// The area a path belongs to even when no screen serves it, so a 404 is drawn in the right chrome.
pub fn area_of(path: &str) -> Area {
    if normalize(path).starts_with("/portal") {
        Area::Portal
    } else {
        Area::Site
    }
}

pub fn by_key(key: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|entry| entry.key == key)
}

/// Whether a menu item for `item` is the current destination at `path`: it is the screen there, or that screen's
/// parent (a client record highlights Clients).
pub fn is_current(item: &Entry, path: &str) -> bool {
    resolve(path).is_some_and(|(here, _)| here.key == item.key || here.parent == Some(item.key))
}

/// The parent screen of the screen at `path`, for the template's back link.
pub fn parent_of(path: &str) -> Option<&'static Entry> {
    resolve(path)
        .and_then(|(here, _)| here.parent)
        .and_then(by_key)
}

/// The public site's header, in menu order.
pub fn header_items() -> impl Iterator<Item = (&'static str, &'static Entry)> {
    ENTRIES.iter().filter_map(|entry| match entry.menu {
        Menu::Header(label) => Some((label, entry)),
        _ => None,
    })
}

/// One surface's rail for this actor, in menu order.
pub fn rail_items(surface: Surface, actor: &Actor) -> Vec<(&'static str, &'static Entry)> {
    ENTRIES
        .iter()
        .filter(|entry| entry.surface == surface && item_visible(entry, actor))
        .filter_map(|entry| match entry.menu {
            Menu::Rail(label) => Some((label, entry)),
            _ => None,
        })
        .collect()
}

/// The operating worlds whose top-nav capsule shows, in display order (SUPPORT before TECH, as the registry orders
/// them).
pub fn visible_surfaces(actor: &Actor) -> Vec<Surface> {
    SURFACE_ORDER
        .iter()
        .copied()
        .filter(|surface| surface_visible(*surface, actor))
        .collect()
}

const SURFACE_ORDER: &[Surface] = &[
    Surface::Core,
    Surface::Accounting,
    Surface::Marketing,
    Surface::Ops,
    Surface::Support,
    Surface::Tech,
];

/// The surface-level rules (`operating-shell.tsx`): TECH is ROOT-only and needs `tech.access`; every surface needs an
/// internal account and at least one reachable rail item (ROOT reaches all).
fn surface_visible(surface: Surface, actor: &Actor) -> bool {
    let (min_level, access_authority) = match surface {
        Surface::Tech => (Some(Level::Root), "tech.access"),
        Surface::Site => return false,
        _ => (None, ""),
    };
    if !actor.holds_authority(access_authority) {
        return false;
    }
    if min_level.is_some_and(|required| actor.level() < required) {
        return false;
    }
    actor.internal()
        && (actor.is_root()
            || ENTRIES.iter().any(|entry| {
                entry.surface == surface
                    && matches!(entry.menu, Menu::Rail(_))
                    && !entry.entitlement.is_empty()
                    && actor
                        .entitlement_codes
                        .iter()
                        .any(|held| held == entry.entitlement)
            }))
}

fn item_visible(entry: &Entry, actor: &Actor) -> bool {
    actor.holds_authority(entry.authority)
        && actor.internal()
        && actor.holds_entitlement(entry.entitlement)
}

fn normalize(path: &str) -> String {
    let path = path.split(['?', '#']).next().unwrap_or("/");
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

fn split(path: &str) -> impl Iterator<Item = &str> {
    path.split('/').filter(|segment| !segment.is_empty())
}

fn decode_segment(segment: &str) -> String {
    crate::app::screen::parse_query(&format!("x={segment}"))
        .remove("x")
        .unwrap_or_else(|| segment.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// ROOT as the portal layout hands it over: the level AND the authorities its roles carry. The rules check the
    /// authority first and do not exempt ROOT from it, exactly as `operating-shell.tsx` did.
    fn root() -> Actor {
        Actor {
            level: Some(Level::Root),
            account_type: "internal".into(),
            authority_codes: ["portal.read", "deal.read", "settings.read", "tech.access"]
                .map(String::from)
                .to_vec(),
            ..Actor::default()
        }
    }

    #[test]
    fn keys_and_paths_are_unique() {
        let keys: BTreeSet<_> = ENTRIES.iter().map(|entry| entry.key).collect();
        let paths: BTreeSet<_> = ENTRIES.iter().map(|entry| entry.path).collect();
        assert_eq!(keys.len(), ENTRIES.len(), "duplicate key");
        assert_eq!(paths.len(), ENTRIES.len(), "duplicate path");
    }

    #[test]
    fn paths_resolve_with_their_params_and_exact_segments_win() {
        let (entry, params) = resolve("/portal/clients/abc%20123?tab=history").unwrap();
        assert_eq!(entry.key, "client-record");
        assert_eq!(params, vec![("personId", "abc 123".to_string())]);
        assert_eq!(resolve("/portal/clients/").unwrap().0.key, "clients");
        assert_eq!(resolve("/").unwrap().0.key, "site-home");
        let (review, params) = resolve("/review/tok/2").unwrap();
        assert_eq!((review.key, params.len()), ("review", 2));
        assert!(resolve("/portal/no-such-screen").is_none());
        assert_eq!(area_of("/portal/no-such-screen"), Area::Portal);
    }

    #[test]
    fn every_next_page_has_a_registry_entry_and_every_entry_a_page() {
        // THE DEEP-LINK GATE. A registry path with no page 404s on refresh; a page with no entry renders "not found"
        // inside the app. Both used to be possible silently.
        let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
        let mut pages = BTreeSet::new();
        let mut stack = vec![app.clone()];
        while let Some(dir) = stack.pop() {
            for item in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = item.path();
                if path.is_dir() {
                    // Route groups and private folders are not URL segments; `api` is not a page.
                    let name = item.file_name().to_string_lossy().to_string();
                    if name != "api" && !name.starts_with('_') {
                        stack.push(path);
                    }
                } else if item.file_name() == "page.tsx" {
                    let route = path
                        .parent()
                        .unwrap()
                        .strip_prefix(&app)
                        .unwrap()
                        .to_string_lossy()
                        .to_string();
                    let route = format!("/{route}");
                    let route: String = route
                        .split('/')
                        .map(|segment| {
                            match segment.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                                Some(param) => format!(":{param}"),
                                None => segment.to_string(),
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("/");
                    pages.insert(if route == "/" {
                        route
                    } else {
                        route.trim_end_matches('/').to_string()
                    });
                }
            }
        }
        let entries: BTreeSet<String> =
            ENTRIES.iter().map(|entry| entry.path.to_string()).collect();
        let missing_entry: Vec<_> = pages.difference(&entries).collect();
        let missing_page: Vec<_> = entries.difference(&pages).collect();
        assert!(
            missing_entry.is_empty(),
            "pages with no registry entry: {missing_entry:?}"
        );
        assert!(
            missing_page.is_empty(),
            "registry entries with no page: {missing_page:?}"
        );
    }

    #[test]
    fn the_rail_is_the_designed_menu_in_its_order() {
        let labels = |surface| {
            rail_items(surface, &root())
                .into_iter()
                .map(|(label, _)| label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(Surface::Core),
            [
                "Cockpit",
                "Clients",
                "Projects",
                "Contracts",
                "Cabinet",
                "Workflows",
                "Forms",
                "Seller Strategy"
            ]
        );
        assert_eq!(
            labels(Surface::Accounting),
            [
                "Dashboard",
                "Receivables",
                "Expenses",
                "P&L Statement",
                "Receipt Scanner"
            ]
        );
        assert_eq!(labels(Surface::Ops), ["Records", "Listing Media"]);
        assert_eq!(
            labels(Surface::Support),
            [
                "System Health",
                "DB Test",
                "WhatsApp Diagnostic",
                "WhatsApp Activation",
                "WhatsApp Public Page",
                "Mux Video Test",
                "Security"
            ]
        );
        assert_eq!(labels(Surface::Tech), ["Cockpit", "Story Board", "UI Lab"]);
        let header: Vec<_> = header_items().map(|(label, _)| label).collect();
        assert_eq!(
            header,
            ["Buyers", "Sellers", "Services", "Guide", "About", "FAQ", "Contact"]
        );
    }

    #[test]
    fn visibility_follows_the_registry_rules() {
        assert_eq!(visible_surfaces(&root()).len(), 6);
        let user = Actor {
            level: Some(Level::User),
            account_type: "internal".into(),
            authority_codes: vec!["portal.read".into()],
            entitlement_codes: vec!["person.read".into()],
        };
        assert_eq!(
            visible_surfaces(&user),
            vec![Surface::Core],
            "one reachable item opens CORE only; TECH is ROOT-only"
        );
        let rail: Vec<_> = rail_items(Surface::Core, &user)
            .into_iter()
            .map(|(label, _)| label)
            .collect();
        assert_eq!(rail, ["Clients"]);
        let guest = Actor {
            account_type: "external".into(),
            ..user
        };
        assert!(
            visible_surfaces(&guest).is_empty(),
            "an external account sees no operating world"
        );
    }

    #[test]
    fn a_retired_screen_is_never_in_a_menu() {
        // The old table marked some routes RETIRED from the nav (the code left in place). The registry keeps that
        // decision: such a screen may exist, but no menu offers it.
        for entry in ENTRIES {
            let retired = crate::model::screen(entry.key)
                .is_some_and(|screen| matches!(screen.nav, crate::model::Nav::Retired));
            assert!(
                !(retired && entry.menu != Menu::None),
                "{} is retired but in a menu",
                entry.key
            );
        }
    }

    #[test]
    fn every_drill_in_names_a_real_parent_and_highlights_it() {
        for entry in ENTRIES {
            if let Some(parent) = entry.parent {
                let parent = by_key(parent)
                    .unwrap_or_else(|| panic!("{}: parent {parent} is not registered", entry.key));
                assert_eq!(
                    parent.area(),
                    entry.area(),
                    "{} and its parent must share a chrome",
                    entry.key
                );
            }
        }
        let clients = by_key("clients").unwrap();
        assert!(
            is_current(clients, "/portal/clients/abc"),
            "a client record highlights Clients"
        );
        assert!(is_current(clients, "/portal/clients"));
        assert!(!is_current(clients, "/portal/deals"));
        assert_eq!(
            parent_of("/portal/deals/42").map(|entry| entry.key),
            Some("deals")
        );
        assert!(parent_of("/portal/deals").is_none());
    }

    #[test]
    fn every_surface_home_is_a_registered_screen() {
        for surface in SURFACE_ORDER {
            let home = crate::navigation::home_path(*surface);
            assert!(
                resolve(home).is_some(),
                "{surface:?} home {home} is not registered"
            );
        }
    }

    /// THE CUTOVER LEDGER. Screens still on the old loop. This number only goes down; at zero, the legacy kinds and the
    /// old loop are deleted. Update it downward when a screen is ported — never upward.
    #[test]
    fn legacy_count_only_goes_down() {
        let legacy = ENTRIES
            .iter()
            .filter(|entry| {
                matches!(
                    entry.kind,
                    Kind::LegacyPortal(_) | Kind::LegacyIsland(_) | Kind::LegacySite
                )
            })
            .count();
        assert!(
            legacy <= LEGACY_CEILING,
            "{legacy} legacy screens; the ceiling is {LEGACY_CEILING}"
        );
    }

    const LEGACY_CEILING: usize = 34;
}
