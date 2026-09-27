//! The Project navigator's tree — a line-for-line port of `buildNavigatorTree` (components/rust-ui/project-islands.tsx).
//!
//! Three levels, per domain: POLES (the property, client or contract a project is about — or, when a project has no
//! anchor in this domain, the domain's collection), then PROJECTS (kind, status, progress), then the project's WORK as a
//! tree (by parent, in order). A project anchored to two things in the domain appears under both.

use std::collections::{BTreeMap, HashSet};

use crate::model::{PortalProject, PortalProjectWorkItem, PortalProjectsPage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Pole,
    Project,
    Work,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NavNode {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    pub subtitle: Option<String>,
    pub domain: String,
    pub project_id: Option<String>,
    pub work_id: Option<String>,
    /// complete, in-progress, dismissed, not-started (work only).
    pub status: Option<String>,
    /// Kind (project) or type (work) first, then " · " and the rest.
    pub meta: String,
    pub search: String,
    pub progress: Option<i64>,
    pub children: Vec<NavNode>,
}

pub const DOMAINS: [(&str, &str, &str); 6] = [
    ("properties", "Properties", "home"),
    ("people", "People", "users"),
    ("deals", "Deals", "handshake"),
    ("firm", "Firm", "building-2"),
    ("marketing", "Marketing", "megaphone"),
    ("accounting", "Accounting", "banknote"),
];

pub fn domain_label(domain: &str) -> &'static str {
    DOMAINS.iter().find(|(key, ..)| *key == domain).map_or("Projects", |(_, label, _)| label)
}

pub fn domain_icon(domain: &str) -> &'static str {
    DOMAINS.iter().find(|(key, ..)| *key == domain).map_or("home", |(.., icon)| icon)
}

pub fn project_kind_icon(kind: &str) -> &'static str {
    match kind.to_uppercase().as_str() {
        "LISTING" => "key-round",
        "MARKETING" => "megaphone",
        "CLOSING" | "DEAL" => "handshake",
        "CLIENT" | "BUYER_REP" => "users",
        "FIRM" => "building-2",
        "ACCOUNTING" => "banknote",
        _ => "file-text",
    }
}

pub fn work_type(category: &str) -> &'static str {
    match category {
        "contracts" => "contract",
        "media" => "media",
        "marketing" => "workflow",
        "accounting" => "accounting",
        "clients" | "properties" => "group",
        _ => "task",
    }
}

pub fn work_type_icon(kind: &str, label: &str) -> &'static str {
    match kind {
        "contract" | "document" => "file-text",
        "approval" => "pen-line",
        "media" => "image",
        "accounting" => "banknote",
        "marketing" => "megaphone",
        "workflow" => "git-branch",
        "task" => "check-circle-2",
        "milestone" => "flag",
        "group" => {
            let lower = label.to_lowercase();
            if ["client", "parties", "people", "person", "seller"].iter().any(|word| lower.contains(word)) {
                "users"
            } else {
                "home"
            }
        }
        _ => "file-text",
    }
}

fn work_status(status: &str) -> &'static str {
    match status {
        "done" => "complete",
        "doing" => "in-progress",
        "dismissed" => "dismissed",
        _ => "not-started",
    }
}

fn status_label(status: &str) -> &'static str {
    match status {
        "done" => "complete",
        "doing" => "in progress",
        "dismissed" => "dismissed",
        _ => "not started",
    }
}

/// The colour a work item's icon takes from its status.
pub fn status_class(status: Option<&str>) -> &'static str {
    match status {
        Some("complete") => "text-[var(--portal-success)]",
        Some("blocked") => "text-[var(--portal-archive)]",
        Some("waiting") => "text-[var(--portal-gold)]",
        Some("in-progress") => "text-[var(--portal-gold)]/80",
        Some("dismissed") => "text-black/30",
        _ => "text-black/45",
    }
}

fn entity_domain(entity_type: &str) -> Option<&'static str> {
    match entity_type {
        "property" => Some("properties"),
        "person" => Some("people"),
        "contract" | "deal" => Some("deals"),
        _ => None,
    }
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

fn due_label(value: Option<&str>) -> String {
    let Some(value) = value else { return String::new() };
    let (Some(month), Some(day)) = (value.get(5..7), value.get(8..10)) else { return String::new() };
    match (month.parse::<usize>(), day.parse::<u32>()) {
        (Ok(month @ 1..=12), Ok(day)) => format!("{} {day}", MONTHS[month - 1]),
        _ => String::new(),
    }
}

fn work_children(
    pole_id: &str,
    project_id: &str,
    parent: Option<&str>,
    items: &[&PortalProjectWorkItem],
    seen: &HashSet<String>,
) -> Vec<NavNode> {
    let mut level: Vec<&&PortalProjectWorkItem> = items
        .iter()
        .filter(|item| item.parent_id.as_deref() == parent && !seen.contains(&item.id))
        .collect();
    level.sort_by(|a, b| {
        a.order
            .unwrap_or(i32::MAX)
            .cmp(&b.order.unwrap_or(i32::MAX))
            .then_with(|| a.due_at.as_deref().unwrap_or("9999").cmp(b.due_at.as_deref().unwrap_or("9999")))
            .then_with(|| a.id.cmp(&b.id))
    });
    level
        .into_iter()
        .map(|item| {
            let mut branch = seen.clone();
            branch.insert(item.id.clone());
            let kind = work_type(&item.category);
            let status = work_status(&item.status);
            let due = due_label(item.due_at.as_deref());
            let meta = [kind, status_label(&item.status), due.as_str()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            let children = work_children(pole_id, project_id, Some(item.id.as_str()), items, &branch);
            let search = [item.title.as_str(), kind, status, item.owner.as_deref().unwrap_or(""), due.as_str()]
                .into_iter()
                .chain(children.iter().map(|child| child.search.as_str()))
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            NavNode {
                id: format!("{pole_id}::{project_id}::{}", item.id),
                kind: NodeKind::Work,
                label: item.title.clone(),
                subtitle: None,
                domain: String::new(),
                project_id: Some(project_id.to_owned()),
                work_id: Some(item.id.clone()),
                status: Some(status.to_owned()),
                meta,
                search,
                progress: None,
                children,
            }
        })
        .collect()
}

fn add<'a>(buckets: &mut Vec<Bucket<'a>>, id: String, label: String, subtitle: String, project: &'a PortalProject) {
    if let Some(bucket) = buckets.iter_mut().find(|bucket| bucket.id == id) {
        if !bucket.projects.iter().any(|p| p.id == project.id) {
            bucket.projects.push(project);
        }
    } else {
        buckets.push(Bucket { id, label, subtitle, projects: vec![project] });
    }
}

struct Bucket<'a> {
    id: String,
    label: String,
    subtitle: String,
    projects: Vec<&'a PortalProject>,
}

/// The whole tree for the page's active domain.
pub fn build(page: &PortalProjectsPage) -> Vec<NavNode> {
    let domain = page.active_domain.as_str();
    let mut by_project: BTreeMap<&str, Vec<&PortalProjectWorkItem>> = BTreeMap::new();
    for item in &page.items {
        if let Some(project_id) = item.project_id.as_deref() {
            by_project.entry(project_id).or_default().push(item);
        }
    }
    let mut buckets: Vec<Bucket> = Vec::new();
    for project in &page.projects {
        let items = by_project.get(project.id.as_str()).cloned().unwrap_or_default();
        if !crate::projects::project_in_domain(project, &page.items, domain) {
            continue;
        }
        let mut anchors: Vec<(String, String)> = Vec::new();
        let mut note = |kind: &str, id: &str| {
            if !anchors.iter().any(|(k, i)| k == kind && i == id) {
                anchors.push((kind.to_owned(), id.to_owned()));
            }
        };
        if let Some(id) = &project.property_id {
            note("property", id);
        }
        if let Some(id) = &project.person_id {
            note("person", id);
        }
        if let Some(id) = &project.contract_id {
            note("contract", id);
        }
        for item in &items {
            if let Some(entity) = &item.entity {
                note(&entity.entity_type, &entity.id);
            }
        }
        let matching: Vec<&(String, String)> =
            anchors.iter().filter(|(kind, _)| entity_domain(kind) == Some(domain)).collect();
        if matching.is_empty() {
            add(&mut buckets, format!("collection-{domain}"), domain_label(domain).to_owned(), String::new(), project);
            continue;
        }
        for (kind, id) in matching {
            let key = format!("{kind}:{id}");
            let label = page.identity_names.get(&key).cloned().unwrap_or_else(|| id.clone());
            let subtitle = match kind.as_str() {
                "person" => "Client",
                "property" => "Property",
                "contract" => "Contract",
                _ => "Workspace",
            };
            add(&mut buckets, format!("entity-{kind}-{id}"), label, subtitle.to_owned(), project);
        }
    }

    buckets
        .into_iter()
        .map(|bucket| {
            let subtitle = if bucket.id.starts_with("collection-") {
                let n = bucket.projects.len();
                format!("{n} {}", if n == 1 { "project" } else { "projects" })
            } else {
                bucket.subtitle
            };
            let projects: Vec<NavNode> = bucket
                .projects
                .iter()
                .map(|project| {
                    let items = by_project.get(project.id.as_str()).cloned().unwrap_or_default();
                    let planned = items.iter().filter(|item| item.status != "dismissed").count();
                    let done = items.iter().filter(|item| item.status == "done").count();
                    let progress = if planned == 0 { 0 } else { ((done as f64 / planned as f64) * 100.0).round() as i64 };
                    let kind = project
                        .project_type
                        .clone()
                        .or_else(|| project.areas.first().cloned())
                        .unwrap_or_else(|| "WORK".into())
                        .to_uppercase();
                    let state = match project.status.as_str() {
                        "doing" => "In progress",
                        "done" => "Complete",
                        "archived" => "Archived",
                        _ => "Open",
                    };
                    let meta = format!("{kind} · {state}");
                    let work = work_children(&bucket.id, &project.id, None, &items, &HashSet::new());
                    let search = [project.name.as_str(), kind.as_str(), meta.as_str()]
                        .into_iter()
                        .map(str::to_owned)
                        .chain(work.iter().map(|node| node.search.clone()))
                        .collect::<Vec<_>>()
                        .join(" ")
                        .to_lowercase();
                    NavNode {
                        id: format!("{}::{}", bucket.id, project.id),
                        kind: NodeKind::Project,
                        label: project.name.clone(),
                        subtitle: None,
                        domain: domain.to_owned(),
                        project_id: Some(project.id.clone()),
                        work_id: None,
                        status: None,
                        meta,
                        search,
                        progress: Some(progress),
                        children: work,
                    }
                })
                .collect();
            let progress = if projects.is_empty() {
                0
            } else {
                (projects.iter().map(|p| p.progress.unwrap_or(0)).sum::<i64>() as f64 / projects.len() as f64).round() as i64
            };
            let search = [bucket.label.as_str(), subtitle.as_str()]
                .into_iter()
                .map(str::to_owned)
                .chain(projects.iter().map(|p| p.search.clone()))
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            NavNode {
                id: bucket.id,
                kind: NodeKind::Pole,
                label: bucket.label,
                subtitle: Some(subtitle),
                domain: domain.to_owned(),
                project_id: None,
                work_id: None,
                status: None,
                meta: String::new(),
                search,
                progress: Some(progress),
                children: projects,
            }
        })
        .collect()
}

/// The node the page's selection points at: the selected work item, else the selected project.
pub fn selected_id(nodes: &[NavNode], project: Option<&str>, work: Option<&str>) -> Option<String> {
    let project = project?;
    fn visit(node: &NavNode, project: &str, work: Option<&str>) -> Option<String> {
        if work.is_some() && node.work_id.as_deref() == work && node.project_id.as_deref() == Some(project) {
            return Some(node.id.clone());
        }
        for child in &node.children {
            if let Some(found) = visit(child, project, work) {
                return Some(found);
            }
        }
        (work.is_none() && node.kind == NodeKind::Project && node.project_id.as_deref() == Some(project))
            .then(|| node.id.clone())
    }
    nodes
        .iter()
        .find_map(|node| visit(node, project, work))
        .or_else(|| {
            nodes.iter().find_map(|node| {
                node.children
                    .iter()
                    .find(|child| child.kind == NodeKind::Project && child.project_id.as_deref() == Some(project))
                    .map(|child| child.id.clone())
            })
        })
}

/// The ids of the nodes above `selected` (they open so the selection is visible).
pub fn ancestors(nodes: &[NavNode], selected: &str) -> Vec<String> {
    fn path(branch: &[NavNode], selected: &str) -> Option<Vec<String>> {
        for node in branch {
            if node.id == selected {
                return Some(Vec::new());
            }
            if let Some(mut rest) = path(&node.children, selected) {
                rest.insert(0, node.id.clone());
                return Some(rest);
            }
        }
        None
    }
    path(nodes, selected).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PortalProjectEntity;

    fn item(id: &str, parent: Option<&str>, order: i32, status: &str, category: &str) -> PortalProjectWorkItem {
        PortalProjectWorkItem {
            id: id.into(),
            title: id.into(),
            category: category.into(),
            status: status.into(),
            project_id: Some("p1".into()),
            parent_id: parent.map(str::to_owned),
            order: Some(order),
            ..Default::default()
        }
    }

    #[test]
    fn properties_are_poles_over_their_projects_over_the_work_tree() {
        let page = PortalProjectsPage {
            active_domain: "properties".into(),
            projects: vec![PortalProject {
                id: "p1".into(),
                name: "Casa Luar Listing".into(),
                status: "doing".into(),
                project_type: Some("listing".into()),
                property_id: Some("h1".into()),
                ..Default::default()
            }],
            items: vec![
                item("b", None, 2, "done", "contracts"),
                item("a", None, 1, "open", "clients"),
                item("a1", Some("a"), 1, "dismissed", "task"),
            ],
            identity_names: [("property:h1".to_owned(), "Casa Luar".to_owned())].into(),
            ..Default::default()
        };
        let tree = build(&page);
        assert_eq!(tree.len(), 1);
        let pole = &tree[0];
        assert_eq!((pole.label.as_str(), pole.subtitle.as_deref(), pole.kind), ("Casa Luar", Some("Property"), NodeKind::Pole));
        let project = &pole.children[0];
        assert_eq!((project.label.as_str(), project.meta.as_str()), ("Casa Luar Listing", "LISTING · In progress"));
        assert_eq!(project.progress, Some(50), "one of two planned items done (dismissed excluded)");
        let work: Vec<&str> = project.children.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(work, ["a", "b"], "in order");
        assert_eq!(project.children[0].children[0].label, "a1", "nested under its parent");
    }

    #[test]
    fn a_project_without_an_anchor_in_the_domain_joins_the_collection() {
        let page = PortalProjectsPage {
            active_domain: "firm".into(),
            projects: vec![PortalProject { id: "p1".into(), name: "Office".into(), areas: vec!["firm".into()], ..Default::default() }],
            items: vec![PortalProjectWorkItem {
                id: "w".into(),
                project_id: Some("p1".into()),
                entity: Some(PortalProjectEntity { entity_type: "person".into(), id: "x".into() }),
                ..Default::default()
            }],
            ..Default::default()
        };
        let tree = build(&page);
        assert_eq!((tree[0].label.as_str(), tree[0].subtitle.as_deref()), ("Firm", Some("1 project")));
        let selected = selected_id(&tree, Some("p1"), Some("w")).unwrap();
        assert_eq!(ancestors(&tree, &selected), vec![tree[0].id.clone(), tree[0].children[0].id.clone()]);
    }
}
