//! Project Management → Catch-Up: the operator's daily queue, inside the Projects workspace.
//!
//! Three worklists behind one rail button, so the day starts in one place:
//!   * **Today** — every unfinished project work item due today, and anything overdue (which the TypeScript workbench
//!     silently dropped: a task due yesterday appeared in neither of its buckets).
//!   * **Unscheduled** — unfinished work with no due date yet.
//!   * **People** — the relationship queue ("who needs me now, and why"), derived by the Catch-Up service from
//!     interactions, showings, deals and tasks (`/api/portal/rust-ui/catch-up`). It used to be its own CORE screen.
//!
//! The work rows are the Projects page's own `items`; completing one or selecting one goes through the Projects
//! intents that already exist, so the editor below the list is the same one every other view uses.

use crate::app::api::{CatchUpAction, CatchUpRead};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::model::{PortalCatchUpPage, PortalPage, PortalProjectWorkItem, PortalProjectsPage};

use super::Msg;

/// Catch-Up's controls, and the relationship queue. `tab` empty means "Today, unless nothing is due today and
/// something is unscheduled" — the old workbench's opening rule.
#[derive(Debug, Clone, PartialEq)]
pub struct CatchUpState {
    pub tab: String,
    pub status: String,
    pub area: String,
    pub people: Remote<PortalCatchUpPage>,
    pub person: Option<String>,
    pub busy: bool,
    pub notice: Option<String>,
}

impl Default for CatchUpState {
    fn default() -> Self {
        Self {
            tab: String::new(),
            status: "all".into(),
            area: "all".into(),
            people: Remote::NotAsked,
            person: None,
            busy: false,
            notice: None,
        }
    }
}

/// Ask for the relationship queue once; later reads come back with each disposition.
pub(super) fn load_people(state: &mut CatchUpState) -> Cmd<Msg> {
    if !matches!(state.people, Remote::NotAsked) {
        return Cmd::none();
    }
    state.people = Remote::Loading;
    Cmd::request(CatchUpRead, Msg::PeopleLoaded)
}

pub(super) fn act(state: &mut CatchUpState, action: CatchUpAction) -> Cmd<Msg> {
    if state.busy {
        return Cmd::none();
    }
    state.busy = true;
    state.notice = None;
    Cmd::request(action, Msg::PeopleAnswered)
}

pub(super) fn apply_people(
    state: &mut CatchUpState,
    answer: Result<PortalPage, ApiError>,
    acted: bool,
) {
    match answer.and_then(|page| {
        page.catch_up
            .ok_or_else(|| ApiError::decode("The answer had no Catch-Up queue in it."))
    }) {
        Ok(page) => {
            let still_there = state
                .person
                .as_ref()
                .is_some_and(|id| page.items.iter().any(|item| &item.person_id == id));
            if !still_there {
                state.person = page.items.first().map(|item| item.person_id.clone());
            }
            state.people = Remote::Loaded(page);
            if acted {
                state.notice = Some("Catch-Up updated.".into());
            }
        }
        Err(error) => {
            if acted {
                // Keep the queue on screen; say why the disposition did not land.
                state.notice = Some(error.message);
            } else {
                state.people = Remote::Failed(error);
            }
        }
    }
}

/// The two work buckets, over the page's real rows. Nothing is invented: Today is dated work (today or overdue),
/// Unscheduled is undated work, and both leave out what is finished or dismissed — except that an item completed
/// today stays in Today, struck through, so the day's progress is visible.
pub(crate) struct Buckets<'a> {
    pub today: Vec<&'a PortalProjectWorkItem>,
    pub unscheduled: Vec<&'a PortalProjectWorkItem>,
}

pub(crate) fn buckets(projects: &PortalProjectsPage) -> Buckets<'_> {
    let today = projects.calendar_today.as_str();
    let mut today_items = Vec::new();
    let mut unscheduled = Vec::new();
    for item in projects
        .items
        .iter()
        .filter(|item| item.project_id.is_some())
    {
        if item.status == "dismissed" {
            continue;
        }
        match due_key(item) {
            Some(due) if !today.is_empty() && due.as_str() == today => today_items.push(item),
            Some(due) if !today.is_empty() && due.as_str() < today && item.status != "done" => {
                today_items.push(item)
            }
            None if item.status != "done" => unscheduled.push(item),
            _ => {}
        }
    }
    let order = |a: &&PortalProjectWorkItem, b: &&PortalProjectWorkItem| {
        status_rank(&a.status)
            .cmp(&status_rank(&b.status))
            .then_with(|| due_key(a).cmp(&due_key(b)))
            .then_with(|| project_name(projects, a).cmp(project_name(projects, b)))
            .then_with(|| a.title.cmp(&b.title))
    };
    today_items.sort_by(order);
    unscheduled.sort_by(order);
    Buckets {
        today: today_items,
        unscheduled,
    }
}

/// The worklist to show: the operator's choice, or the opening rule when they have not chosen.
pub(crate) fn effective_tab<'a>(state: &'a CatchUpState, buckets: &Buckets<'_>) -> &'a str {
    if !state.tab.is_empty() {
        return state.tab.as_str();
    }
    if buckets.today.is_empty() && !buckets.unscheduled.is_empty() {
        "unscheduled"
    } else {
        "today"
    }
}

/// A due date is a calendar date: the persisted `YYYY-MM-DD` prefix, never shifted through a timezone.
pub(crate) fn due_key(item: &PortalProjectWorkItem) -> Option<String> {
    item.due_at.as_deref().and_then(crate::calendar::date_key)
}

pub(crate) fn overdue(item: &PortalProjectWorkItem, today: &str) -> bool {
    item.status != "done"
        && due_key(item).is_some_and(|due| !today.is_empty() && due.as_str() < today)
}

pub(crate) fn status_matches(item: &PortalProjectWorkItem, status: &str) -> bool {
    match status {
        "open" => item.status != "done",
        "doing" => item.status == "doing",
        "done" => item.status == "done",
        _ => true,
    }
}

pub(crate) fn project_name<'a>(
    projects: &'a PortalProjectsPage,
    item: &PortalProjectWorkItem,
) -> &'a str {
    item.project_id
        .as_deref()
        .and_then(|id| projects.projects.iter().find(|project| project.id == id))
        .map(|project| project.name.as_str())
        .unwrap_or("")
}

fn status_rank(status: &str) -> u8 {
    match status {
        "doing" => 0,
        "open" => 1,
        "done" => 2,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page() -> PortalProjectsPage {
        serde_json::from_value(json!({
            "calendarToday": "2026-09-28",
            "projects": [
                { "id": "p1", "name": "Villa listing", "status": "doing" },
                { "id": "p2", "name": "Firm ops", "status": "open" }
            ],
            "items": [
                { "id": "due-today", "projectId": "p2", "title": "Sign listing", "status": "open", "dueAt": "2026-09-28" },
                { "id": "doing-today", "projectId": "p1", "title": "Photos", "status": "doing", "dueAt": "2026-09-28T00:00:00+00:00" },
                { "id": "overdue", "projectId": "p1", "title": "Order survey", "status": "open", "dueAt": "2026-09-20" },
                { "id": "done-late", "projectId": "p1", "title": "Old task", "status": "done", "dueAt": "2026-09-20" },
                { "id": "done-today", "projectId": "p1", "title": "Call seller", "status": "done", "dueAt": "2026-09-28" },
                { "id": "later", "projectId": "p1", "title": "Open house", "status": "open", "dueAt": "2026-10-05" },
                { "id": "undated", "projectId": "p2", "title": "Books", "status": "open" },
                { "id": "undated-done", "projectId": "p2", "title": "Filed", "status": "done" },
                { "id": "dismissed", "projectId": "p2", "title": "Skip", "status": "dismissed", "dueAt": "2026-09-28" },
                { "id": "adhoc", "title": "No project", "status": "open", "dueAt": "2026-09-28" }
            ]
        }))
        .expect("a projects page")
    }

    #[test]
    fn today_holds_due_today_and_overdue_work_in_working_order() {
        let page = page();
        let ids: Vec<&str> = buckets(&page)
            .today
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        assert_eq!(ids, ["doing-today", "overdue", "due-today", "done-today"]);
    }

    #[test]
    fn unscheduled_holds_only_unfinished_undated_work() {
        let page = page();
        let ids: Vec<&str> = buckets(&page)
            .unscheduled
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        assert_eq!(ids, ["undated"]);
    }

    #[test]
    fn it_opens_on_unscheduled_only_when_nothing_is_due_today() {
        let mut page = page();
        let state = CatchUpState::default();
        assert_eq!(effective_tab(&state, &buckets(&page)), "today");
        page.items.retain(|item| item.due_at.is_none());
        assert_eq!(effective_tab(&state, &buckets(&page)), "unscheduled");
    }

    #[test]
    fn the_people_queue_is_read_once_and_a_failed_disposition_keeps_it() {
        let mut state = CatchUpState::default();
        let request = load_people(&mut state).into_requests().remove(0);
        assert_eq!(request.path, "/api/portal/rust-ui/catch-up");
        assert!(
            load_people(&mut state).into_requests().is_empty(),
            "asked once"
        );

        let answer: PortalPage = serde_json::from_value(json!({ "catchUp": {
            "generatedAt": "2026-09-28T12:00:00Z", "total": 1, "highPriorityCount": 1,
            "items": [{ "personId": "p1", "displayName": "Alicia Rivera", "role": "buyer", "status": "active",
                "reasonCode": "unanswered_inbound", "reason": "An inbound message has no reply.", "priority": 100,
                "signalAt": "2026-09-28T11:00:00Z", "signalAtLabel": "Sep 28" }]
        }}))
        .expect("a catch-up page");
        apply_people(&mut state, Ok(answer), false);
        assert_eq!(state.person.as_deref(), Some("p1"));

        let request = act(
            &mut state,
            CatchUpAction::snooze("p1".into(), "unanswered_inbound".into(), 3),
        )
        .into_requests()
        .remove(0);
        assert_eq!(request.body.unwrap()["days"], 3);
        apply_people(&mut state, Err(ApiError::decode("refused")), true);
        assert!(
            matches!(state.people, Remote::Loaded(_)),
            "the queue stays on screen"
        );
        assert!(state.notice.is_some());
    }
}
