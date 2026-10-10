//! Moved from `forms_template.rs` (move only): parse_section, parse_participants, parse_signatures, parse_template_xml, DEFAULT_TEMPLATES_DIR, TEMPLATES_DIR_ENV, resolve_repo_path, templates_dir, TemplateLibrary, load_from_dir.

#[allow(unused_imports)]
use super::*;

pub(super) fn parse_section(
    element: &Element,
    field_ids: &[String],
    section_ids: &mut Vec<String>,
) -> Result<TemplateSectionDefinition, TemplateXmlError> {
    let name = element.require_attr("id")?.to_string();
    if section_ids.contains(&name) {
        return Err(TemplateXmlError::new(format!(
            "Duplicate section id \"{name}\"."
        )));
    }
    section_ids.push(name.clone());
    let label = element.require_attr("title")?.to_string();
    let mut segments: Vec<TemplateSectionSegment> = Vec::new();
    let mut values: Vec<String> = Vec::new();
    for child in element.children() {
        match child {
            Node::Text(text) => {
                if !text.trim().is_empty() {
                    segments.push(TemplateSectionSegment::Text(text.clone()));
                }
            }
            Node::Element(interpolation) => {
                if interpolation.name != "value" {
                    return Err(TemplateXmlError::new(format!(
                        "Unknown element <{}> inside <section id=\"{name}\">.",
                        interpolation.name
                    )));
                }
                let field = interpolation.require_attr("field")?.to_string();
                if !field_ids.contains(&field) {
                    return Err(TemplateXmlError::new(format!(
                        "Section \"{name}\" references unknown field \"{field}\"."
                    )));
                }
                values.push(field.clone());
                segments.push(TemplateSectionSegment::Value(field));
            }
        }
    }
    let when = parse_when(element.attr("when"), &format!("Section \"{name}\""))?;
    Ok(TemplateSectionDefinition {
        name,
        label,
        editable: element.bool_attr("editable", false)?,
        segments,
        values,
        when,
    })
}

pub(super) fn parse_participants(
    element: &Element,
) -> Result<Vec<TemplateParticipantRole>, TemplateXmlError> {
    let mut roles: Vec<TemplateParticipantRole> = Vec::new();
    for child in element.elements() {
        if child.name != "participant" {
            return Err(TemplateXmlError::new(format!(
                "Unknown element <{}> inside <participants>.",
                child.name
            )));
        }
        let role = child.require_attr("role")?.to_string();
        let label = child
            .attr("label")
            .map(str::to_string)
            .unwrap_or_else(|| role.clone());
        let multiple = child.bool_attr("multiple", false)?;
        roles.push(TemplateParticipantRole {
            role,
            label,
            multiple,
        });
    }
    Ok(roles)
}

pub(super) fn parse_signatures(
    element: &Element,
    field_ids: &[String],
    email_field_ids: &[String],
) -> Result<Vec<TemplateSignatureGroup>, TemplateXmlError> {
    let mut groups: Vec<TemplateSignatureGroup> = Vec::new();
    for child in element.elements() {
        if child.name != "signature-group" {
            return Err(TemplateXmlError::new(format!(
                "Unknown element <{}> inside <signatures>.",
                child.name
            )));
        }
        let role = child.require_attr("role")?.to_string();
        let label = child
            .attr("label")
            .map(str::to_string)
            .unwrap_or_else(|| role.clone());
        let field = child
            .attr("field")
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if let Some(field) = field.as_deref() {
            if !field_ids.iter().any(|id| id == field) {
                return Err(TemplateXmlError::new(format!(
                    "Signature group \"{role}\" references unknown field \"{field}\"."
                )));
            }
        }
        let initials = child.bool_attr("initials", false)?;
        let email = child
            .attr("email")
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if let Some(email) = email.as_deref() {
            if !email_field_ids.iter().any(|id| id == email) {
                return Err(TemplateXmlError::new(format!(
                    "Signature group \"{role}\" names \"{email}\", which is not an email field."
                )));
            }
        }
        groups.push(TemplateSignatureGroup {
            role,
            label,
            field,
            initials,
            email,
        });
    }
    Ok(groups)
}

/// Parse one template file. Every refusal here mirrors the TypeScript contract's refusal, so a template that is wrong
/// is refused for the same stated reason on either side.
pub fn parse_template_xml(xml: &str) -> Result<TemplateDefinition, TemplateXmlError> {
    let root = build_tree(tokenize(xml)?)?;
    if root.name != "form" {
        return Err(TemplateXmlError::new(format!(
            "Template root must be <form> (got <{}>).",
            root.name
        )));
    }
    let id = root.require_attr("id")?.to_string();
    let raw_version = root.require_attr("version")?;
    let invalid_version = || {
        TemplateXmlError::new(format!(
            "<form> version must be a positive integer (got \"{raw_version}\")."
        ))
    };
    let version: i32 = raw_version.trim().parse().map_err(|_| invalid_version())?;
    if version < 1 {
        return Err(invalid_version());
    }
    let title = root.require_attr("title")?.to_string();
    let document_type = root
        .attr("documentType")
        .map(str::to_string)
        .unwrap_or_else(|| title.clone());
    let issuer = root
        .attr("issuer")
        .map(str::to_string)
        .unwrap_or_else(|| "CulebraLuxe Real Estate".to_string());
    let raw_presentation = root.attr("presentation").unwrap_or("report");
    let Some(presentation) = TemplatePresentation::parse(raw_presentation) else {
        return Err(TemplateXmlError::new(format!(
            "<form> presentation must be one of agreement, letter, information, report (got \"{raw_presentation}\")."
        )));
    };
    let mut field_ids: Vec<String> = Vec::new();
    let mut section_ids: Vec<String> = Vec::new();
    let mut fields: Vec<TemplateFieldDefinition> = Vec::new();
    let mut sections: Vec<TemplateSectionDefinition> = Vec::new();
    let mut participants: Vec<TemplateParticipantRole> = Vec::new();
    let mut signature_groups: Vec<TemplateSignatureGroup> = Vec::new();
    for child in root.children() {
        match child {
            Node::Text(text) => {
                if !text.trim().is_empty() {
                    return Err(TemplateXmlError::new(
                        "Unexpected text content directly under <form>.",
                    ));
                }
            }
            Node::Element(child) => match child.name.as_str() {
                "field" => fields.push(parse_field(child, &mut field_ids)?),
                "section" => sections.push(parse_section(child, &field_ids, &mut section_ids)?),
                "participants" => participants = parse_participants(child)?,
                "signatures" => {
                    let email_field_ids: Vec<String> = fields
                        .iter()
                        .filter(|field| field.field_type == TemplateFieldType::Email)
                        .map(|field| field.name.clone())
                        .collect();
                    signature_groups = parse_signatures(child, &field_ids, &email_field_ids)?
                }
                other => {
                    return Err(TemplateXmlError::new(format!(
                        "Unknown element <{other}> inside <form>."
                    )))
                }
            },
        }
    }
    if fields.is_empty() {
        return Err(TemplateXmlError::new(
            "Template must declare at least one <field>.",
        ));
    }
    Ok(TemplateDefinition {
        id,
        version,
        display_name: document_type.clone(),
        document_type_label: document_type,
        fields,
        sections,
        rendering: TemplateRendering {
            title,
            issuer,
            presentation,
        },
        participants,
        signature_groups,
    })
}

// ---------------------------------------------------------------- the library

/// Where templates live: the environment's directory when it names one, otherwise the CRATE-relative default.
///
/// CRATE-RELATIVE, because the templates are authored in this crate (`middle/model/forms/templates`) — and that is the
/// half of the story `resolve_repo_path` needs: a path that names the crate's own files stays true if the crate moves
/// again, while turning it into a real directory is the one place allowed to know the repository's shape.
pub const DEFAULT_TEMPLATES_DIR: &str = "forms/templates";

/// The environment variable that points at the template directory, for a deployment that ships the files elsewhere.
pub const TEMPLATES_DIR_ENV: &str = "FORMS_TEMPLATES_DIR";

/// The nearest ancestor of `directory` that holds `relative`, nearest first.
fn find_in_ancestors(directory: &Path, relative: &str) -> Option<PathBuf> {
    let mut candidate: Option<&Path> = Some(directory);
    while let Some(current) = candidate {
        let path = current.join(relative);
        if path.exists() {
            return Some(path);
        }
        candidate = current.parent();
    }
    None
}

/// A path inside the repository, RESOLVED rather than assumed.
///
/// WHY THIS EXISTS: a repository-relative path is only correct when the process happens to be started from the
/// repository root, and nothing guarantees that. A launcher used to start the API with the old workspace directory
/// (`rust/`) as its working directory (`scripts/dev-start.mjs`), so every render failed with "Cannot read the template
/// directory middle/model/forms/templates" — a launcher detail deciding whether documents could be composed at all.
///
/// WHY BOTH BASES ARE SEARCHED, RATHER THAN ONE: the callers do not agree on what `relative` is relative to, and each one
/// is right about its own default. The templates default is CRATE-relative (`forms/templates`, inside `model`); the
/// wordmark default is REPOSITORY-relative (`public/brand/CLLOGO.png`, `web/src/vault/artifact.rs`). A search that knows
/// only one base breaks the other, and the shape that shipped did exactly that: from the repository root — where
/// `scripts/dev.sh` starts the API — the crate-relative default was not found, and the forms screen answered "Cannot
/// read the template directory forms/templates: No such file or directory (os error 2)" while every unit test passed,
/// because `cargo test` starts in the crate directory where that same default happens to be right. So: the working
/// directory and its ANCESTORS are searched, then the directory the BUILD knew and its ancestors, and only then the
/// plain default, so a failure names the path that could not be found.
pub fn resolve_repo_path(relative: &str) -> PathBuf {
    let start = std::env::current_dir().unwrap_or_default();
    resolve_repo_path_from(&start, relative)
}

/// The same search, started from a NAMED directory.
///
/// WHY IT IS PUBLIC, AND WHY NAMING THE STARTING DIRECTORY IS THE POINT: `cargo test` starts a test binary in the crate
/// directory while `scripts/dev.sh` starts the API in the REPOSITORY ROOT, so a test that cannot name the launcher's
/// directory cannot catch a break in it. That is exactly how the templates stayed unreadable in dev while every test
/// passed (see `the_template_directory_resolves_from_the_repository_root_too`).
pub fn resolve_repo_path_from(start: &Path, relative: &str) -> PathBuf {
    if let Some(path) = find_in_ancestors(start, relative) {
        return path;
    }
    // The build knows where its source was, and that outlives whoever launched it and from where. Its ANCESTORS are
    // searched rather than one fixed depth: `../..` was the OLD workspace's depth (`rust/core/domain`, whose templates sat
    // in `lib/` — a hardcoded `../../../lib/forms/templates`), and after the three-tier move the same fixed depth pointed
    // at a repository root that holds no `forms/` at all. With the ancestors searched, a crate-relative default resolves
    // at the crate, a repository-relative one at the repository root, and neither depends on the depth between them.
    if let Some(path) = find_in_ancestors(Path::new(env!("CARGO_MANIFEST_DIR")), relative) {
        return path;
    }
    PathBuf::from(relative)
}

pub fn templates_dir() -> PathBuf {
    std::env::var(TEMPLATES_DIR_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| resolve_repo_path(DEFAULT_TEMPLATES_DIR))
}

/// Every template found in a directory, parsed.
#[derive(Debug, Clone, Default)]
pub struct TemplateLibrary {
    pub(super) templates: Vec<TemplateDefinition>,
}

impl TemplateLibrary {
    /// Read every `*.xml` in `dir` and parse it.
    ///
    /// WHY THE DIRECTORY IS SCANNED RATHER THAN LISTED IN CODE: adding a template version is an AUTHORING act — dropping
    /// an XML file — and a version a live record points at has to stay renderable. A list in code would make the first
    /// a rebuild and the second a risk, which is the opposite of what the format is for.
    pub fn load_from_dir(dir: &Path) -> Result<Self, TemplateXmlError> {
        let entries = std::fs::read_dir(dir).map_err(|error| {
            TemplateXmlError::new(format!(
                "Cannot read the template directory {}: {error}",
                dir.display()
            ))
        })?;
        let mut paths: Vec<PathBuf> = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| {
                TemplateXmlError::new(format!("Cannot read a template directory entry: {error}"))
            })?;
            let path = entry.path();
            let is_xml = path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"));
            if is_xml {
                paths.push(path);
            }
        }
        paths.sort();
        let mut templates: Vec<TemplateDefinition> = Vec::new();
        for path in paths {
            let source = std::fs::read_to_string(&path).map_err(|error| {
                TemplateXmlError::new(format!("Cannot read {}: {error}", path.display()))
            })?;
            let template = parse_template_xml(&source).map_err(|error| {
                TemplateXmlError::new(format!(
                    "Template {} failed to load: {}",
                    path.display(),
                    error.message
                ))
            })?;
            templates.push(template);
        }
        Ok(Self { templates })
    }

    pub fn load_default() -> Result<Self, TemplateXmlError> {
        Self::load_from_dir(&templates_dir())
    }

    pub fn all(&self) -> &[TemplateDefinition] {
        &self.templates
    }

    /// The exact version a persisted record points at. `None` means the record is unresolvable, which the caller must
    /// report rather than render a different version of the same document.
    pub fn version(&self, id: &str, version: i32) -> Option<&TemplateDefinition> {
        self.templates
            .iter()
            .find(|template| template.id == id && template.version == version)
    }

    /// The NEWEST AUTHORED version of a family.
    ///
    /// THIS IS NOT APPROVAL, and the difference is load-bearing. Dropping an XML file is an authoring act, so the newest
    /// file is only the newest DRAFT: which version a new document may be authored against is a human decision, and it
    /// lives in ONE place — `lib/forms/template-registry.ts` `ACTIVE_TEMPLATE_VERSIONS`. A create therefore arrives here
    /// with the version already chosen (`CreateFormInstanceRequest::template_version`) and this library stores what it is
    /// told. Today the two agree for every family, and
    /// `the_newest_authored_version_of_each_family_is_the_approved_one` fails the moment they stop agreeing — so that
    /// agreement is asserted rather than assumed.
    pub fn newest(&self, id: &str) -> Option<&TemplateDefinition> {
        self.templates
            .iter()
            .filter(|template| template.id == id)
            .max_by_key(|template| template.version)
    }

    /// The template families present, each with its newest AUTHORED version. Sorted by id.
    pub fn families(&self) -> Vec<(&str, i32)> {
        let mut ids: Vec<&str> = self
            .templates
            .iter()
            .map(|template| template.id.as_str())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter()
            .filter_map(|id| self.newest(id).map(|template| (id, template.version)))
            .collect()
    }
}
