//! The person and project editors, and money formatting.

use super::*;

pub(super) fn person_editor(
    model: &Vm<'_>,
    person: Option<&PortalOpsPerson>,
    on_msg: &Callback<Msg>,
) -> Html {
    let Some(person) = person else {
        return empty_record("Person");
    };

    match model.ops.section.as_str() {
        "contact" => html! {
            <div class="space-y-4">
                {section_intro("Contact", "Person identities stay canonical. This slice shows existing email and phone values without inventing a second identity writer.")}
                <div class="grid gap-3 lg:grid-cols-2">
                    {readonly_card("Email", person.email.as_deref().unwrap_or("—"))}
                    {readonly_card("Phone", person.phone.as_deref().unwrap_or("—"))}
                    {readonly_card("Location", person.location.as_deref().unwrap_or("—"))}
                    {readonly_card("Role", &person.role)}
                </div>
            </div>
        },
        "relations" => html! {
            <div class="space-y-4">
                {section_intro("Relations", "Open the relationship workspace for communication history, properties, deals and operational context.")}
                <a href={format!("/portal/clients/{}", person.id)}
                    class="inline-flex h-10 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[11px] font-semibold uppercase tracking-[0.12em] text-white">
                    {"Open Client Relationship →"}
                </a>
            </div>
        },
        _ => html! {
            <div class="space-y-4">
                {section_intro("Person identity", "The same workbench shell, backed by the canonical Person service.")}
                if person.manual_override {
                    <p class="text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--portal-gold-muted)]">
                        {"Fixed by hand — the Apple Contacts sync will not overwrite this record"}
                    </p>
                }
                {field_panel(model, on_msg, "Canonical person", PERSON_FIELDS)}
                <div class="grid gap-3 lg:grid-cols-2">
                    {readonly_card("Role", &person.role)}
                    {readonly_card("Canonical ID", &person.id)}
                </div>
            </div>
        },
    }
}

pub(super) fn project_editor(
    model: &Vm<'_>,
    project: Option<&PortalOpsProject>,
    on_msg: &Callback<Msg>,
) -> Html {
    let Some(project) = project else {
        return empty_record("Project");
    };

    match model.ops.section.as_str() {
        "links" => html! {
            <div class="space-y-4">
                {section_intro("Project links", "Typed links connect a Project to its playbook and canonical Person, Property or Contract context.")}
                {field_panel(model, on_msg, "Bindings", PROJECT_LINKS)}
                <div class="grid gap-3 lg:grid-cols-2">
                    {readonly_card("Starts", project.starts_at.as_deref().unwrap_or("—"))}
                    {readonly_card("Ends", project.ends_at.as_deref().unwrap_or("—"))}
                </div>
            </div>
        },
        _ => html! {
            <div class="space-y-4">
                {section_intro("Project", "Edit the Project record without leaving OPPS. The full Project workspace remains the execution surface.")}
                {field_panel(model, on_msg, "Project facts", PROJECT_FIELDS)}
                <a href="/portal/projects"
                    class="inline-flex h-10 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-4 text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy)]">
                    {"Open Project Workspace →"}
                </a>
            </div>
        },
    }
}

/// A stored amount ("2350000" or "2350000.50") as US dollars: $2,350,000 / $2,350,000.50. Empty stays empty.
pub(in super::super) fn usd(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    let (whole, fraction) = raw
        .split_once('.')
        .map_or((raw, None), |(w, f)| (w, Some(f)));
    let digits: String = whole.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_start_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let mut grouped = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    match fraction {
        Some(cents) => format!(
            "${grouped}.{}",
            cents
                .chars()
                .filter(char::is_ascii_digit)
                .take(2)
                .collect::<String>()
        ),
        None => format!("${grouped}"),
    }
}
