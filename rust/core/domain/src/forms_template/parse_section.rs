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
        groups.push(TemplateSignatureGroup {
            role,
            label,
            field,
            initials,
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
                "signatures" => signature_groups = parse_signatures(child, &field_ids)?,
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

/// Where templates live: the environment's directory when it names one, otherwise the repository-relative default.
pub const DEFAULT_TEMPLATES_DIR: &str = "lib/forms/templates";

/// The environment variable that points at the template directory, for a deployment that ships the files elsewhere.
pub const TEMPLATES_DIR_ENV: &str = "FORMS_TEMPLATES_DIR";

/// A path inside the repository, RESOLVED rather than assumed.
///
/// WHY THIS EXISTS: the defaults below are repository-relative, and a repository-relative path is only correct when the
/// process happens to be started from the repository root. The dev launcher starts the API with `rust/` as its working
/// directory (`scripts/dev-start.mjs`), so every render failed with "Cannot read the template directory
/// lib/forms/templates" — a launcher detail deciding whether documents could be composed at all. So: the working directory
/// and its ANCESTORS are searched, then the compile-time repository root, and only then the plain default, so a failure
/// names the path that could not be found.
pub fn resolve_repo_path(relative: &str) -> PathBuf {
    let from_working_dir = std::env::current_dir().ok().and_then(|start| {
        let mut candidate: Option<&std::path::Path> = Some(start.as_path());
        while let Some(directory) = candidate {
            let path = directory.join(relative);
            if path.exists() {
                return Some(path);
            }
            candidate = directory.parent();
        }
        None
    });
    if let Some(path) = from_working_dir {
        return path;
    }
    // The build knows where its source was, and that outlives whoever launched it and from where.
    let from_build = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative);
    if from_build.exists() {
        return from_build;
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
