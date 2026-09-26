//! The form template: the XML contract, parsed — and the directory it is authored in.
//!
//! WHY THE FILES ARE READ AT RUNTIME, AND NEVER COMPILED IN. XML is the canonical authoring format for these documents,
//! and templates are versioned FILES: a live document re-renders from the exact version it was issued under, so a
//! version a record already points at must stay renderable. A new version, or a new template, is therefore an XML file
//! dropped in the directory — not a code change and not a rebuild. Nothing about a template is a constant here: the
//! directory is scanned, every file is parsed, and the library is keyed by `(id, version)`.
//!
//! WHY THE PARSER IS HAND-WRITTEN. The grammar is small, closed and specified below — one root element, four kinds of
//! child, attributes and inline `<value/>` references — and the workspace depends on no XML crate. A focused parser is
//! also the only way to keep the TypeScript contract's REFUSALS: a template that is wrong must be rejected with the
//! same reason on both sides rather than silently rendering a different document.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// The value that shows every gated field and section at once. It is never a live transaction value.
pub const SHOW_ALL_VALUE: &str = "Show All";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateXmlError {
    pub message: String,
}

impl TemplateXmlError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for TemplateXmlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for TemplateXmlError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateFieldType {
    Text,
    Money,
    Date,
    Textarea,
    Select,
}

impl TemplateFieldType {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "text" => Some(Self::Text),
            "money" => Some(Self::Money),
            "date" => Some(Self::Date),
            "textarea" => Some(Self::Textarea),
            "select" => Some(Self::Select),
            _ => None,
        }
    }
}

/// A visibility gate: `when="field:Value,Value"`. The named field must hold one of the values, case-insensitively.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateWhen {
    pub field: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateFieldDefinition {
    pub name: String,
    pub label: String,
    pub field_type: TemplateFieldType,
    pub required: bool,
    /// The canonical binding this field is prefilled from, when the template declares one.
    pub binding: Option<String>,
    pub options: Vec<String>,
    pub when: Option<TemplateWhen>,
}

/// One run of a section's prose: literal text, or the value of a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateSectionSegment {
    Text(String),
    Value(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateSectionDefinition {
    pub name: String,
    pub label: String,
    pub editable: bool,
    pub segments: Vec<TemplateSectionSegment>,
    /// The field names this section's prose interpolates, in the order they appear.
    pub values: Vec<String>,
    pub when: Option<TemplateWhen>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplatePresentation {
    Agreement,
    Letter,
    Information,
    Report,
}

impl TemplatePresentation {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "agreement" => Some(Self::Agreement),
            "letter" => Some(Self::Letter),
            "information" => Some(Self::Information),
            "report" => Some(Self::Report),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateRendering {
    pub title: String,
    pub issuer: String,
    pub presentation: TemplatePresentation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateParticipantRole {
    pub role: String,
    pub label: String,
    pub multiple: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateSignatureGroup {
    pub role: String,
    pub label: String,
    pub field: Option<String>,
    pub initials: bool,
}

/// One template version, exactly as the XML declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateDefinition {
    pub id: String,
    pub version: i32,
    pub display_name: String,
    pub document_type_label: String,
    pub fields: Vec<TemplateFieldDefinition>,
    pub sections: Vec<TemplateSectionDefinition>,
    pub rendering: TemplateRendering,
    pub participants: Vec<TemplateParticipantRole>,
    pub signature_groups: Vec<TemplateSignatureGroup>,
}

impl TemplateDefinition {
    pub fn field(&self, name: &str) -> Option<&TemplateFieldDefinition> {
        self.fields.iter().find(|field| field.name == name)
    }

    /// The fields this template marks required, in declaration order.
    pub fn required_fields(&self) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|field| field.required)
            .map(|field| field.name.as_str())
            .collect()
    }

    /// Whether a gated field or section is visible for the values supplied.
    pub fn when_satisfied(when: Option<&TemplateWhen>, values: &BTreeMap<String, String>) -> bool {
        let Some(when) = when else { return true };
        let actual = values
            .get(&when.field)
            .map(|value| value.trim())
            .unwrap_or_default();
        if actual.is_empty() {
            return false;
        }
        if actual.eq_ignore_ascii_case(SHOW_ALL_VALUE) {
            return true;
        }
        when.values
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(actual))
    }
}

// ---------------------------------------------------------------- tokenizer

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Open {
        name: String,
        attrs: Vec<(String, String)>,
    },
    SelfClose {
        name: String,
        attrs: Vec<(String, String)>,
    },
    Close {
        name: String,
    },
    Text(String),
}

/// Named and numeric character references, decoded in the order the TypeScript contract uses: the specific entities
/// first and `&amp;` LAST, so `&amp;lt;` reads as the text `&lt;` rather than as a tag.
fn decode_entities(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn name_end(value: &str) -> usize {
    value
        .find(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == '-' || character == ':')
        })
        .unwrap_or(value.len())
}

/// The attribute pairs of one tag, and whatever is left over — `/` for a self-closing tag, nothing for an opening one.
/// Anything else is refused. Only whitespace may stand between the pairs.
fn parse_attrs(
    tag: &str,
    mut rest: &str,
    offset: usize,
) -> Result<(Vec<(String, String)>, String), TemplateXmlError> {
    let mut attrs: Vec<(String, String)> = Vec::new();
    loop {
        let trimmed = rest.trim_start();
        if trimmed.is_empty() {
            return Ok((attrs, String::new()));
        }
        if trimmed == "/" {
            return Ok((attrs, "/".to_string()));
        }
        let end = name_end(trimmed);
        if end == 0 {
            return Err(TemplateXmlError::new(format!(
                "Malformed XML: unexpected content in <{tag}> at offset {offset}."
            )));
        }
        let name = &trimmed[..end];
        let after_name = trimmed[end..].trim_start();
        let Some(after_eq) = after_name.strip_prefix('=') else {
            return Err(TemplateXmlError::new(format!(
                "Malformed XML: attribute \"{name}\" in <{tag}> has no value."
            )));
        };
        let after_eq = after_eq.trim_start();
        let Some(quote) = after_eq.chars().next() else {
            return Err(TemplateXmlError::new(format!(
                "Malformed XML: attribute \"{name}\" in <{tag}> is unterminated."
            )));
        };
        if quote != '"' && quote != '\'' {
            return Err(TemplateXmlError::new(format!(
                "Malformed XML: attribute \"{name}\" in <{tag}> must be quoted."
            )));
        }
        let body = &after_eq[1..];
        let Some(close) = body.find(quote) else {
            return Err(TemplateXmlError::new(format!(
                "Malformed XML: attribute \"{name}\" in <{tag}> is unterminated."
            )));
        };
        attrs.push((name.to_string(), decode_entities(&body[..close])));
        rest = &body[close + 1..];
    }
}

fn tokenize(source: &str) -> Result<Vec<Token>, TemplateXmlError> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut i = 0usize;
    while i < source.len() {
        let Some(relative) = source[i..].find('<') else {
            let text = &source[i..];
            if !text.trim().is_empty() {
                tokens.push(Token::Text(decode_entities(text)));
            }
            break;
        };
        let lt = i + relative;
        if lt > i {
            let text = &source[i..lt];
            if !text.trim().is_empty() {
                tokens.push(Token::Text(decode_entities(text)));
            }
        }
        let rest = &source[lt..];
        if rest.starts_with("<!--") {
            let Some(end) = rest.find("-->") else {
                return Err(TemplateXmlError::new("Malformed XML: unterminated comment."));
            };
            i = lt + end + 3;
            continue;
        }
        if rest.starts_with("<![CDATA[") {
            let Some(end) = rest.find("]]>") else {
                return Err(TemplateXmlError::new("Malformed XML: unterminated CDATA."));
            };
            tokens.push(Token::Text(rest[9..end].to_string()));
            i = lt + end + 3;
            continue;
        }
        if rest.starts_with("<?") {
            let Some(end) = rest.find("?>") else {
                return Err(TemplateXmlError::new(
                    "Malformed XML: unterminated processing instruction.",
                ));
            };
            i = lt + end + 2;
            continue;
        }
        let Some(gt) = rest.find('>') else {
            return Err(TemplateXmlError::new(format!(
                "Malformed XML: unterminated tag at offset {lt}."
            )));
        };
        let raw = rest[1..gt].trim();
        i = lt + gt + 1;
        if let Some(closing) = raw.strip_prefix('/') {
            let closing = closing.trim();
            if closing.is_empty() {
                return Err(TemplateXmlError::new(format!(
                    "Malformed XML: empty closing tag at offset {lt}."
                )));
            }
            tokens.push(Token::Close {
                name: closing.to_string(),
            });
            continue;
        }
        let end = name_end(raw);
        let starts_well = raw
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_');
        if end == 0 || !starts_well {
            return Err(TemplateXmlError::new(format!(
                "Malformed XML: invalid tag at offset {lt}."
            )));
        }
        let name = &raw[..end];
        let (attrs, trailing) = parse_attrs(name, raw[end..].trim(), lt)?;
        if trailing == "/" {
            tokens.push(Token::SelfClose {
                name: name.to_string(),
                attrs,
            });
        } else if trailing.is_empty() {
            tokens.push(Token::Open {
                name: name.to_string(),
                attrs,
            });
        } else {
            return Err(TemplateXmlError::new(format!(
                "Malformed XML: unexpected content in <{name}> at offset {lt}: \"{trailing}\"."
            )));
        }
    }
    Ok(tokens)
}

// ---------------------------------------------------------------- tree

#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Element(Element),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Element {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
}

impl Element {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn require_attr(&self, name: &str) -> Result<&str, TemplateXmlError> {
        match self.attr(name) {
            Some(value) if !value.is_empty() => Ok(value),
            _ => Err(TemplateXmlError::new(format!(
                "<{}> requires attribute \"{name}\".",
                self.name
            ))),
        }
    }

    fn bool_attr(&self, name: &str, fallback: bool) -> Result<bool, TemplateXmlError> {
        match self.attr(name) {
            None | Some("") => Ok(fallback),
            Some(value) => match value.to_ascii_lowercase().as_str() {
                "true" | "1" => Ok(true),
                "false" | "0" => Ok(false),
                _ => Err(TemplateXmlError::new(format!(
                    "<{}> attribute \"{name}\" must be true or false (got \"{value}\").",
                    self.name
                ))),
            },
        }
    }

    fn children(&self) -> impl Iterator<Item = &Node> {
        self.children.iter()
    }

    fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|child| match child {
            Node::Element(element) => Some(element),
            Node::Text(_) => None,
        })
    }
}

fn build_tree(tokens: Vec<Token>) -> Result<Element, TemplateXmlError> {
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    let attach = |stack: &mut Vec<Element>, root: &mut Option<Element>, element: Element| {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(Node::Element(element));
            Ok(())
        } else if root.is_none() {
            *root = Some(element);
            Ok(())
        } else {
            Err(TemplateXmlError::new(
                "Malformed XML: multiple root elements.",
            ))
        }
    };
    for token in tokens {
        match token {
            Token::Open { name, attrs } => stack.push(Element {
                name,
                attrs,
                children: Vec::new(),
            }),
            Token::SelfClose { name, attrs } => attach(
                &mut stack,
                &mut root,
                Element {
                    name,
                    attrs,
                    children: Vec::new(),
                },
            )?,
            Token::Text(text) => {
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(Node::Text(text));
                }
            }
            Token::Close { name } => {
                let Some(top) = stack.pop() else {
                    return Err(TemplateXmlError::new(format!(
                        "Malformed XML: unexpected closing </{name}>."
                    )));
                };
                if top.name != name {
                    return Err(TemplateXmlError::new(format!(
                        "Malformed XML: unexpected closing </{name}>."
                    )));
                }
                attach(&mut stack, &mut root, top)?;
            }
        }
    }
    if let Some(open) = stack.last() {
        return Err(TemplateXmlError::new(format!(
            "Malformed XML: unclosed element <{}>.",
            open.name
        )));
    }
    root.ok_or_else(|| TemplateXmlError::new("Malformed XML: empty document."))
}

// ---------------------------------------------------------------- elements

/// The canonical bindings a field may be prefilled from. A template that names any other is refused rather than
/// silently rendering an empty value.
const BINDINGS: [&str; 8] = [
    "deal.client.name",
    "deal.property.label",
    "deal.offer.amount",
    "deal.financing.type",
    "deal.closing.date",
    "person.displayName",
    "property.name",
    "property.location",
];

fn parse_when(raw: Option<&str>, label: &str) -> Result<Option<TemplateWhen>, TemplateXmlError> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let Some((field, rest)) = raw.split_once(':') else {
        return Err(TemplateXmlError::new(format!(
            "{label}: when must be \"field:Value\" or \"field:Value,Value\" (got \"{raw}\")."
        )));
    };
    let field = field.trim();
    let values: Vec<String> = rest
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();
    if field.is_empty() || values.is_empty() {
        return Err(TemplateXmlError::new(format!(
            "{label}: when must name a field and at least one value (got \"{raw}\")."
        )));
    }
    Ok(Some(TemplateWhen {
        field: field.to_string(),
        values,
    }))
}

fn parse_field(
    element: &Element,
    field_ids: &mut Vec<String>,
) -> Result<TemplateFieldDefinition, TemplateXmlError> {
    let name = element.require_attr("id")?.to_string();
    if field_ids.contains(&name) {
        return Err(TemplateXmlError::new(format!(
            "Duplicate field id \"{name}\"."
        )));
    }
    field_ids.push(name.clone());
    let label = element.require_attr("label")?.to_string();
    let raw_type = element.require_attr("type")?;
    let Some(field_type) = TemplateFieldType::parse(raw_type) else {
        return Err(TemplateXmlError::new(format!(
            "Field \"{name}\" has unknown type \"{raw_type}\"."
        )));
    };
    let binding = match element.attr("source") {
        None => None,
        Some(source) => {
            if !BINDINGS.contains(&source) {
                return Err(TemplateXmlError::new(format!(
                    "Field \"{name}\" has unknown source binding \"{source}\"."
                )));
            }
            Some(source.to_string())
        }
    };
    let options = if field_type == TemplateFieldType::Select {
        let parsed: Vec<String> = element
            .attr("options")
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect();
        if parsed.is_empty() {
            return Err(TemplateXmlError::new(format!(
                "Select field \"{name}\" requires a non-empty options list."
            )));
        }
        parsed
    } else {
        Vec::new()
    };
    let when = parse_when(element.attr("when"), &format!("Field \"{name}\""))?;
    Ok(TemplateFieldDefinition {
        name,
        label,
        field_type,
        required: element.bool_attr("required", false)?,
        binding,
        options,
        when,
    })
}

fn parse_section(
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

fn parse_participants(element: &Element) -> Result<Vec<TemplateParticipantRole>, TemplateXmlError> {
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

fn parse_signatures(
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
    templates: Vec<TemplateDefinition>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The real authoring directory, reached from this crate rather than from the working directory — a test that
    /// depended on the working directory would pass or fail by where it was run from.
    fn templates_dir_in_repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../lib/forms/templates")
    }

    fn library() -> TemplateLibrary {
        TemplateLibrary::load_from_dir(&templates_dir_in_repo())
            .expect("the repository's templates load")
    }

    /// THE PRODUCTION ENTRY POINT, not the test helper. Its default is repository-relative, and the dev launcher starts
    /// the API with `rust/` as its working directory — so this assertion is what keeps a launcher detail from deciding
    /// whether documents can be composed at all.
    #[test]
    fn the_template_directory_resolves_from_any_working_directory() {
        let directory = templates_dir();
        assert!(
            directory.join("LISTING-01.v4.xml").exists(),
            "the templates did not resolve: {}",
            directory.display()
        );
    }

    #[test]
    fn every_repository_template_parses() {
        let library = library();
        assert!(
            library.all().len() >= 9,
            "expected the nine authored versions, found {}",
            library.all().len()
        );
        let ids: Vec<&str> = library.families().into_iter().map(|(id, _)| id).collect();
        for expected in [
            "OFFER-01",
            "LISTING-01",
            "PR-PNS",
            "PR-PNS-AMD",
            "SHOW-INFO",
            "SHOW-RPT",
        ] {
            assert!(
                ids.contains(&expected),
                "template family {expected} is missing from the directory"
            );
        }
    }

    #[test]
    fn a_persisted_version_resolves_to_that_exact_version() {
        let library = library();
        // The reason versions are files rather than one mutable active template: live records point at exact versions.
        for version in [2, 3, 4] {
            let template = library
                .version("LISTING-01", version)
                .unwrap_or_else(|| panic!("LISTING-01 v{version} is not loadable"));
            assert_eq!(template.version, version);
        }
        assert_eq!(library.newest("LISTING-01").map(|t| t.version), Some(4));
        assert!(library.version("LISTING-01", 99).is_none());
    }

    /// THE CANARY. The newest authored file is only a draft; which version a NEW document may use is a human decision that
    /// lives in `lib/forms/template-registry.ts` (`ACTIVE_TEMPLATE_VERSIONS`). The two agree today, and this test is what
    /// keeps that true: a failure here means an XML was dropped whose version nobody approved — either approve it in the
    /// registry in the same change, or remove the file.
    #[test]
    fn the_newest_authored_version_of_each_family_is_the_approved_one() {
        let library = library();
        for (id, approved) in [
            ("OFFER-01", 2),
            ("LISTING-01", 4),
            ("PR-PNS", 3),
            ("PR-PNS-AMD", 1),
            ("SHOW-INFO", 1),
            ("SHOW-RPT", 1),
        ] {
            assert_eq!(
                library.newest(id).map(|template| template.version),
                Some(approved),
                "{id}: the newest XML is not the approved version — see ACTIVE_TEMPLATE_VERSIONS"
            );
        }
    }

    #[test]
    fn a_template_carries_its_fields_sections_participants_and_signatures() {
        let library = library();
        let template = library
            .version("LISTING-01", 4)
            .expect("LISTING-01 v4 loads");

        assert_eq!(
            template.rendering.presentation,
            TemplatePresentation::Agreement
        );
        assert_eq!(template.document_type_label, "Listing Agreement");
        assert!(template.rendering.issuer.starts_with("Culebraluxe"));

        let price = template.field("listPrice").expect("listPrice exists");
        assert_eq!(price.field_type, TemplateFieldType::Money);
        assert!(price.required);
        assert_eq!(price.label, "Asking Price");

        let listing_type = template.field("listingType").expect("listingType exists");
        assert_eq!(listing_type.options.len(), 2);
        assert_eq!(listing_type.options[0], "Exclusive Right to Sell");

        let partnership = template
            .sections
            .iter()
            .find(|section| section.name == "partnership")
            .expect("the partnership section exists");
        assert!(!partnership.editable);
        assert!(
            partnership
                .segments
                .iter()
                .any(|segment| *segment == TemplateSectionSegment::Value("sellerName".into())),
            "the prose interpolates the seller's name"
        );
        assert!(partnership.values.contains(&"sellerName".to_string()));

        let seller = template
            .participants
            .iter()
            .find(|participant| participant.role == "SELLER")
            .expect("SELLER is a participant role");
        assert!(seller.multiple);

        assert_eq!(template.signature_groups.len(), 2);
        assert_eq!(template.signature_groups[0].role, "SELLER");
        assert!(template.signature_groups[0].initials);
        assert_eq!(
            template.signature_groups[0].field.as_deref(),
            Some("sellerName")
        );
    }

    #[test]
    fn an_empty_section_carries_no_segments() {
        let template = parse_template_xml(
            r#"<form id="T" version="1" title="T">
                 <field id="a" label="A" type="text"/>
                 <section id="empty" title="Empty" editable="true"></section>
               </form>"#,
        )
        .expect("an empty section is allowed");
        assert!(template.sections[0].segments.is_empty());
        assert!(template.sections[0].editable);
    }

    #[test]
    fn a_template_that_is_wrong_is_refused_with_a_reason() {
        let cases: [(&str, &str); 7] = [
            (
                "<form id=\"T\" version=\"1\" title=\"T\"><bogus/></form>",
                "Unknown element",
            ),
            (
                "<form id=\"T\" version=\"1\" title=\"T\"><field id=\"a\" label=\"A\" type=\"text\"/><field id=\"a\" label=\"A\" type=\"text\"/></form>",
                "Duplicate field id",
            ),
            (
                "<form id=\"T\" version=\"1\" title=\"T\"><field id=\"a\" label=\"A\" type=\"select\"/></form>",
                "non-empty options list",
            ),
            (
                "<form id=\"T\" version=\"1\" title=\"T\"><field id=\"a\" label=\"A\" type=\"text\" source=\"nowhere\"/></form>",
                "unknown source binding",
            ),
            (
                "<form id=\"T\" version=\"1\" title=\"T\" presentation=\"poem\"><field id=\"a\" label=\"A\" type=\"text\"/></form>",
                "presentation must be one of",
            ),
            (
                "<form id=\"T\" version=\"1\" title=\"T\"><field id=\"a\" label=\"A\" type=\"text\" when=\"novalue\"/></form>",
                "when must be",
            ),
            (
                "<form id=\"T\" version=\"1\" title=\"T\"><section id=\"s\" title=\"S\"><value field=\"missing\"/></section></form>",
                "references unknown field",
            ),
        ];
        for (xml, expected) in cases {
            let error = parse_template_xml(xml).expect_err("the template is refused");
            assert!(
                error.message.contains(expected),
                "expected a refusal mentioning \"{expected}\", got \"{}\"",
                error.message
            );
        }
        assert!(
            parse_template_xml(
                "<form id=\"T\" version=\"1\" title=\"T\"><field id=\"a\" label=\"A\" type=\"text\"/>"
            )
            .is_err(),
            "an unclosed document is refused"
        );
        let root = parse_template_xml("<letter id=\"T\" version=\"1\" title=\"T\"/>")
            .expect_err("a foreign root element is refused");
        assert!(root.message.contains("root must be <form>"));
    }

    #[test]
    fn a_when_gate_reads_the_named_field_case_insensitively() {
        let values = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
            pairs
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect()
        };
        let gate = TemplateWhen {
            field: "financing".to_string(),
            values: vec!["Cash".to_string(), "Blend".to_string()],
        };
        assert!(TemplateDefinition::when_satisfied(None, &values(&[])));
        assert!(TemplateDefinition::when_satisfied(
            Some(&gate),
            &values(&[("financing", "cash")])
        ));
        assert!(!TemplateDefinition::when_satisfied(
            Some(&gate),
            &values(&[("financing", "Bank")])
        ));
        assert!(
            !TemplateDefinition::when_satisfied(Some(&gate), &values(&[])),
            "an unset gate is hidden rather than shown"
        );
        assert!(TemplateDefinition::when_satisfied(
            Some(&gate),
            &values(&[("financing", "Show All")])
        ));
    }
}







