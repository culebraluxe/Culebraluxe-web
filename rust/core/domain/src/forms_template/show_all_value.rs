//! Moved from `forms_template.rs` (move only): SHOW_ALL_VALUE, TemplateXmlError, new, fmt, TemplateFieldType, parse, TemplateWhen, TemplateFieldDefinition, TemplateSectionSegment, TemplateSectionDefinition, TemplatePresentation, TemplateRendering, TemplateParticipantRole, TemplateSignatureGroup, TemplateDefinition, field, Token, decode_entities, name_end, parse_attrs, tokenize, Node, Element, attr, build_tree, BINDINGS, parse_when, parse_field.

#[allow(unused_imports)]
use super::*;

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
    pub(super) fn parse(value: &str) -> Option<Self> {
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
    pub(super) fn parse(value: &str) -> Option<Self> {
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
pub(super) enum Token {
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
pub(super) fn decode_entities(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

pub(super) fn name_end(value: &str) -> usize {
    value
        .find(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == '-' || character == ':')
        })
        .unwrap_or(value.len())
}

/// The attribute pairs of one tag, and whatever is left over — `/` for a self-closing tag, nothing for an opening one.
/// Anything else is refused. Only whitespace may stand between the pairs.
pub(super) fn parse_attrs(
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

pub(super) fn tokenize(source: &str) -> Result<Vec<Token>, TemplateXmlError> {
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
pub(super) enum Node {
    Element(Element),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Element {
    pub(super) name: String,
    pub(super) attrs: Vec<(String, String)>,
    pub(super) children: Vec<Node>,
}

impl Element {
    pub(super) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    pub(super) fn require_attr(&self, name: &str) -> Result<&str, TemplateXmlError> {
        match self.attr(name) {
            Some(value) if !value.is_empty() => Ok(value),
            _ => Err(TemplateXmlError::new(format!(
                "<{}> requires attribute \"{name}\".",
                self.name
            ))),
        }
    }

    pub(super) fn bool_attr(&self, name: &str, fallback: bool) -> Result<bool, TemplateXmlError> {
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

    pub(super) fn children(&self) -> impl Iterator<Item = &Node> {
        self.children.iter()
    }

    pub(super) fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|child| match child {
            Node::Element(element) => Some(element),
            Node::Text(_) => None,
        })
    }
}

pub(super) fn build_tree(tokens: Vec<Token>) -> Result<Element, TemplateXmlError> {
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
pub(super) const BINDINGS: [&str; 8] = [
    "deal.client.name",
    "deal.property.label",
    "deal.offer.amount",
    "deal.financing.type",
    "deal.closing.date",
    "person.displayName",
    "property.name",
    "property.location",
];

pub(super) fn parse_when(raw: Option<&str>, label: &str) -> Result<Option<TemplateWhen>, TemplateXmlError> {
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

pub(super) fn parse_field(
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
