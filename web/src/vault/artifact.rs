//! The vault's artifact port, implemented with the Rust form renderer.
//!
//! WHAT THIS REPLACES. Five places in the API attach `UnavailableVaultArtifactPort`, whose whole behaviour is to answer
//! "Vault artifact renderer is not configured on this transport". That stub is why an issued document could not be
//! produced: the command that versions it, resolves its participants and writes its receipts was already here, and only
//! the renderer behind it was missing.
//!
//! THE TEMPLATES ARE READ FROM DISK, NOT COMPILED IN. They are the canonical authoring format and they are versioned
//! files: a live document re-renders from the version it was issued under, and a new version is an XML file dropped in
//! the directory. So the library is reloaded whenever the directory changes — an edited or added template stays live
//! without a restart, and a run of documents still reads the files once.

use async_trait::async_trait;
use model::forms_template::{templates_dir, TemplateLibrary};
use model::{VaultArtifactFailure, VaultCommandOutcome, VaultRenderRequest, VaultRenderedArtifact};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::vault::forms_render::{render_form, Logo, Participant, RenderedForm, COORDINATE_SPACE};
use crate::vault::VaultArtifactPort;

/// The brand wordmark, relative to the process's working directory unless the environment names another path.
pub const LOGO_ENV: &str = "FORMS_BRAND_LOGO";
pub const DEFAULT_LOGO_PATH: &str = "public/brand/CLLOGO.png";

/// Renders issued documents from the XML templates the repository authors.
pub struct FormArtifactPort {
    templates_dir: PathBuf,
    logo_path: PathBuf,
    library: Mutex<CachedLibrary>,
    logo: Mutex<Option<CachedLogo>>,
}

#[derive(Default)]
struct CachedLibrary {
    /// The directory's stamp when this library was read: how many template files it held, and the newest of their
    /// modifications. A count catches a file being added or removed even when two writes land in the same second.
    read_at: Option<(usize, SystemTime)>,
    library: Option<Arc<TemplateLibrary>>,
}

#[derive(Clone)]
struct CachedLogo {
    /// The asset's modification time, so a replaced wordmark is picked up rather than kept for the process's life.
    modified: Option<SystemTime>,
    logo: Option<Logo>,
}

impl Default for FormArtifactPort {
    fn default() -> Self {
        Self::new(templates_dir(), default_logo_path())
    }
}

/// The wordmark, from the environment when it names one, otherwise the repository's public asset.
pub fn default_logo_path() -> PathBuf {
    std::env::var(LOGO_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        // The wordmark is a repository asset like the templates, so it is resolved the same way: the launcher's working
        // directory must not decide whether the document has a wordmark on it.
        .unwrap_or_else(|| model::forms_template::resolve_repo_path(DEFAULT_LOGO_PATH))
}

impl FormArtifactPort {
    pub fn new(templates_dir: PathBuf, logo_path: PathBuf) -> Self {
        Self {
            templates_dir,
            logo_path,
            library: Mutex::new(CachedLibrary::default()),
            logo: Mutex::new(None),
        }
    }

    /// The directory's stamp: how many template files it holds and the newest modification among them.
    fn directory_stamp(&self) -> Option<(usize, SystemTime)> {
        let entries = std::fs::read_dir(&self.templates_dir).ok()?;
        let mut count = 0usize;
        let mut newest: Option<SystemTime> = None;
        for entry in entries.flatten() {
            let path = entry.path();
            let is_xml = path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"));
            if !is_xml {
                continue;
            }
            count += 1;
            if let Ok(modified) = entry.metadata().and_then(|metadata| metadata.modified()) {
                newest = Some(match newest {
                    Some(current) if current > modified => current,
                    _ => modified,
                });
            }
        }
        newest.map(|newest| (count, newest))
    }

    fn library(&self) -> Result<Arc<TemplateLibrary>, VaultArtifactFailure> {
        let stamp = self.directory_stamp();
        let mut cache = self
            .library
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(library) = cache.library.as_ref() {
            if cache.read_at == stamp {
                return Ok(Arc::clone(library));
            }
        }
        let library = TemplateLibrary::load_from_dir(&self.templates_dir).map_err(|error| {
            failure(format!(
                "The form templates could not be read: {}",
                error.message
            ))
        })?;
        let library = Arc::new(library);
        cache.read_at = stamp;
        cache.library = Some(Arc::clone(&library));
        Ok(library)
    }

    /// The wordmark, or `None` when the asset is absent — which draws the text wordmark instead, as the TypeScript
    /// renderer also does.
    fn logo(&self) -> Option<Logo> {
        let modified = std::fs::metadata(&self.logo_path)
            .and_then(|metadata| metadata.modified())
            .ok();
        let mut cache = self
            .logo
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(cached) = cache.as_ref() {
            if cached.modified == modified {
                return cached.logo.clone();
            }
        }
        let logo = std::fs::read(&self.logo_path)
            .ok()
            .and_then(|bytes| Logo::from_png(&bytes));
        *cache = Some(CachedLogo {
            modified,
            logo: logo.clone(),
        });
        logo
    }
}

fn failure(message: impl Into<String>) -> VaultArtifactFailure {
    VaultArtifactFailure {
        outcome: VaultCommandOutcome::PreconditionFailure,
        message: message.into(),
    }
}

/// A file name that survives every filesystem: the characters the TypeScript renderer keeps, with the bound party's
/// name first so a reader recognises the document in a list.
pub fn form_filename(
    template_id: &str,
    template_display_name: &str,
    values: &BTreeMap<String, String>,
) -> String {
    let who = ["buyerName", "sellerName", "clientName"]
        .iter()
        .find_map(|key| {
            values
                .get(*key)
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or(template_display_name);
    format!(
        "{}-{}.pdf",
        file_safe_name(who),
        file_safe_name(template_id)
    )
}

/// `\w`, whitespace and `-` survive, a run of whitespace becomes one `-`, and an empty result is `form`.
pub fn file_safe_name(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric()
                || *character == '_'
                || character.is_whitespace()
                || *character == '-'
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return "form".to_string();
    }
    let mut out = String::with_capacity(trimmed.len());
    let mut pending_space = false;
    for character in trimmed.chars() {
        if character.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space {
            out.push('-');
            pending_space = false;
        }
        out.push(character);
    }
    out
}

/// A participant's name for a signature block: the person's name, or the role when the row has none.
fn participant_name(person: &model::FormSignerPerson) -> String {
    if person.name.trim().is_empty() {
        person.role.clone()
    } else {
        person.name.clone()
    }
}

/// The metadata an issued document's snapshot carries: where its signatures go, in the provider-neutral contract the
/// BoldSign adapter reads (`signatureAnchors`, each with its rectangle and coordinate space).
pub fn render_metadata(rendered: &RenderedForm) -> serde_json::Value {
    let anchors: Vec<serde_json::Value> = rendered
        .signature_anchors
        .iter()
        .map(|anchor| {
            json!({
                "role": anchor.role,
                "slotId": anchor.slot_id,
                "kind": anchor.kind.as_str(),
                "pageIndex": anchor.page_index,
                "pageWidth": anchor.page_width,
                "pageHeight": anchor.page_height,
                "rect": {
                    "x": anchor.rect.x,
                    "y": anchor.rect.y,
                    "width": anchor.rect.width,
                    "height": anchor.rect.height,
                },
                "coordinateSpace": COORDINATE_SPACE,
            })
        })
        .collect();
    // The full evidence, in the shape `parse_applied_signature_slot_ids` proves it from: provenance plus where it landed.
    let applied = serde_json::to_value(&rendered.applied_evidence).unwrap_or_else(|_| json!([]));
    json!({
        "pageCount": rendered.page_count,
        "pageSize": { "width": 612, "height": 792 },
        "coordinateSpace": COORDINATE_SPACE,
        "signatureAnchors": anchors,
        // What the issuer already signed into the PDF. The signing step reads this to leave those blocks alone.
        "appliedSignatures": applied,
    })
}

#[async_trait]
impl VaultArtifactPort for FormArtifactPort {
    async fn render_issued_document(
        &self,
        request: VaultRenderRequest,
    ) -> Result<VaultRenderedArtifact, VaultArtifactFailure> {
        let library = self.library()?;
        let template = library
            .version(&request.template_id, request.template_version)
            .ok_or_else(|| {
                failure(format!(
                    "Template {} v{} is not in the templates directory ({}).",
                    request.template_id,
                    request.template_version,
                    self.templates_dir.display()
                ))
            })?;
        let participants: Vec<Participant> = request
            .participants
            .iter()
            .map(|person| Participant {
                role: person.role.clone(),
                // The slot the participant occupies, as canonicalization assigned it: the anchors and the envelope both
                // speak in slot ids, so a null here would leave a signature line no provider could address.
                slot_id: person.slot_id.clone(),
                name: participant_name(person),
            })
            .collect();
        let logo = self.logo();
        let rendered = render_form(
            template,
            &request.field_values,
            &request.sections,
            request.issued_version,
            &participants,
            logo.as_ref(),
            &request.applied_signatures,
        )
        .map_err(|error| failure(error.message))?;
        let filename = form_filename(&template.id, &template.display_name, &request.field_values);
        // The metadata is built before the bytes are moved out, because it is a function of the same rendering.
        let metadata = render_metadata(&rendered);
        Ok(VaultRenderedArtifact {
            bytes: rendered.bytes,
            filename,
            document_type_label: template.document_type_label.clone(),
            display_name: template.display_name.clone(),
            render_metadata: metadata,
        })
    }
}

/// The port the composition root attaches: the renderer, reading the repository's own paths by default.
pub fn port() -> Arc<dyn VaultArtifactPort> {
    Arc::new(FormArtifactPort::default())
}

/// The process's port.
///
/// ONE INSTANCE, because the only state a port holds is the template library and the decoded wordmark, both cached
/// against the files' own timestamps. A fresh port per request would re-read the directory for every document, and the
/// cache exists precisely so a run of documents reads it once.
pub fn shared() -> Arc<dyn VaultArtifactPort> {
    static SHARED: std::sync::OnceLock<Arc<dyn VaultArtifactPort>> = std::sync::OnceLock::new();
    Arc::clone(SHARED.get_or_init(|| Arc::new(FormArtifactPort::default())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::forms::FormSignerPerson;

    fn repository_path(relative: &str) -> PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
    }

    fn renderer() -> FormArtifactPort {
        FormArtifactPort::new(
            repository_path("../middle/model/forms/templates"),
            repository_path("../public/brand/CLLOGO.png"),
        )
    }

    fn request() -> VaultRenderRequest {
        VaultRenderRequest {
            form_instance_id: "form-1".to_string(),
            contract_id: None,
            template_id: "LISTING-01".to_string(),
            template_version: 4,
            field_values: [
                ("sellerName", "Lisa Penfield"),
                ("property", "Casa Luar"),
                ("propertyLocation", "Culebra, Puerto Rico"),
                ("listPrice", "1250000"),
                ("startDate", "2026-01-15"),
                ("endDate", "2027-01-15"),
                ("listingType", "Exclusive Right to Sell"),
            ]
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
            sections: BTreeMap::new(),
            issued_version: 1,
            participants: vec![
                FormSignerPerson {
                    person_id: Some("p1".to_string()),
                    name: "Lisa Penfield".to_string(),
                    email: Some("lisa@example.com".to_string()),
                    role: "SELLER".to_string(),
                    slot_id: Some("SELLER:1".to_string()),
                },
                FormSignerPerson {
                    person_id: None,
                    name: "Broker".to_string(),
                    email: None,
                    role: "SELLER_BROKER".to_string(),
                    slot_id: Some("SELLER_BROKER:1".to_string()),
                },
            ],
            actor_app_user_id: None,
            issued_at: Some("2026-01-15T00:00:00Z".to_string()),
            applied_signatures: Vec::new(),
        }
    }

    #[tokio::test]
    async fn the_port_renders_an_issued_listing_agreement() {
        let artifact = renderer()
            .render_issued_document(request())
            .await
            .expect("the agreement issues");
        assert!(artifact.bytes.starts_with(b"%PDF-1.4"));
        assert_eq!(artifact.filename, "Lisa-Penfield-LISTING-01.pdf");
        assert_eq!(artifact.document_type_label, "Listing Agreement");
        assert_eq!(artifact.display_name, "Listing Agreement");
        let anchors = artifact.render_metadata["signatureAnchors"]
            .as_array()
            .expect("the snapshot records where signatures go");
        assert!(anchors.len() >= 6, "both parties sign, three regions each");
        assert_eq!(anchors[0]["coordinateSpace"], COORDINATE_SPACE);
        assert!(
            anchors[0]["rect"]["width"].as_f64().unwrap_or_default() > 0.0,
            "an anchor has area"
        );
        assert!(
            artifact.render_metadata["pageCount"]
                .as_u64()
                .unwrap_or_default()
                >= 1
        );
    }

    #[tokio::test]
    async fn an_unknown_template_version_is_refused_with_a_reason() {
        let mut absent = request();
        absent.template_version = 99;
        let error = renderer()
            .render_issued_document(absent)
            .await
            .expect_err("v99 is not authored");
        assert!(
            error.message.contains("LISTING-01 v99"),
            "{}",
            error.message
        );
        assert!(error.message.contains("not in the templates directory"));
        // The refusal is explicit and typed, which is what the test this replaced used to assert about the stub: a
        // missing renderer and a missing template are both precondition failures, never a silent empty document.
        assert_eq!(error.outcome, VaultCommandOutcome::PreconditionFailure);
    }

    #[tokio::test]
    async fn an_added_template_file_is_live_without_a_restart() {
        // The point of reading the directory rather than compiling it in: an authored version is live immediately.
        let directory = std::env::temp_dir().join(format!("cl-templates-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("a temp directory");
        std::fs::copy(
            repository_path("../middle/model/forms/templates/SHOW-INFO.xml"),
            directory.join("SHOW-INFO.xml"),
        )
        .expect("the template is copied");

        let renderer = FormArtifactPort::new(
            directory.clone(),
            repository_path("../public/brand/CLLOGO.png"),
        );
        let mut showing = request();
        showing.template_id = "SHOW-INFO".to_string();
        showing.template_version = 1;
        assert!(
            renderer
                .render_issued_document(showing.clone())
                .await
                .is_ok(),
            "the copied template renders"
        );
        let mut absent = showing.clone();
        absent.template_id = "OFFER-01".to_string();
        assert!(
            renderer.render_issued_document(absent).await.is_err(),
            "a template that is not in the directory does not render"
        );

        // Drop a second template in: no restart, and the very next issue finds it.
        std::fs::copy(
            repository_path("../middle/model/forms/templates/OFFER-01.v2.xml"),
            directory.join("OFFER-01.v2.xml"),
        )
        .expect("the second template is copied");
        let mut offer = request();
        offer.template_id = "OFFER-01".to_string();
        offer.template_version = 2;
        assert!(
            renderer.render_issued_document(offer).await.is_ok(),
            "an added template is live at once"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn the_file_name_carries_the_party_and_the_template() {
        let mut values = BTreeMap::new();
        assert_eq!(
            form_filename("LISTING-01", "Listing Agreement", &values),
            "Listing-Agreement-LISTING-01.pdf"
        );
        values.insert("sellerName".to_string(), "José Pérez".to_string());
        // As in the TypeScript renderer the safe-name filter is ASCII: an accented letter is dropped, not transliterated.
        assert_eq!(
            form_filename("LISTING-01", "Listing Agreement", &values),
            "Jos-Prez-LISTING-01.pdf"
        );
    }
}
