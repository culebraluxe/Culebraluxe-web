//! Shared classes, labels and number/currency formatting for the Deals views.

#[allow(unused_imports)]
use super::*;

pub(super) fn small_action_class() -> Classes {
    classes!(
        "min-h-8",
        "rounded-[var(--portal-tab-radius)]",
        "border",
        "border-[var(--portal-panel-border)]",
        "px-2.5",
        "text-[9px]",
        "font-medium",
        "uppercase",
        "tracking-[0.11em]",
        "text-[var(--portal-navy-soft)]",
        "disabled:opacity-35"
    )
}

pub(super) fn format_number(value: f64) -> String {
    if value.fract().abs() < 0.000_001 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

pub(super) fn group_integer(value: i64) -> String {
    let negative = value < 0;
    let digits = value.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let grouped = grouped.chars().rev().collect::<String>();
    format!("{}{}", if negative { "-" } else { "" }, grouped)
}

pub(super) fn empty_workspace(loading: bool, text: &str) -> Html {
    html! {
        <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] px-8 py-12 text-center">
            <h1 class="font-serif text-2xl font-light text-[var(--portal-navy)]">
                { if loading { "Contract workspace" } else { "Contract unavailable" } }
            </h1>
            <p class="mt-2 text-sm font-light text-black/45">{ text }</p>
            if !loading {
                <a href="/portal/deals" class="mt-5 inline-flex text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-navy-soft)]">
                    {"← Back to Contracts"}
                </a>
            }
        </section>
    }
}

pub(super) fn detail(label: &str, value: &str) -> Html {
    html! {
        <div>
            <div class="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">{ label }</div>
            <div class="mt-1 text-sm font-light leading-5 text-black/70">{ value }</div>
        </div>
    }
}

pub(super) fn field_class() -> Classes {
    classes!(
        "mt-1",
        "block",
        "h-10",
        "w-full",
        "rounded-[var(--portal-tab-radius)]",
        "border",
        "border-[var(--portal-panel-border)]",
        "bg-white/70",
        "px-3",
        "text-sm",
        "font-light",
        "normal-case",
        "tracking-normal",
        "text-black/70",
        "outline-none",
        "focus:border-[var(--portal-navy)]"
    )
}

pub(super) fn participant_role_label(role: &str) -> &str {
    match role {
        "client" => "Client",
        "owner" => "Owner",
        "seller" => "Seller",
        "other" => "Other",
        value => value,
    }
}

pub(super) fn channel_label(channel: &str) -> &str {
    match channel {
        "website" => "Website",
        "email" => "Email",
        "call" => "Phone Call",
        "imessage" => "iMessage",
        "sms" => "SMS",
        "meeting" => "Meeting",
        "showing" => "Showing",
        "document" => "Document",
        "manual" => "Manual Entry",
        "whatsapp" => "WhatsApp",
        value => value,
    }
}

pub(super) fn stage_label(stage: &str) -> &str {
    match stage {
        "new_lead" => "New Lead",
        "qualified" => "Qualified",
        "showing" => "Showing",
        "offer" => "Offer",
        "under_contract" => "Under Contract",
        "closed" => "Closed",
        other => other,
    }
}

pub(super) fn stage_class(stage: &str) -> &'static str {
    match stage {
        "new_lead" => "bg-black/5 text-black/50",
        "qualified" => "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]",
        "showing" => "bg-[var(--portal-mist-2)] text-[var(--portal-navy-soft)]",
        "offer" => "bg-[var(--portal-mist)] text-[var(--portal-navy)]",
        "under_contract" => "bg-[var(--portal-success-pale)] text-[var(--portal-success)]",
        "closed" => "bg-[var(--portal-navy)] text-white",
        _ => "bg-black/5 text-black/55",
    }
}

pub(super) fn title_case(value: &str) -> String {
    value
        .split(|ch: char| ch == '_' || ch.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn format_currency(value: Option<f64>) -> String {
    let Some(value) = value else {
        return "—".into();
    };
    let rounded = value.round() as i64;
    let negative = rounded < 0;
    let digits = rounded.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let grouped = grouped.chars().rev().collect::<String>();
    format!("{}{}{}", if negative { "-" } else { "" }, "$", grouped)
}
