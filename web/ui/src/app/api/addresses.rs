//! Auth form actions, link addresses, file endpoints, property media commands, and the public site's reads and intake.

#[allow(unused_imports)]
use super::*;

/// Sign-in and sign-out are full-page form posts (the server sets or clears the session cookie and redirects), not
/// fetches — but their URLs still live here, with every other one.
pub mod auth {
    pub const SIGN_OUT: &str = "/api/auth/signout";
    pub const SIGN_IN_GOOGLE: &str = "/api/auth/signin/google";
    pub const EMAIL_CODE_CALLBACK: &str = "/api/auth/callback/email-code";
    pub const BREAK_GLASS_CALLBACK: &str = "/api/auth/callback/break-glass";

    /// Google sign-in, returning to `back` (a portal path) afterwards.
    pub fn sign_in_google(back: &str) -> String {
        format!("{SIGN_IN_GOOGLE}?callbackUrl={}", encode(back))
    }

    fn encode(text: &str) -> String {
        text.bytes()
            .map(|byte| match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                    (byte as char).to_string()
                }
                _ => format!("%{byte:02X}"),
            })
            .collect()
    }
}

/// Addresses a page LINKS to rather than fetches — an image's `src`, a document's download `href`, a form's `action`.
/// They are URLs all the same, so they live here.
pub mod links {
    /// A stored photograph (`?size=card|thumb` picks a derived copy; none is the web copy).
    pub fn media(media_id: &str) -> String {
        format!("/api/media/{media_id}")
    }

    /// A property document for the public site.
    pub fn property_document(document_id: &str) -> String {
        format!("/api/media/documents/{document_id}")
    }

    /// A Vault document's PDF: the executed copy when `signed`, the issued one otherwise.
    pub fn vault_document(document_id: &str, signed: bool) -> String {
        format!(
            "/api/portal/documents/{document_id}/file{}",
            if signed { "?artifact=signed" } else { "" }
        )
    }

    /// A Vault document's signature audit trail.
    pub fn vault_audit(document_id: &str) -> String {
        format!("/api/portal/documents/{document_id}/file?artifact=audit")
    }

    /// The Seller Strategy PDF (a form post).
    pub const SELLER_STRATEGY_PDF: &str = "/api/portal/seller-strategy/pdf";
}

/// Where property photographs are sent, in pieces (`Cmd::upload`).
pub struct PropertyMediaChunked;

impl FileEndpoint for PropertyMediaChunked {
    fn path(&self) -> String {
        "/api/property-media/chunked".into()
    }
}

/// A project document's signed copy: the PDF and the date it was signed (`Cmd::post_form`).
pub struct ProjectSignedCopy {
    pub document_id: String,
}

impl FileEndpoint for ProjectSignedCopy {
    fn path(&self) -> String {
        format!("/api/portal/projects/documents/{}/signed", self.document_id)
    }
}

/// Make one of a property's photographs its hero (the previous hero returns to the gallery).
pub struct PropertyHero {
    pub property_id: String,
    pub media_id: String,
}

impl Endpoint for PropertyHero {
    const METHOD: Method = Method::Post;
    type Response = serde_json::Value;
    fn path(&self) -> String {
        "/api/property-media/hero".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "propertyId": self.property_id, "mediaId": self.media_id }))
    }
}

/// FIND by catastro: the other record for that parcel is merged into this property (see the server's
/// `merge_parcel_record`). Answers `{ merged, mergedName }`.
pub struct PropertyMergeParcel {
    pub property_id: String,
    pub catastro: String,
}

impl Endpoint for PropertyMergeParcel {
    const METHOD: Method = Method::Post;
    type Response = serde_json::Value;
    fn path(&self) -> String {
        "/api/portal/property/merge-parcel".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "propertyId": self.property_id, "catastro": self.catastro }))
    }
}

/// A contract known to be signed, its PDF still to come: recorded as sent, and the project's signing step done.
pub struct ProjectDocumentSignedCopyToCome {
    pub document_id: String,
    pub project_id: String,
    pub signed_at: String,
}

impl Endpoint for ProjectDocumentSignedCopyToCome {
    const METHOD: Method = Method::Post;
    type Response = serde_json::Value;
    fn path(&self) -> String {
        format!(
            "/api/portal/projects/documents/{}/signed-copy-to-come",
            self.document_id
        )
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "projectId": self.project_id, "signedAt": self.signed_at }))
    }
}

/// Take a photograph off a property (deleted with its copies unless another property shows it).
pub struct PropertyMediaRemove {
    pub property_id: String,
    pub media_id: String,
}

impl Endpoint for PropertyMediaRemove {
    const METHOD: Method = Method::Post;
    type Response = serde_json::Value;
    fn path(&self) -> String {
        "/api/property-media/remove".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "propertyId": self.property_id, "mediaId": self.media_id }))
    }
}

/// Listing Media: listings with their photo counts, one opened when `selected` is set. Answers `{ listingMedia: ... }`.
pub struct ListingMediaRead {
    pub selected: Option<String>,
    pub search: String,
    /// 0-based.
    pub page: usize,
}

impl Endpoint for ListingMediaRead {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        let mut path = format!(
            "/api/portal/rust-ui/listing-media?page={}&search={}",
            self.page,
            encode(&self.search)
        );
        if let Some(selected) = self.selected.as_deref().filter(|id| !id.is_empty()) {
            path.push_str(&format!("&selected={}", encode(selected)));
        }
        path
    }
}

/// A public page's content (hero, blocks, listings, guide, FAQ), as the site reads it. `scope` is the record a page is
/// about (a property's slug). Answers the page itself.
pub struct PublicPage {
    pub screen: &'static str,
    pub scope: Option<String>,
}

impl Endpoint for PublicPage {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PageContent;
    fn path(&self) -> String {
        match self.scope.as_deref().filter(|scope| !scope.is_empty()) {
            Some(scope) => format!(
                "/api/rust-ui/public-page?screen={}&scope={}",
                self.screen,
                encode(scope)
            ),
            None => format!("/api/rust-ui/public-page?screen={}", self.screen),
        }
    }
}

/// A website lead (contact form, quick enquiries). The relay hands it to the one intake pipeline. Answers
/// `{ accepted, status }`.
pub struct WebsiteIntake {
    pub body: serde_json::Value,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct IntakeAnswer {
    pub accepted: bool,
}

/// The Google Maps browser key for the JS-API property map. Answers
/// `{ key }`, with `key` null when unconfigured — the page shows its waiting
/// state instead of a map.
pub struct MapsKey;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct MapsKeyAnswer {
    pub key: Option<String>,
}

impl Endpoint for MapsKey {
    const METHOD: Method = Method::Get;
    type Response = MapsKeyAnswer;
    fn path(&self) -> String {
        "/api/rust-ui/maps-key".into()
    }
}

/// The public signer edge: the session behind one signing link, then the
/// recipient's own actions. The token travels in the body, never the URL.
pub struct SignerSessionPost {
    pub token: String,
}

impl Endpoint for SignerSessionPost {
    const METHOD: Method = Method::Post;
    type Response = crate::model::SignerSession;
    fn path(&self) -> String {
        "/v1/signer/session".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "accessToken": self.token }))
    }
}

/// One signer command through the edge: open, consent, field, complete or
/// decline. Answers the durable command result.
pub struct SignerActPost {
    pub action: &'static str,
    pub body: serde_json::Value,
}

impl Endpoint for SignerActPost {
    const METHOD: Method = Method::Post;
    type Response = serde_json::Value;
    fn path(&self) -> String {
        format!("/v1/signer/{}", self.action)
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(self.body.clone())
    }
}

impl Endpoint for WebsiteIntake {
    const METHOD: Method = Method::Post;
    type Response = IntakeAnswer;
    fn path(&self) -> String {
        "/api/rust-ui/website-intake".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(self.body.clone())
    }
}
