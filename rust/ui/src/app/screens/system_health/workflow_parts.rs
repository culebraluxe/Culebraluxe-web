//! Workflow diagnostics' small parts: metric tiles, anomaly rows, labels, severity and status pills, instants.

#[allow(unused_imports)]
use super::*;

/// A table row: the same hairline separator and hover the portal's other tables use.
pub(super) const ROW: &str = "border-b border-[var(--portal-border)] transition-colors last:border-b-0 hover:bg-[var(--portal-blue-pale)]/40";

/// One metric card, with the emphasis the live card's red value used.
pub(super) fn workflow_metric(label: &str, value: &str, detail_text: &str, emphasize: bool) -> Html {
    html! {
        <div class="rounded-[var(--portal-panel-radius)] portal-glass-panel p-6">
            <div class="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
                { label }
            </div>
            <div class={classes!(
                "mt-4",
                "font-serif",
                "text-3xl",
                "font-light",
                if emphasize { "text-red-700" } else { "text-[var(--portal-navy)]" },
            )}>
                { value }
            </div>
            <div class="mt-2 text-xs font-light text-black/40">{ detail_text }</div>
        </div>
    }
}

/// One anomaly: its severity, what it says, and the instance it is about when it names one.
pub(super) fn anomaly_row(anomaly: &PortalWorkflowAnomaly, index: usize) -> Html {
    html! {
        <div
            key={format!("{}-{}", anomaly.kind, index)}
            class="flex flex-wrap items-start gap-x-3 gap-y-2 rounded-sm border border-[var(--portal-border)]/70 bg-white p-3"
        >
            { severity_pill(&anomaly.severity) }
            <div class="min-w-0 flex-1 text-sm font-light leading-6 text-black/70">
                { &anomaly.message }
            </div>
            if let Some(instance_id) = anomaly.instance_id.as_deref() {
                <span class="font-mono text-[11px] text-black/40">{ instance_id }</span>
            }
        </div>
    }
}

/// What an instance row is about: the property when the projection resolved one, else the subject, else nothing.
pub(super) fn instance_label(instance: &PortalWorkflowInstance) -> String {
    if let Some(property_name) = instance.property_name.as_deref() {
        return property_name.to_string();
    }
    match instance.subject_type.as_deref() {
        Some(subject_type) => format!(
            "{}:{}",
            subject_type,
            instance.subject_id.as_deref().unwrap_or("—")
        ),
        None => "—".to_string(),
    }
}

/// The definition's own name for an instance, when the deployed definitions were read alongside it.
pub(super) fn definition_name_for(
    diagnostics: &PortalWorkflowDiagnostics,
    instance: &PortalWorkflowInstance,
) -> Option<String> {
    diagnostics
        .definitions
        .iter()
        .find(|definition: &&PortalWorkflowDefinition| {
            definition.key == instance.definition_key
                && definition.version == instance.definition_version
        })
        .map(|definition| definition.name.clone())
}

/// A severity flag. `critical` is red, `warning` is the softer navy and everything else is the plain one — the live
/// component's three tones, which is why this is not a two-way choice.
pub(super) fn severity_pill(severity: &str) -> Html {
    let tone = match severity {
        "critical" => "bg-red-50 text-red-700 border-red-200",
        "warning" => {
            "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)] border-[var(--portal-border)]"
        }
        _ => "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)] border-[var(--portal-border)]",
    };
    html! {
        <span class={classes!(PILL, tone)}>
            { severity }
        </span>
    }
}

/// An instance's status, which says the outcome when there is one that matters.
///
/// THE LABEL IS NOT THE STATUS ALONE: an instance whose status is still `active` shows the status, and one that finished
/// shows its outcome — `failed`, `cancelled`, `conflict` — because "completed" is the least interesting thing about a run
/// that ended badly. That rule, and the three tones, are the live pill's.
pub(super) fn status_pill(status: &str, outcome: Option<&str>) -> Html {
    let mut tone =
        "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)] border-[var(--portal-border)]";
    if status == "completed" {
        tone = "bg-emerald-50 text-emerald-700 border-emerald-200";
    } else if status == "error" || outcome == Some("failed") || outcome == Some("conflict") {
        tone = "bg-red-50 text-red-700 border-red-200";
    } else if outcome == Some("cancelled") || status == "suspended" {
        tone =
            "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)] border-[var(--portal-border)]";
    }
    let label = match outcome {
        Some(outcome) if status != "active" => outcome.to_string(),
        _ => status.to_string(),
    };
    html! {
        <span class={classes!(PILL, tone)}>
            { label }
        </span>
    }
}

/// An instant as the reader's browser prints it, and the raw value when it is not a date at all.
///
/// THE BROWSER FORMATS IT, NOT THE SERVER: the live screen called `toLocaleString`, so a timestamp reads in the operator's
/// own zone. `js_sys::Date` is that formatter — reaching for it here is parity, not convenience.
pub(super) fn format_instant(value: &str) -> String {
    if value.is_empty() {
        return "—".to_string();
    }
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(value));
    if date.get_time().is_nan() {
        return value.to_string();
    }
    date.to_locale_string("en-GB", &wasm_bindgen::JsValue::UNDEFINED)
        .as_string()
        .unwrap_or_else(|| value.to_string())
}
