//! The `Screen` trait — the abstract class every screen implements. See docs/agent/UI-SCREEN-ARCHITECTURE.md.
//!
//! A screen is behaviour only: its own `Model`, its own `Msg`, and three functions. Its IDENTITY (key, path, surface,
//! nav label, entitlement) is its line in the registry (`app/registry.rs`), because the router, the nav and the headless
//! walk are generated from that one table.
//!
//! THE THREE FUNCTIONS ARE PURE. `init` and `update` return a `Cmd` describing what should happen; they never fetch,
//! touch storage or the DOM. `view` turns the model into `Html`. That is what lets every screen be unit-tested with plain
//! `cargo test`, and what keeps loading, errors and HTTP identical everywhere: the shell does them, once.

use std::collections::BTreeMap;

use yew::{Callback, Html};

use crate::app::cmd::Cmd;
use crate::navigation::Actor;

pub trait Screen: 'static {
    /// Everything this screen remembers. Nothing else in the app can see it.
    type Model: Default + Clone + PartialEq + 'static;
    /// Everything that can happen on this screen: user intents and answers to its own commands.
    type Msg: 'static;

    /// The screen was opened (or reopened for another record). Returns its first state and first commands.
    fn init(ctx: &ScreenCtx) -> (Self::Model, Cmd<Self::Msg>);

    /// A message arrived. Change the model; return what should happen next.
    fn update(model: &mut Self::Model, msg: Self::Msg, ctx: &ScreenCtx) -> Cmd<Self::Msg>;

    /// Draw the model. Controls send messages through `link`.
    fn view(model: &Self::Model, ctx: &ScreenCtx, link: &Link<Self::Msg>) -> Html;

    /// The URL's QUERY changed while this screen stays open (a `?tab=`, a `?selected=`): `ctx` already has the new
    /// query. The screen keeps its state; return what should happen (usually nothing — the view reads the query — or
    /// a read for the newly selected row). A new PATH or record id is not this: that restarts the screen with `init`.
    fn url_changed(_model: &mut Self::Model, _ctx: &ScreenCtx) -> Cmd<Self::Msg> {
        Cmd::none()
    }
}

/// What the shell tells a screen about where it is. Read-only.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScreenCtx {
    /// Who is signed in, as the portal layout handed it over. Empty on the public site.
    pub actor: Actor,
    /// The record id a record route carries (`/portal/clients/:id`).
    pub id: Option<String>,
    /// The URL's query, decoded.
    pub query: BTreeMap<String, String>,
    /// The path this screen was opened at.
    pub path: String,
    /// What the signed-in user may do, as the Rust security service answered for this portal visit (the shell reads it
    /// once). `None` until it arrives, and on the public site. UI VISIBILITY ONLY: the server authorizes every call.
    pub grants: Option<crate::model::PortalEntitlements>,
}

impl ScreenCtx {
    /// Whether to offer `action`. Mirrors the server's rule: internal accounts only; managing entitlements and roles is
    /// ROOT's; everything else is ROOT or a held entitlement. Nothing is offered before the grants arrive.
    pub fn can(&self, action: &str) -> bool {
        self.grants.as_ref().is_some_and(|grants| {
            // The same list and account rule the server enforces (`domain::security`); this only decides what to offer.
            if grants.account_type != domain::security::INTERNAL_ACCOUNT {
                false
            } else if domain::security::is_root_only(action) {
                grants.is_root
            } else {
                grants.security_level == "ROOT"
                    || grants.entitlement_codes.iter().any(|code| code == action)
            }
        })
    }

    pub fn query(&self, key: &str) -> Option<&str> {
        self.query
            .get(key)
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
    }
}

/// How a view sends messages. A thin wrapper so views never hold a raw component scope.
pub struct Link<Msg: 'static> {
    send: Callback<Msg>,
}

impl<Msg: 'static> Link<Msg> {
    pub fn new(send: Callback<Msg>) -> Self {
        Self { send }
    }

    /// A callback for an event handler: the event becomes a message.
    pub fn callback<E: 'static>(&self, to_msg: impl Fn(E) -> Msg + 'static) -> Callback<E> {
        let send = self.send.clone();
        Callback::from(move |event: E| send.emit(to_msg(event)))
    }

    pub fn send(&self, msg: Msg) {
        self.send.emit(msg);
    }
}

impl<Msg: 'static> Clone for Link<Msg> {
    fn clone(&self) -> Self {
        Self {
            send: self.send.clone(),
        }
    }
}

/// Parse a query string (`?a=1&b=two%20words`) into a map. Pure, so it is tested here.
///
/// A part that does not decode (a stray `%`, a truncated escape, bytes that are not UTF-8 — a mangled marketing link) is
/// KEPT as written rather than dropped: a `propertyId` that arrives slightly damaged should still reach the screen,
/// which can refuse it by name, instead of vanishing without a trace.
pub fn parse_query(query: &str) -> BTreeMap<String, String> {
    query
        .trim_start_matches('?')
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            let decode = |text: &str| {
                let spaced = text.replace('+', " ");
                percent_decode(&spaced).unwrap_or(spaced)
            };
            (decode(key), decode(value))
        })
        .collect()
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {

    /// The portal offers a root-only action exactly when the server would allow it: the same list
    /// (`domain::security::ROOT_ONLY_ACTIONS`), never a copy.
    #[test]
    fn root_only_actions_are_offered_to_root_alone() {
        let ctx = |is_root: bool, level: &str| ScreenCtx {
            grants: Some(crate::model::PortalEntitlements {
                account_type: domain::security::INTERNAL_ACCOUNT.into(),
                security_level: level.into(),
                is_root,
                entitlement_codes: domain::security::ROOT_ONLY_ACTIONS.iter().map(|code| code.to_string()).collect(),
            }),
            ..ScreenCtx::default()
        };
        for action in domain::security::ROOT_ONLY_ACTIONS {
            assert!(!ctx(false, "BUSINESS_POWER_USER").can(action), "{action}: holding the grant is not enough");
            assert!(ctx(true, "ROOT").can(action), "{action}: root may");
        }
        let external = ScreenCtx {
            grants: Some(crate::model::PortalEntitlements { account_type: "external".into(), is_root: true, ..Default::default() }),
            ..ScreenCtx::default()
        };
        assert!(!external.can(domain::security::ROLE_MANAGE), "an external account is offered nothing");
    }

    #[test]
    fn a_part_that_does_not_decode_is_kept_as_written() {
        let query = parse_query("?propertyId=villa%2&ref=50%&name=caf%C3%A9&bad=%FF");
        assert_eq!(query.get("propertyId").map(String::as_str), Some("villa%2"), "a truncated escape");
        assert_eq!(query.get("ref").map(String::as_str), Some("50%"), "a stray percent sign");
        assert_eq!(query.get("name").map(String::as_str), Some("café"), "a good escape still decodes");
        assert_eq!(query.get("bad").map(String::as_str), Some("%FF"), "bytes that are not UTF-8");
    }
    use super::*;

    #[test]
    fn the_query_is_decoded_and_blank_values_read_as_absent() {
        let query =
            parse_query("?propertyId=abc-1&requestType=private_viewing&note=two+words%21&empty=");
        let ctx = ScreenCtx {
            query,
            ..ScreenCtx::default()
        };
        assert_eq!(ctx.query("propertyId"), Some("abc-1"));
        assert_eq!(ctx.query("note"), Some("two words!"));
        assert_eq!(ctx.query("empty"), None);
        assert_eq!(ctx.query("missing"), None);
    }
}
