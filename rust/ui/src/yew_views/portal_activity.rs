//! `/portal/activity` — the unified activity feed, as the live screen renders it.
//!
//! PARITY, NOT A LIST OF CELLS. `components/portal/activity-feed.tsx` is a header ("Portal / Activity" with its
//! subtitle), a panel with its own heading and entry count, and one row per interaction: the time on the left, the
//! channel and its direction in a fixed column, then the person, the summary, and the property or deal the line belongs
//! to. The generic row renderer showed five cells and none of that structure; this is the screen.
//!
//! THE LINKS ARE LINKS. A line about a person links to that person's record and a line about a deal links to that deal —
//! which is why the payload carries `personId` and `dealId` rather than only the names it was rendering.

use yew::prelude::*;

use crate::model::{Msg, PortalActivityEntry};
use crate::yew_views::portal_shell::PortalShell;

/// The channel as the screen labels it, from `channelLabel` in the component.
fn channel_label(channel: &str) -> String {
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
        other => return capitalise(other),
    }
    .to_string()
}

/// The fallback the component used for a channel it does not know: its first letter up, the rest as it came.
fn capitalise(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

#[derive(Properties, PartialEq)]
pub struct ActivityProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

/// The portal screen wrapper: the shell, and the feed inside it.
pub struct Activity;

impl Component for Activity {
    type Message = ();
    type Properties = ActivityProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen =
            crate::model::screen("activity").expect("the activity screen is in the registry");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { self.feed(&props.model) }
            </PortalShell>
        }
    }
}

impl Activity {
    /// The header and the panel, which is the whole of the screen.
    fn feed(&self, model: &crate::model::Model) -> Html {
        let entries: &[PortalActivityEntry] = model
            .page
            .as_ref()
            .and_then(|page| page.portal.as_ref())
            .map(|portal| portal.activity.as_slice())
            .unwrap_or(&[]);
        html! {
            <div>
                <header class="mb-6">
                    <p class="text-xs font-light uppercase tracking-[0.28em] text-[var(--portal-blue-gray)]">{"Portal"}</p>
                    <h1 class="mt-3 font-serif text-3xl font-light">{"Activity"}</h1>
                    <p class="mt-2 text-sm font-light text-black/45">
                        {"Every interaction across the book, ordered by when it happened."}
                    </p>
                </header>
                <section class="overflow-hidden rounded-[var(--portal-panel-radius)] portal-glass-panel">
                    <div class="flex items-center justify-between border-b border-[var(--portal-border)] px-6 py-5">
                        <div>
                            <h2 class="font-serif text-2xl font-light">{"Unified Activity Feed"}</h2>
                            <p class="mt-1 text-xs font-light text-black/40">
                                {"Calls, email, messages, meetings, showings, website and notes."}
                            </p>
                        </div>
                        <div class="text-xs font-light text-black/35">
                            { format!("{} entries", entries.len()) }
                        </div>
                    </div>
                    if entries.is_empty() {
                        <div class="px-6 py-12 text-sm font-light text-black/40">
                            {"No activity recorded yet. Interactions will appear here as they are captured."}
                        </div>
                    } else {
                        { for entries.iter().map(entry_row) }
                    }
                </section>
            </div>
        }
    }
}

/// One interaction: the time, the channel and its direction, then who and what it was about.
fn entry_row(entry: &PortalActivityEntry) -> Html {
    let property = match (entry.deal_property_name.clone(), entry.deal_id.clone()) {
        (Some(deal_property), Some(deal_id)) => Some(html! {
            <a href={format!("/portal/deals/{deal_id}")}
                class="text-[var(--portal-navy-soft)] underline-offset-2 hover:underline">
                { deal_property }
            </a>
        }),
        (Some(deal_property), None) => Some(html! { { deal_property } }),
        (None, _) => None,
    };
    // The component shows the deal's property, and the interaction's own property beside it only when they differ.
    let second_property = entry
        .property_name
        .clone()
        .filter(|name| Some(name) != entry.deal_property_name.as_ref());
    html! {
        <div class="grid gap-3 border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0 md:grid-cols-[150px_130px_1fr]">
            <div class="text-xs font-light text-black/40">{ entry.occurred_at_label.clone() }</div>
            <div class="text-xs font-light uppercase tracking-[0.12em] text-[var(--portal-blue-gray)]">
                { channel_label(&entry.channel) }
                if let Some(direction) = entry.direction.clone() {
                    <span class="ml-2 normal-case text-black/35">{ direction }</span>
                }
            </div>
            <div>
                if let Some(person_id) = entry.person_id.clone() {
                    <a href={format!("/portal/clients/{person_id}")}
                        class="inline-flex min-h-11 items-center text-sm font-medium text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]">
                        { entry.person_name.clone().unwrap_or_else(|| "Unknown person".to_string()) }
                    </a>
                } else {
                    <div class="text-sm font-medium">
                        { entry.person_name.clone().unwrap_or_else(|| "Unknown person".to_string()) }
                    </div>
                }
                <div class="mt-1 text-sm font-light text-black/55">
                    { entry
                        .summary
                        .clone()
                        .or_else(|| entry.title.clone())
                        .unwrap_or_else(|| "Interaction".to_string()) }
                </div>
                if property.is_some() || second_property.is_some() {
                    <div class="mt-1 text-xs font-light text-black/40">
                        { property }
                        if let Some(name) = second_property {
                            { format!(" · {name}") }
                        }
                    </div>
                }
            </div>
        </div>
    }
}
