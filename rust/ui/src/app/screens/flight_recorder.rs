//! TECH — the Flight Recorder console for one process instance (`/portal/tech/flight-recorder/:instanceId`).
//!
//! The trace console was a JavaScript widget and was deleted with the rest of the TypeScript (owner decision,
//! 2026-09-26). This is the Rust port: Yew owns the read, the 30-second refresh, the keep-the-last-good-trace
//! failure, the filters and the selection; the console renders FIVE views — Timeline, Workflow Graph, Causality
//! Graph, System Swimlane and Raw Events — all from one immutable transaction snapshot. No virtualizer library
//! and no graph library: the pure layouts live in `crate::flight_recorder` and the SVG is drawn by hand.
//!
//! FAITHFUL TO THE OWNER'S CONSOLE: three columns, the same tabs, the same semantic colours, the same
//! cross-selection (an event highlights its workflow node and vice versa), and the same keyboard (Up/Down/Escape)
//! on the timeline container rather than a window listener.

use yew::prelude::*;

use crate::app::api::FlightRecorderRead;
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::flight_recorder::{
    self as fr, adapt_flight_recorder_transaction, build_causal_event_pairs, build_causal_graph,
    build_selection_causality, build_unresolved_causes, format_clock, format_display_time,
    format_duration, format_offset, group_events_by_system, is_selected_causal_edge,
    layout_causal_graph, layout_master_workflow, raw_event_fields, CausalLayout,
    ConsoleWorkflowView, EventKind, EventStatus, FlightRecorderTrace, SystemId, TraceEvent,
};

/// How often the open console re-reads its trace.
const REFRESH_MS: u32 = 30_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Timeline,
    Workflow,
    Causality,
    Swimlane,
    Raw,
}

impl Tab {
    const ALL: [Tab; 5] = [
        Tab::Timeline,
        Tab::Workflow,
        Tab::Causality,
        Tab::Swimlane,
        Tab::Raw,
    ];

    fn label(self) -> &'static str {
        match self {
            Tab::Timeline => "Timeline",
            Tab::Workflow => "Workflow Graph",
            Tab::Causality => "Causality Graph",
            Tab::Swimlane => "System Swimlane",
            Tab::Raw => "Raw Events",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Density {
    #[default]
    Compact,
    Expanded,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<FlightRecorderTrace>,
    pub loading: bool,
    /// A failed refresh keeps the last good trace and says why.
    pub notice: Option<String>,
    pub selected_event_id: Option<String>,
    pub selected_node_id: Option<String>,
    pub tab: Tab,
    pub density: Density,
    pub query: String,
    pub kind_filters: Vec<EventKind>,
    /// The Raw Events row whose payload is expanded.
    pub expanded_raw: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<domain::FlightRecorderTransaction, ApiError>),
    Tick,
    RefreshRequested,
    TabSelected(Tab),
    DensitySelected(Density),
    QueryChanged(String),
    KindToggled(EventKind),
    FiltersCleared,
    EventSelected(String),
    NodeSelected(String),
    RawToggled(String),
    /// Arrow Up/Down move the selection; Escape clears it.
    KeyNav(KeyNav),
    /// A browser-only action (copy, download): no model change.
    Noop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyNav {
    Up,
    Down,
    Escape,
    /// Any other key: nothing happens, but the handler still returns a message.
    None,
}

pub struct FlightRecorder;

fn read(ctx: &ScreenCtx, model: &mut Model) -> Cmd<Msg> {
    let Some(instance_id) = ctx
        .id
        .clone()
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
    else {
        model.notice = Some("Flight Recorder requires a process-instance id.".into());
        model.loading = false;
        return Cmd::none();
    };
    model.loading = true;
    Cmd::request(FlightRecorderRead { instance_id }, Msg::Loaded)
}

/// The events the current filters keep, in order. Pure; the view and the reducer share it.
fn filtered_ids(model: &Model) -> Vec<String> {
    let Some(trace) = model.read.loaded() else {
        return Vec::new();
    };
    let query = model.query.trim().to_lowercase();
    trace
        .events
        .iter()
        .filter(|event| {
            if !model.kind_filters.is_empty() && !model.kind_filters.contains(&event.kind) {
                return false;
            }
            if query.is_empty() {
                return true;
            }
            let mut haystack = format!(
                "{} {} {} {} {}",
                event.title,
                event.event_type,
                event.system.label(),
                event.id,
                event.payload_json()
            );
            for (_, value) in &event.details {
                haystack.push(' ');
                haystack.push_str(value);
            }
            haystack.to_lowercase().contains(&query)
        })
        .map(|event| event.id.clone())
        .collect()
}

impl Screen for FlightRecorder {
    type Model = Model;
    type Msg = Msg;

    fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        let mut model = Model {
            read: Remote::Loading,
            tab: Tab::Timeline,
            density: Density::Compact,
            ..Model::default()
        };
        let read = read(ctx, &mut model);
        (model, Cmd::batch([read, Cmd::after(REFRESH_MS, Msg::Tick)]))
    }

    fn update(model: &mut Model, msg: Msg, ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::Loaded(answer) => {
                model.loading = false;
                match answer {
                    Ok(transaction) => {
                        model.read =
                            Remote::Loaded(adapt_flight_recorder_transaction(&transaction));
                        model.notice = None;
                        // A selection that no longer exists is cleared rather than left dangling.
                        if let Some(selected) = model.selected_event_id.clone() {
                            let exists = model
                                .read
                                .loaded()
                                .is_some_and(|trace| trace.events.iter().any(|e| e.id == selected));
                            if !exists {
                                model.selected_event_id = None;
                                model.selected_node_id = None;
                            }
                        }
                    }
                    // A failed refresh keeps the line on screen and says why.
                    Err(error) if model.read.loaded().is_some() => {
                        model.notice = Some(error.message)
                    }
                    Err(error) => model.read = Remote::Failed(error),
                }
                Cmd::none()
            }
            Msg::Tick => {
                let refresh = if model.loading || model.read.loaded().is_none() {
                    Cmd::none()
                } else {
                    read(ctx, model)
                };
                Cmd::batch([refresh, Cmd::after(REFRESH_MS, Msg::Tick)])
            }
            Msg::RefreshRequested if !model.loading => {
                model.notice = None;
                read(ctx, model)
            }
            Msg::RefreshRequested => Cmd::none(),
            Msg::TabSelected(tab) => {
                model.tab = tab;
                Cmd::none()
            }
            Msg::DensitySelected(density) => {
                model.density = density;
                Cmd::none()
            }
            Msg::QueryChanged(query) => {
                model.query = query;
                Cmd::none()
            }
            Msg::KindToggled(kind) => {
                if let Some(index) = model.kind_filters.iter().position(|k| *k == kind) {
                    model.kind_filters.remove(index);
                } else {
                    model.kind_filters.push(kind);
                }
                Cmd::none()
            }
            Msg::FiltersCleared => {
                model.kind_filters.clear();
                model.query.clear();
                Cmd::none()
            }
            Msg::EventSelected(id) => {
                let node = model
                    .read
                    .loaded()
                    .and_then(|trace| trace.events.iter().find(|e| e.id == id))
                    .and_then(|event| event.workflow_node_id.clone());
                model.selected_event_id = Some(id);
                model.selected_node_id = node;
                Cmd::none()
            }
            Msg::NodeSelected(id) => {
                model.selected_node_id =
                    (model.selected_node_id.as_deref() != Some(id.as_str())).then_some(id);
                Cmd::none()
            }
            Msg::RawToggled(id) => {
                model.expanded_raw =
                    (model.expanded_raw.as_deref() != Some(id.as_str())).then_some(id);
                Cmd::none()
            }
            Msg::KeyNav(KeyNav::Escape) => {
                model.selected_event_id = None;
                model.selected_node_id = None;
                Cmd::none()
            }
            Msg::Noop => Cmd::none(),
            Msg::KeyNav(direction) => {
                let ids = filtered_ids(model);
                if ids.is_empty() {
                    return Cmd::none();
                }
                let current = model
                    .selected_event_id
                    .as_ref()
                    .and_then(|id| ids.iter().position(|candidate| candidate == id));
                let next = match direction {
                    KeyNav::Down => Some(
                        current
                            .map(|index| (index + 1).min(ids.len() - 1))
                            .unwrap_or(0),
                    ),
                    KeyNav::Up => Some(current.map(|index| index.saturating_sub(1)).unwrap_or(0)),
                    KeyNav::Escape => None,
                    KeyNav::None => return Cmd::none(),
                };
                if let Some(index) = next {
                    let id = ids[index].clone();
                    let node = model
                        .read
                        .loaded()
                        .and_then(|trace| trace.events.iter().find(|e| e.id == id))
                        .and_then(|event| event.workflow_node_id.clone());
                    model.selected_event_id = Some(id);
                    model.selected_node_id = node;
                }
                Cmd::none()
            }
        }
    }

    fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        let console = match &model.read {
            Remote::NotAsked => Html::default(),
            Remote::Loading => loading(),
            Remote::Failed(error) => load_failure(error, link),
            Remote::Loaded(trace) => console(model, trace, link),
        };
        html! {
            <div class="relative">
                { console }
                if let Some(notice) = model.notice.clone() {
                    <div class="pointer-events-none absolute inset-x-4 top-16 z-[80] flex items-center justify-between gap-3 rounded-md border border-amber-400/25 bg-[#111827]/95 px-3 py-2 text-[10px] text-amber-100 shadow-xl" role="alert">
                        <span>{ format!("Refresh failed — showing the last good trace. {notice}") }</span>
                    </div>
                }
                if ctx.id.is_none() {
                    <p class="pt-2 text-center font-mono text-[10px] text-black/40">{ "No instance id in the URL." }</p>
                }
            </div>
        }
    }
}

fn loading() -> Html {
    html! {
        <div class="grid min-h-[42rem] place-items-center rounded-xl border border-white/10 bg-[#0b1220] text-sm text-slate-400" data-screen-state="loading">
            <div class="text-center">
                <div class="mx-auto h-7 w-7 animate-spin rounded-full border-2 border-white/10 border-t-[#c6a15b]" />
                <p class="mt-3">{ "Loading trace…" }</p>
            </div>
        </div>
    }
}

fn load_failure(error: &ApiError, link: &Link<Msg>) -> Html {
    html! {
        <div class="grid min-h-[42rem] place-items-center rounded-xl border border-white/10 bg-[#0b1220] p-6 text-slate-300" data-screen-state="failed">
            <div class="max-w-xl rounded-lg border border-rose-400/25 bg-rose-400/[0.05] px-6 py-5" role="alert">
                <p class="text-[10px] font-semibold uppercase tracking-[0.16em] text-rose-300">
                    { "Flight Recorder could not load" }
                </p>
                <p class="mt-2 text-sm leading-6 text-slate-300">{ error.message.clone() }</p>
                <button type="button" onclick={link.callback(|_: MouseEvent| Msg::RefreshRequested)}
                    class="mt-4 rounded-md border border-white/15 px-3 py-1.5 text-[10px] font-medium uppercase tracking-[0.12em] text-white hover:border-[#c6a15b]/50 hover:text-[#e0c489]">
                    { "Retry" }
                </button>
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// The console
// ---------------------------------------------------------------------------

fn console(model: &Model, trace: &FlightRecorderTrace, link: &Link<Msg>) -> Html {
    let selected = selected_event(trace, model);
    html! {
        <div class="flex h-[calc(100vh-6.5rem)] min-h-[44rem] flex-col overflow-hidden rounded-xl border border-white/10 bg-[#0b1220] text-slate-100">
            { header(model, link) }
            <div class="flex min-h-0 flex-1">
                { left_rail(trace, model, link) }
                <main class="flex min-w-0 flex-1 flex-col border-x border-white/5">
                    { toolbar(model, trace, link) }
                    { main_view(trace, model, link) }
                </main>
                { event_details(selected, link) }
            </div>
        </div>
    }
}

fn header(model: &Model, link: &Link<Msg>) -> Html {
    html! {
        <header class="flex h-14 shrink-0 items-center gap-4 border-b border-white/5 px-4">
            <div class="flex items-center gap-2">
                <span class="grid h-8 w-8 place-items-center rounded-lg bg-slate-800 text-sm">{ "FR" }</span>
                <div>
                    <div class="text-sm font-semibold">{ "Flight Recorder" }</div>
                    <div class="text-[11px] text-slate-400">{ "End-to-end transaction timeline & causality" }</div>
                </div>
            </div>
            <div class="mx-auto w-full max-w-xl">
                <input
                    value={model.query.clone()}
                    oninput={link.callback(|event: InputEvent| Msg::QueryChanged(crate::app::template::input_value(&event)))}
                    placeholder="Search events, entities, correlations..."
                    class="w-full rounded-lg border border-white/10 bg-slate-900/80 px-3 py-1.5 text-sm text-slate-100 outline-none ring-sky-500/40 placeholder:text-slate-500 focus:ring-2"
                />
            </div>
            <div class="flex items-center gap-2 text-xs">
                <span class="rounded-full border border-white/10 px-2.5 py-1 text-emerald-300">{ "Auto Refresh · On" }</span>
                <span class="rounded-full border border-emerald-500/30 bg-emerald-500/10 px-2.5 py-1 text-emerald-300">{ "Live" }</span>
            </div>
        </header>
    }
}

fn left_rail(trace: &FlightRecorderTrace, model: &Model, link: &Link<Msg>) -> Html {
    let summary = &trace.summary;
    let context = &summary.business_context;
    let context_rows: Vec<(&str, Option<String>)> = vec![
        ("Deal", context.deal.clone()),
        ("Property", context.property.clone()),
        ("Client", context.client.clone()),
        ("Workflow", context.workflow.clone()),
        ("Initiated By", context.initiated_by.clone()),
        (
            "At",
            context
                .initiated_at
                .clone()
                .map(|at| format_display_time(fr::epoch_ms(&at))),
        ),
    ];
    let mut counts: Vec<(EventKind, usize)> = EventKind::ALL
        .iter()
        .map(|kind| {
            (
                *kind,
                trace
                    .events
                    .iter()
                    .filter(|event| event.kind == *kind)
                    .count(),
            )
        })
        .collect();
    counts.retain(|(_, count)| *count > 0);
    html! {
        <aside class="flex w-[280px] shrink-0 flex-col gap-4 overflow-y-auto p-4">
            <div>
                <div class="text-[10px] uppercase tracking-wide text-slate-500">{ "Correlation ID" }</div>
                <div class="mt-1 break-all font-mono text-[11px] text-slate-200">{ summary.correlation_id.clone() }</div>
                <div class="mt-2 flex items-center gap-2 text-xs">
                    <span class="text-slate-400">{ format!("Root: {}", summary.root_title) }</span>
                    <span class={classes!("rounded-full", "px-2", "py-0.5", "text-[10px]", summary.root_kind.token_chip())}>
                        { summary.root_kind.label() }
                    </span>
                </div>
            </div>
            <dl class="grid grid-cols-4 gap-2 text-center">
                { stat("Duration", &format_duration(summary.duration_ms), false) }
                { stat("Events", &summary.event_count.to_string(), false) }
                { stat("Systems", &summary.system_count.to_string(), false) }
                { stat("Status", summary.status.label(), true) }
            </dl>
            if let Some(window) = summary.instances {
                <p class="rounded-md border border-amber-400/30 bg-amber-400/[0.06] px-2 py-1.5 text-[10px] text-amber-200/90">
                    { format!("Showing the newest {} of {} workflow instances for this subject.", window.shown, window.total) }
                </p>
            }
            <section>
                <h2 class="text-[10px] uppercase tracking-wide text-slate-500">{ "Business Context" }</h2>
                <dl class="mt-2 space-y-1.5 text-xs">
                    { for context_rows.into_iter().filter_map(|(label, value)| value.map(|value| (label, value))).map(|(label, value)| html! {
                        <div class="grid grid-cols-[88px_1fr] gap-2">
                            <dt class="text-slate-500">{ label }</dt>
                            <dd class="text-slate-200">{ value }</dd>
                        </div>
                    }) }
                </dl>
            </section>
            <section>
                <h2 class="text-[10px] uppercase tracking-wide text-slate-500">{ "Master Workflow" }</h2>
                { master_map(trace.workflow.as_ref(), model, link, false) }
            </section>
            <section>
                <ul class="space-y-1 text-xs">
                    { for counts.into_iter().map(|(kind, count)| html! {
                        <li>
                            <button type="button" onclick={link.callback(move |_: MouseEvent| Msg::KindToggled(kind))}
                                class={classes!("flex", "w-full", "items-center", "justify-between", "rounded", "px-1", "py-0.5",
                                    if model.kind_filters.contains(&kind) { "bg-white/5" } else { "" })}>
                                <span class="flex items-center gap-2">
                                    <span class={classes!("h-2.5", "w-2.5", "rounded-full", kind.token_bg())} />
                                    { kind.label() }
                                </span>
                                <span class="text-slate-500">{ count.to_string() }</span>
                            </button>
                        </li>
                    }) }
                </ul>
            </section>
        </aside>
    }
}

fn stat(label: &str, value: &str, status: bool) -> Html {
    html! {
        <div>
            <dt class="text-[10px] uppercase text-slate-500">{ label.to_owned() }</dt>
            <dd class={classes!("mt-0.5", "text-sm", if status { "text-emerald-400" } else { "text-white" })}>{ value.to_owned() }</dd>
        </div>
    }
}

fn toolbar(model: &Model, trace: &FlightRecorderTrace, link: &Link<Msg>) -> Html {
    let filtered_ids = filtered_ids(model);
    let filtered = filtered_ids.len();
    let filtered_events: Vec<TraceEvent> = filtered_ids
        .iter()
        .filter_map(|id| trace.events.iter().find(|event| &event.id == id).cloned())
        .collect();
    let filtered_label = if model.kind_filters.is_empty() && model.query.trim().is_empty() {
        "Filter".to_string()
    } else {
        format!("Filter ({filtered})")
    };
    let export = fr::trace_to_json(trace);
    let export_name = format!("flight-recorder-{}.json", trace.summary.correlation_id);
    let download = fr::events_to_json(&filtered_events);
    let download_name = format!(
        "flight-recorder-filtered-{}.json",
        trace.summary.correlation_id
    );
    html! {
        <div class="flex items-center justify-between gap-3 border-b border-white/5 px-4">
            <nav class="flex gap-1" aria-label="Trace views">
                { for Tab::ALL.into_iter().map(|tab| {
                    let active = model.tab == tab;
                    html! {
                        <button type="button" onclick={link.callback(move |_: MouseEvent| Msg::TabSelected(tab))}
                            class={classes!("relative", "px-3", "py-3", "text-sm",
                                if active { "text-white" } else { "text-slate-400 hover:text-slate-200" })}>
                            { tab.label() }
                            if active {
                                <span class="absolute inset-x-2 -bottom-px h-0.5 rounded bg-sky-400" />
                            }
                        </button>
                    }
                }) }
            </nav>
            <div class="flex items-center gap-2 py-2">
                <button type="button" onclick={link.callback(|_: MouseEvent| Msg::FiltersCleared)}
                    class="rounded-md border border-white/10 px-2.5 py-1.5 text-xs text-slate-300">
                    { filtered_label }
                </button>
                <div class="flex rounded-md border border-white/10 p-0.5 text-xs">
                    { for [Density::Compact, Density::Expanded].into_iter().map(|density| {
                        let active = model.density == density;
                        html! {
                            <button type="button" onclick={link.callback(move |_: MouseEvent| Msg::DensitySelected(density))}
                                class={classes!("rounded", "px-2.5", "py-1", "capitalize",
                                    if active { "bg-slate-700 text-white" } else { "text-slate-400" })}>
                                { match density { Density::Compact => "compact", Density::Expanded => "expanded" } }
                            </button>
                        }
                    }) }
                </div>
                <button type="button" onclick={link.callback(move |_: MouseEvent| { download_json(&export, &export_name); Msg::Noop })}
                    class="rounded-md border border-white/10 px-2.5 py-1.5 text-xs text-slate-300">
                    { "Export" }
                </button>
                <button type="button" onclick={link.callback(move |_: MouseEvent| { download_json(&download, &download_name); Msg::Noop })}
                    class="rounded-md border border-white/10 px-2.5 py-1.5 text-xs text-slate-300">
                    { "Download" }
                </button>
            </div>
        </div>
    }
}

fn main_view(trace: &FlightRecorderTrace, model: &Model, link: &Link<Msg>) -> Html {
    match model.tab {
        Tab::Timeline => timeline(trace, model, link),
        Tab::Workflow => master_map(trace.workflow.as_ref(), model, link, true),
        Tab::Causality => causality(trace, model, link),
        Tab::Swimlane => swimlane(trace, model, link),
        Tab::Raw => raw_events(trace, model, link),
    }
}

// ---------------------------------------------------------------------------
// Timeline
// ---------------------------------------------------------------------------

fn timeline(trace: &FlightRecorderTrace, model: &Model, link: &Link<Msg>) -> Html {
    let row_height = if model.density == Density::Compact {
        "56px"
    } else {
        "88px"
    };
    let ids = filtered_ids(model);
    if ids.is_empty() {
        return empty_view("No events match the current filters.");
    }
    let events: Vec<&TraceEvent> = ids
        .iter()
        .filter_map(|id| trace.events.iter().find(|event| &event.id == id))
        .collect();
    html! {
        <div class="flex min-h-0 flex-1 flex-col">
            <div class="grid grid-cols-[108px_minmax(0,1.2fr)_minmax(0,1.4fr)_140px] border-b border-white/5 px-3 py-2 text-[10px] uppercase tracking-wide text-slate-500">
                <div>{ "Time" }</div>
                <div>{ "Event" }</div>
                <div>{ "Details" }</div>
                <div>{ "System" }</div>
            </div>
            <div tabindex="0" onkeydown={link.callback(|event: KeyboardEvent| {
                    // The owner's console called preventDefault on the arrows; without it the focused
                    // scroll container moves the viewport as well as the selection.
                    let msg = match event.key().as_str() {
                        "ArrowDown" => Msg::KeyNav(KeyNav::Down),
                        "ArrowUp" => Msg::KeyNav(KeyNav::Up),
                        "Escape" => Msg::KeyNav(KeyNav::Escape),
                        _ => Msg::KeyNav(KeyNav::None),
                    };
                    if matches!(event.key().as_str(), "ArrowDown" | "ArrowUp") {
                        event.prevent_default();
                    }
                    msg
                })}
                class="min-h-0 flex-1 overflow-auto outline-none" role="grid" aria-rowcount={events.len().to_string()}>
                { for events.into_iter().map(|event| {
                    let selected = model.selected_event_id.as_deref() == Some(event.id.as_str());
                    let matches_node = model.selected_node_id.is_some()
                        && event.workflow_node_id.as_deref() == model.selected_node_id.as_deref();
                    let details: Vec<&(String, String)> = if model.density == Density::Compact {
                        event.details.iter().take(2).collect()
                    } else {
                        event.details.iter().collect()
                    };
                    let id = event.id.clone();
                    html! {
                        <button type="button" role="row" aria-selected={selected.to_string()} onclick={link.callback(move |_: MouseEvent| Msg::EventSelected(id.clone()))}
                            style={format!("height:{row_height}")}
                            class={classes!("grid", "h-full", "w-full", "grid-cols-[108px_minmax(0,1.2fr)_minmax(0,1.4fr)_140px]", "items-center", "overflow-hidden", "border-b", "border-white/5", "px-3", "text-left", "text-sm",
                                if selected { "bg-slate-800" } else if matches_node { "bg-amber-500/10" } else { "hover:bg-slate-900/80" },
                                if matches_node { "border-l-2 border-l-amber-400" } else { "" })}>
                            <div class="font-mono text-[11px] leading-tight text-slate-300">
                                <div>{ format_clock(event.occurred_at_ms) }</div>
                                <div class="text-slate-500">{ format_offset(event.offset_ms) }</div>
                            </div>
                            <div class="flex min-w-0 items-center gap-2">
                                <span class={classes!("grid", "h-7", "w-7", "shrink-0", "place-items-center", "rounded-md", "text-white", event.kind.token_bg())}>
                                    { kind_glyph(event.kind, "h-4 w-4") }
                                </span>
                                <div class="min-w-0">
                                    <div class="truncate font-medium">{ event.title.clone() }</div>
                                    <div class="truncate text-[11px] text-slate-500">{ event.subtitle.clone() }</div>
                                </div>
                            </div>
                            <div class="min-w-0 text-[12px] leading-snug text-slate-300">
                                { for details.into_iter().map(|(key, value)| html! {
                                    <div class="truncate"><span class="text-slate-500">{ format!("{key}: ") }</span>{ value.clone() }</div>
                                }) }
                            </div>
                            <div>
                                <span class={classes!("inline-flex", "rounded-md", "px-2", "py-0.5", "text-[11px]", event.system.chip_class())}>
                                    { event.system.label() }
                                </span>
                            </div>
                        </button>
                    }
                }) }
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Master Workflow map (mini + full)
// ---------------------------------------------------------------------------

fn master_map(
    workflow: Option<&ConsoleWorkflowView>,
    model: &Model,
    link: &Link<Msg>,
    full: bool,
) -> Html {
    let Some(workflow) = workflow.filter(|workflow| !workflow.nodes.is_empty()) else {
        return if full {
            empty_view("No master workflow to map.")
        } else {
            html! { <div class="mt-2 rounded-lg border border-white/5 bg-slate-900/40 p-3 text-xs text-slate-500">{ "No master workflow to map." }</div> }
        };
    };
    let layout = layout_master_workflow(workflow);
    let scale = if full { 1.9 } else { 1.0 };
    let wrapper = if full {
        "min-h-0 w-full flex-1 overflow-auto bg-slate-900/20"
    } else {
        "mt-2 h-52 overflow-auto rounded-lg border border-white/5 bg-slate-900/40"
    };
    html! {
        <div class={wrapper}>
            <svg width={(layout.width * scale).to_string()} height={(layout.height * scale).to_string()}
                viewBox={format!("0 0 {} {}", layout.width, layout.height)} class="min-w-full">
                { for layout.edges.iter().map(|edge| html! {
                    <line x1={edge.x1.to_string()} y1={edge.y1.to_string()} x2={edge.x2.to_string()} y2={edge.y2.to_string()}
                        stroke="rgba(255,255,255,0.15)" stroke-width="1" />
                }) }
                { for layout.nodes.iter().map(|node| {
                    let selected = model.selected_node_id.as_deref() == Some(node.id.as_str());
                    let future = node.state == "NOT_VISITED";
                    let current = node.state == "CURRENT";
                    let failed = node.state == "FAILED";
                    let recovered = node.state == "RECOVERED";
                    let ring_color = if selected { "#ffffff" }
                        else if current { "#c6a15b" }
                        else if recovered { "#fbbf24" }
                        else if failed { "#f87171" }
                        else { "rgba(255,255,255,0.25)" };
                    let ring_width = if selected || current { "2.5" } else if failed { "2" } else { "1" };
                    let tooltip = format!("{} — {} ({})", node.name, node.state, node.semantic_kind.label());
                    let id = node.id.clone();
                    html! {
                        <g transform={format!("translate({},{})", node.x, node.y)}>
                            <circle r={(layout.node_radius + 2.0).to_string()} fill="none" stroke={ring_color}
                                stroke-width={ring_width} opacity={if future { "0.4" } else { "1" }} />
                            <circle r={layout.node_radius.to_string()} fill={node.semantic_kind.hex()}
                                opacity={if future { "0.35" } else { "0.92" }}
                                stroke={if failed { "#f87171" } else { "rgba(0,0,0,0.25)" }}
                                stroke-width={if failed { "1.5" } else { "0.75" }} />
                            { kind_glyph_at(node.semantic_kind, -7.0, -7.0, 14.0, 14.0) }
                            <circle r={(layout.node_radius + 9.0).to_string()} fill="transparent"
                                style="cursor:pointer" onclick={link.callback(move |_: MouseEvent| Msg::NodeSelected(id.clone()))}>
                                <title>{ tooltip }</title>
                            </circle>
                            <text y={(-layout.node_radius - 5.0).to_string()} text-anchor="middle" font-size="8" class="fill-slate-300">{ node.name.clone() }</text>
                            if node.state == "COMPLETED" {
                                <text text-anchor="middle" font-size="9" class="fill-white">{ "✓" }</text>
                            }
                        </g>
                    }
                }) }
            </svg>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Causality graph
// ---------------------------------------------------------------------------

fn causality(trace: &FlightRecorderTrace, model: &Model, link: &Link<Msg>) -> Html {
    let ids = filtered_ids(model);
    let events: Vec<TraceEvent> = ids
        .iter()
        .filter_map(|id| trace.events.iter().find(|event| &event.id == id).cloned())
        .collect();
    let graph = build_causal_graph(&events);
    let layout = layout_causal_graph(&graph);
    if layout.nodes.is_empty() {
        return empty_view("No causal relationships recorded for these events.");
    }
    let selected_node = model.selected_event_id.as_deref().and_then(|selected| {
        layout
            .nodes
            .iter()
            .find(|node| node.node.members.iter().any(|member| member.id == selected))
            .map(|node| node.node.id.clone())
    });
    let unresolved: Vec<&str> = build_unresolved_causes(&events)
        .iter()
        .map(|cause| events[cause.event].id.as_str())
        .collect();
    html! {
        <div class="min-h-0 flex-1 overflow-auto">
            { causal_svg(&layout, selected_node.as_deref(), &unresolved, link) }
        </div>
    }
}

fn causal_svg(
    layout: &CausalLayout,
    selected_node: Option<&str>,
    unresolved_event_ids: &[&str],
    link: &Link<Msg>,
) -> Html {
    let parents: Vec<String> = selected_node
        .map(|node| {
            layout
                .edges
                .iter()
                .filter(|edge| edge.target == node)
                .map(|edge| edge.source.clone())
                .collect()
        })
        .unwrap_or_default();
    let children: Vec<String> = selected_node
        .map(|node| {
            layout
                .edges
                .iter()
                .filter(|edge| edge.source == node)
                .map(|edge| edge.target.clone())
                .collect()
        })
        .unwrap_or_default();
    html! {
        <svg width={layout.width.to_string()} height={layout.height.to_string()}
            viewBox={format!("0 0 {} {}", layout.width, layout.height)} class="min-w-full">
            <defs>
                <marker id="causal-arrow" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto" markerUnits="strokeWidth">
                    <path d="M0,0 L7,3.5 L0,7 z" fill="rgba(255,255,255,0.4)" />
                </marker>
                <marker id="causal-arrow-active" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto" markerUnits="strokeWidth">
                    <path d="M0,0 L7,3.5 L0,7 z" fill="#c6a15b" />
                </marker>
            </defs>
            { for layout.edges.iter().filter_map(|edge| {
                let a = layout.nodes.iter().find(|node| node.node.id == edge.source)?;
                let b = layout.nodes.iter().find(|node| node.node.id == edge.target)?;
                let active = is_selected_causal_edge(edge, selected_node);
                Some(html! {
                    <line x1={a.x.to_string()} y1={a.y.to_string()} x2={b.x.to_string()} y2={b.y.to_string()}
                        stroke={if active { "#c6a15b" } else { "rgba(255,255,255,0.16)" }}
                        stroke-width={if active { "1.6" } else { "1" }}
                        marker-end={if active { "url(#causal-arrow-active)" } else { "url(#causal-arrow)" }} />
                })
            }) }
            { for layout.nodes.iter().map(|node| {
                let selected = selected_node == Some(node.node.id.as_str());
                let is_parent = parents.contains(&node.node.id);
                let is_child = children.contains(&node.node.id);
                let has_unresolved = node.node.members.iter().any(|member| unresolved_event_ids.contains(&member.id.as_str()));
                let dim = selected_node.is_some() && !selected && !is_parent && !is_child;
                let radius = layout.node_radius + if selected { 3.0 } else if is_parent || is_child { 2.0 } else { 0.0 };
                let label = if node.node.count > 1 {
                    format!("{} ×{}", node.node.label, node.node.count)
                } else {
                    node.node.label.clone()
                };
                let first_id = node.node.members[0].id.clone();
                html! {
                    <g transform={format!("translate({},{})", node.x, node.y)} opacity={if dim { "0.4" } else { "1" }}>
                        <circle r={radius.to_string()} fill={node.node.color.hex()}
                            stroke={if selected { "#fff" } else if is_parent { "#c6a15b" } else { "rgba(255,255,255,0.25)" }}
                            stroke-width={if selected || is_parent || is_child { "2" } else { "1" }} />
                        <circle r={(layout.node_radius + 8.0).to_string()} fill="transparent" style="cursor:pointer"
                            onclick={link.callback(move |_: MouseEvent| Msg::EventSelected(first_id.clone()))}>
                            <title>{ node.node.label.clone() }</title>
                        </circle>
                        <text y={(-layout.node_radius - 4.0).to_string()} text-anchor="middle" font-size="8" class="fill-slate-300">{ label }</text>
                        if has_unresolved {
                            <text y={(layout.node_radius + 12.0).to_string()} text-anchor="middle" font-size="7" class="fill-amber-400">{ "? missing cause" }</text>
                        }
                    </g>
                }
            }) }
        </svg>
    }
}

// ---------------------------------------------------------------------------
// System Swimlane
// ---------------------------------------------------------------------------

fn swimlane(trace: &FlightRecorderTrace, model: &Model, link: &Link<Msg>) -> Html {
    let ids = filtered_ids(model);
    let events: Vec<TraceEvent> = ids
        .iter()
        .filter_map(|id| trace.events.iter().find(|event| &event.id == id).cloned())
        .collect();
    if events.is_empty() {
        return empty_view("No recorded events to display.");
    }
    let lanes = group_events_by_system(&events);
    let pairs = build_causal_event_pairs(&events);
    let selection = build_selection_causality(&events, model.selected_event_id.as_deref());
    let max_ms = events
        .iter()
        .map(|event| event.offset_ms)
        .max()
        .unwrap_or(1)
        .max(1) as f64;

    let lane_h = 56.0;
    let ruler_h = 26.0;
    let label_w = 120.0;
    let pad = 18.0;
    let width = 1100.0;
    let height = ruler_h + pad * 2.0 + lanes.len() as f64 * lane_h;
    let x_for =
        |ms: i64| pad + label_w + (ms as f64 / max_ms).powf(0.5) * (width - pad * 2.0 - label_w);
    let lane_y = |index: usize| ruler_h + pad + index as f64 * lane_h + lane_h / 2.0;
    let lane_index =
        |system: SystemId| lanes.iter().position(|(candidate, _)| *candidate == system);

    html! {
        <div class="min-h-0 flex-1 overflow-auto">
            <div class="flex items-center gap-2 px-4 py-2 text-[10px] uppercase tracking-wide text-slate-500">
                <span>{ "System Swimlane" }</span>
                <span class="text-slate-600">{ "time → · bounded scale (timestamps shown on hover)" }</span>
            </div>
            <svg width="100%" height={height.to_string()} viewBox={format!("0 0 {width} {height}")}
                preserveAspectRatio="xMidYMid meet" class="min-w-[900px] max-w-none" role="group" aria-label="System swimlane">
                { for [0.0f64, 0.25, 0.5, 0.75, 1.0].into_iter().map(|fraction| {
                    let x = pad + label_w + fraction * (width - pad * 2.0 - label_w);
                    html! {
                        <g>
                            <line x1={x.to_string()} y1={(ruler_h - 6.0).to_string()} x2={x.to_string()} y2={ruler_h.to_string()} stroke="rgba(255,255,255,0.25)" />
                            <text x={x.to_string()} y={(ruler_h - 9.0).to_string()} text-anchor="middle" font-size="8" class="fill-slate-500">
                                { format!("{}ms", (fraction * max_ms).round() as i64) }
                            </text>
                        </g>
                    }
                }) }
                { for lanes.iter().enumerate().map(|(index, (system, _))| {
                    let y = lane_y(index);
                    html! {
                        <g>
                            <line x1={(pad + label_w).to_string()} y1={(y - lane_h / 2.0).to_string()} x2={(width - pad).to_string()} y2={(y - lane_h / 2.0).to_string()} stroke="rgba(255,255,255,0.06)" />
                            <text x={(pad + 6.0).to_string()} y={(y + 3.0).to_string()} font-size="10" class="fill-slate-400">{ system.label() }</text>
                        </g>
                    }
                }) }
                { for pairs.iter().filter(|pair| events[pair.from].system != events[pair.to].system).map(|pair| {
                    let from = &events[pair.from];
                    let to = &events[pair.to];
                    let from_y = lane_y(lane_index(from.system).unwrap_or(0));
                    let to_y = lane_y(lane_index(to.system).unwrap_or(0));
                    let active = model.selected_event_id.as_deref() == Some(from.id.as_str())
                        || model.selected_event_id.as_deref() == Some(to.id.as_str())
                        || selection.parents.contains(&from.id)
                        || selection.children.contains(&to.id);
                    html! {
                        <line x1={x_for(from.offset_ms).to_string()} y1={from_y.to_string()}
                            x2={x_for(to.offset_ms).to_string()} y2={to_y.to_string()}
                            stroke={if active { "#c6a15b" } else { "rgba(198,161,91,0.35)" }}
                            stroke-width={if active { "1.6" } else { "1" }} stroke-dasharray="3 2" />
                    }
                }) }
                { for events.iter().filter_map(|event| {
                    let index = lane_index(event.system)?;
                    let x = x_for(event.offset_ms);
                    let y = lane_y(index);
                    let selected = model.selected_event_id.as_deref() == Some(event.id.as_str());
                    let is_parent = selection.parents.contains(&event.id);
                    let is_child = selection.children.contains(&event.id);
                    let dim = model.selected_event_id.is_some() && !selected && !is_parent && !is_child;
                    let id = event.id.clone();
                    let tooltip = format!("{} · {} · {} · {}", event.title, event.system.label(), format_clock(event.occurred_at_ms), event.status.label());
                    Some(html! {
                        <g transform={format!("translate({x},{y})")} opacity={if dim { "0.45" } else { "1" }}
                            style={format!("cursor:pointer;color:{}", event.kind.hex())}
                            onclick={link.callback(move |_: MouseEvent| Msg::EventSelected(id.clone()))}>
                            <title>{ tooltip }</title>
                            { kind_glyph_at(event.kind, -8.0, -8.0, 16.0, 16.0) }
                            <circle r={if selected || is_parent || is_child { "10" } else { "9" }} fill="none"
                                stroke={if selected { "#fff" } else if is_parent || is_child { "#c6a15b" } else { "rgba(255,255,255,0.3)" }}
                                stroke-width={if selected || is_parent || is_child { "1.6" } else { "1" }} />
                        </g>
                    })
                }) }
            </svg>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Raw Events
// ---------------------------------------------------------------------------

fn raw_events(trace: &FlightRecorderTrace, model: &Model, link: &Link<Msg>) -> Html {
    let ids = filtered_ids(model);
    let events: Vec<&TraceEvent> = ids
        .iter()
        .filter_map(|id| trace.events.iter().find(|event| &event.id == id))
        .collect();
    if events.is_empty() {
        return empty_view("No recorded events to display.");
    }
    html! {
        <div class="min-h-0 flex-1 overflow-auto">
            <table class="w-full border-collapse text-left text-xs">
                <thead class="sticky top-0 bg-[#0b1220] text-[10px] uppercase tracking-wide text-slate-500">
                    <tr>
                        <th class="px-3 py-2">{ "Time" }</th>
                        <th class="px-3 py-2">{ "Event type" }</th>
                        <th class="px-3 py-2">{ "Event id" }</th>
                        <th class="px-3 py-2">{ "System" }</th>
                        <th class="px-3 py-2">{ "Status" }</th>
                        <th class="px-3 py-2">{ "Node" }</th>
                        <th class="px-2 py-2"></th>
                    </tr>
                </thead>
                <tbody>
                    { for events.into_iter().map(|event| {
                        let selected = model.selected_event_id.as_deref() == Some(event.id.as_str());
                        let expanded = model.expanded_raw.as_deref() == Some(event.id.as_str());
                        let id = event.id.clone();
                        let toggle_id = event.id.clone();
                        let fields = raw_event_fields(event);
                        let payload = event.payload_json();
                        html! {
                            <>
                                <tr class={classes!("cursor-pointer", "border-b", "border-white/5",
                                    if selected { "bg-slate-800" } else { "hover:bg-slate-900/80" })}
                                    onclick={link.callback(move |_: MouseEvent| Msg::EventSelected(id.clone()))}>
                                    <td class="px-3 py-1.5 font-mono text-[11px] text-slate-300">{ format_clock(event.occurred_at_ms) }</td>
                                    <td class="px-3 py-1.5 font-mono text-[11px] text-slate-200">{ event.event_type.clone() }</td>
                                    <td class="px-3 py-1.5 font-mono text-[11px] text-slate-300">{ event.id.clone() }</td>
                                    <td class="px-3 py-1.5">
                                        <span class={classes!("inline-flex", "rounded-md", "px-2", "py-0.5", "text-[10px]", event.system.chip_class())}>
                                            { event.system.label() }
                                        </span>
                                    </td>
                                    <td class="px-3 py-1.5">{ event.status.label() }</td>
                                    <td class="px-3 py-1.5 font-mono text-[11px] text-slate-400">
                                        { event.workflow_node_id.clone().unwrap_or_else(|| "—".into()) }
                                    </td>
                                    <td class="px-2 py-1.5 text-right">
                                        <button type="button" onclick={link.callback(move |_: MouseEvent| Msg::RawToggled(toggle_id.clone()))}
                                            class="rounded px-2 py-0.5 text-slate-400 hover:text-white">
                                            { if expanded { "−" } else { "+" } }
                                        </button>
                                    </td>
                                </tr>
                                if expanded {
                                    <tr class="border-b border-white/5 bg-[#0a1018]">
                                        <td colspan="7" class="px-4 py-3">
                                            <div class="grid grid-cols-2 gap-x-6 gap-y-1">
                                                { for fields.iter().map(|field| html! {
                                                    <div class="flex items-baseline gap-2 text-[11px]">
                                                        <span class="w-40 shrink-0 text-slate-500">{ field.key.clone() }</span>
                                                        <code class={classes!("break-all", "text-slate-200", if field.mono { "font-mono" } else { "" })}>{ field.value.clone() }</code>
                                                    </div>
                                                }) }
                                            </div>
                                            <div class="mt-3 flex items-center gap-2">
                                                <span class="text-[10px] uppercase tracking-wide text-slate-500">{ "Payload" }</span>
                                                 <button type="button" onclick={link.callback({
                                                         let payload = payload.clone();
                                                         move |_: MouseEvent| { copy_text(&payload); Msg::Noop }
                                                     })}
                                                    class="rounded border border-white/10 px-2 py-0.5 text-[11px] text-slate-300 hover:text-white">
                                                    { "Copy JSON" }
                                                </button>
                                            </div>
                                            <pre class="mt-1 overflow-x-auto rounded-md bg-black/30 p-2 font-mono text-[10px] leading-relaxed text-slate-300">{ payload }</pre>
                                        </td>
                                    </tr>
                                }
                            </>
                        }
                    }) }
                </tbody>
            </table>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Event details (right panel)
// ---------------------------------------------------------------------------

fn selected_event<'a>(trace: &'a FlightRecorderTrace, model: &Model) -> Option<&'a TraceEvent> {
    model
        .selected_event_id
        .as_deref()
        .and_then(|id| trace.events.iter().find(|event| event.id == id))
}

fn event_details(event: Option<&TraceEvent>, link: &Link<Msg>) -> Html {
    let Some(event) = event else {
        return html! {
            <aside class="w-[360px] shrink-0 p-6 text-sm text-slate-500">{ "Select an event in the timeline" }</aside>
        };
    };
    let overview: Vec<(&str, String)> = vec![
        ("Event ID", event.id.clone()),
        ("Time", format_display_time(event.occurred_at_ms)),
        ("Duration", format_duration(event.duration_ms)),
        ("System", event.system.label().to_owned()),
        ("Correlation ID", event.correlation_id.clone()),
        (
            "Causation ID",
            event.causation_id.clone().unwrap_or_else(|| "—".into()),
        ),
        ("Event Type", event.event_type.clone()),
    ];
    let payload = event.payload_json();
    let cause = event.causation_id.clone();
    html! {
        <aside class="flex w-[360px] shrink-0 flex-col overflow-y-auto border-l border-white/5">
            <div class="flex items-start justify-between gap-3 border-b border-white/5 p-4">
                <div class="flex items-start gap-2">
                    <span class={classes!("grid", "h-8", "w-8", "place-items-center", "rounded-md", "text-white", event.kind.token_bg())}>
                        { kind_glyph(event.kind, "h-4 w-4") }
                    </span>
                    <div>
                        <div class="font-medium">{ event.title.clone() }</div>
                        <div class="text-xs text-slate-400">{ format!("{} Event", event.kind.label()) }</div>
                    </div>
                </div>
                <span class={classes!("rounded-full", "px-2", "py-0.5", "text-[11px]",
                    match event.status {
                        EventStatus::Failed => "bg-rose-500/15 text-rose-300",
                        EventStatus::Success => "bg-emerald-500/15 text-emerald-300",
                        EventStatus::Pending => "bg-amber-500/15 text-amber-300",
                        _ => "bg-slate-500/15 text-slate-300",
                    })}>
                    { event.status.label() }
                </span>
            </div>
            <section class="border-b border-white/5 p-4">
                <h2 class="text-[10px] uppercase tracking-wide text-slate-500">{ "Overview" }</h2>
                <dl class="mt-2 space-y-1.5 text-xs">
                    { for overview.into_iter().map(|(key, value)| {
                        let clickable = key == "Causation ID" && cause.is_some();
                        let target = cause.clone();
                        html! {
                            <div class="grid grid-cols-[108px_1fr] gap-2">
                                <dt class="text-slate-500">{ key }</dt>
                                <dd class="break-all font-mono text-[11px] text-slate-200">
                                    if clickable {
                                        <button type="button" class="text-sky-300 hover:underline"
                                            onclick={link.callback(move |_: MouseEvent| Msg::EventSelected(target.clone().unwrap_or_default()))}>
                                            { value }
                                        </button>
                                    } else {
                                        { value }
                                    }
                                </dd>
                            </div>
                        }
                    }) }
                </dl>
            </section>
            <section class="border-b border-white/5 p-4">
                <div class="mb-2 flex items-center justify-between">
                    <h2 class="text-[10px] uppercase tracking-wide text-slate-500">{ "Payload" }</h2>
                    <button type="button" class="text-[11px] text-slate-400 hover:text-white"
                        onclick={link.callback({
                            let payload = payload.clone();
                            move |_: MouseEvent| { copy_text(&payload); Msg::Noop }
                        })}>
                        { "Copy" }
                    </button>
                </div>
                <pre class="overflow-x-auto rounded-lg bg-[#0a1018] p-3 font-mono text-[11px] leading-relaxed text-slate-300">{ payload }</pre>
            </section>
            <section class="border-b border-white/5 p-4">
                <h2 class="text-[10px] uppercase tracking-wide text-slate-500">{ "Tags" }</h2>
                <div class="mt-2 flex flex-wrap gap-1.5">
                    { for event.tags.iter().map(|tag| {
                        let tag = tag.clone();
                        let label = tag.clone();
                        html! {
                            <button type="button" onclick={link.callback(move |_: MouseEvent| Msg::QueryChanged(tag.clone()))}
                                class="rounded-full border border-white/10 px-2 py-0.5 text-[11px] text-slate-300 hover:bg-white/5">
                                { label }
                            </button>
                        }
                    }) }
                </div>
            </section>
            <section class="p-4">
                <h2 class="text-[10px] uppercase tracking-wide text-slate-500">{ "Related Events" }</h2>
                <ul class="mt-2 space-y-1">
                    { for event.related_event_ids.iter().map(|related| {
                        let id = related.id.clone();
                        html! {
                            <li>
                                <button type="button" onclick={link.callback(move |_: MouseEvent| Msg::EventSelected(id.clone()))}
                                    class="flex w-full items-center justify-between rounded px-1 py-1 text-left text-xs text-sky-300 hover:bg-white/5">
                                    <span>{ related.title.clone() }</span>
                                    <span class="font-mono text-[11px] text-slate-500">{ format_offset(related.offset_ms) }</span>
                                </button>
                            </li>
                        }
                    }) }
                </ul>
            </section>
        </aside>
    }
}

fn empty_view(message: &str) -> Html {
    html! {
        <div class="grid flex-1 place-items-center text-sm text-slate-500">{ message.to_owned() }</div>
    }
}

// ---------------------------------------------------------------------------
// Kind glyphs (the owner's per-kind SVGs, ported from KindGlyph.tsx)
// ---------------------------------------------------------------------------

fn kind_path(kind: EventKind) -> &'static str {
    match kind {
        EventKind::Command => "M12 2l2.4 7.2H22l-6 4.4 2.3 7.4L12 16.8 5.7 21l2.3-7.4L2 9.2h7.6z",
        EventKind::DomainEvent => "M12 3a9 9 0 100 18 9 9 0 000-18zm0 4v5l4 2",
        EventKind::Workflow => "M7 7h4v4H7zM13 7h4v4h-4zM7 13h4v4H7zM13 13h4v4h-4z",
        EventKind::Task => "M9 3h6l1 3h3v15H5V6h3l1-3zm0 8h6m-6 4h4",
        EventKind::Integration => "M7 12a5 5 0 015-5h2m3 5a5 5 0 01-5 5h-2M8 12h8",
        EventKind::Persistence => "M4 7a8 3 0 0016 0A8 3 0 004 7zm0 5c0 1.7 3.6 3 8 3s8-1.3 8-3M4 17c0 1.7 3.6 3 8 3s8-1.3 8-3",
        EventKind::Unknown => "M12 3a9 9 0 100 18 9 9 0 000-18zm.5 6v6m0 3h.01",
    }
}

fn kind_glyph(kind: EventKind, class: &str) -> Html {
    html! {
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" class={class.to_owned()}>
            <path d={kind_path(kind)} stroke-linecap="round" stroke-linejoin="round" />
        </svg>
    }
}

fn kind_glyph_at(kind: EventKind, x: f64, y: f64, width: f64, height: f64) -> Html {
    html! {
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75"
            x={x.to_string()} y={y.to_string()} width={width.to_string()} height={height.to_string()}>
            <path d={kind_path(kind)} stroke-linecap="round" stroke-linejoin="round" />
        </svg>
    }
}

// ---------------------------------------------------------------------------
// Browser helpers (Rust calling the browser, never a JavaScript file)
// ---------------------------------------------------------------------------

/// Copy text to the clipboard, best effort. Silent when the browser refuses (no permission, insecure origin).
fn copy_text(text: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.navigator().clipboard().write_text(text);
    }
}

/// Serialize a JSON value and offer it as a file download via a data-URI anchor.
fn download_json(data: &serde_json::Value, filename: &str) {
    use base64::Engine as _;
    use wasm_bindgen::JsCast as _;
    let Ok(text) = serde_json::to_string_pretty(data) else {
        return;
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Ok(element) = document.create_element("a") else {
        return;
    };
    if let Ok(anchor) = element.dyn_into::<web_sys::HtmlAnchorElement>() {
        anchor.set_href(&format!("data:application/json;base64,{encoded}"));
        anchor.set_download(filename);
        anchor.click();
    }
}

/// The pretty payload JSON for display and copy, shared by the panels.
trait PayloadJson {
    fn payload_json(&self) -> String;
}

impl PayloadJson for TraceEvent {
    fn payload_json(&self) -> String {
        serde_json::to_string_pretty(self.payload.as_ref().unwrap_or(&serde_json::Value::Null))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn trace() -> FlightRecorderTrace {
        adapt_flight_recorder_transaction(&serde_json::from_value(json!({
            "transaction": {"correlationId": "c"},
            "workflows": [],
            "events": [
                {"eventId":"e1","occurredAt":"2026-09-22T12:00:00.000Z","eventType":"RUN_START","sourceSystem":"forge_observer","workflowNodeId":"a"},
                {"eventId":"e2","occurredAt":"2026-09-22T12:00:01.000Z","eventType":"RUN_END","sourceSystem":"forge_observer","outcome":"FAILURE","workflowNodeId":"a"}
            ]
        }))
        .unwrap())
    }

    fn ctx() -> ScreenCtx {
        ScreenCtx {
            id: Some("11111111-1111-1111-1111-111111111111".into()),
            ..ScreenCtx::default()
        }
    }

    #[test]
    fn it_reads_its_instance_refreshes_on_a_timer_and_keeps_the_last_good_trace() {
        let ctx = ctx();
        let (mut model, cmd) = FlightRecorder::init(&ctx);
        let request = cmd.into_requests().remove(0);
        assert_eq!(
            request.path,
            "/api/portal/flight-recorder/11111111-1111-1111-1111-111111111111"
        );
        FlightRecorder::update(
            &mut model,
            request.respond(Ok(json!({
                "transaction": {"correlationId": "c"},
                "workflows": [],
                "events": []
            }))),
            &ctx,
        );
        assert!(model.read.loaded().is_some());
        // The tick re-reads, but only one read is in flight at a time.
        assert!(matches!(
            FlightRecorder::update(&mut model, Msg::Tick, &ctx),
            Cmd::Batch(_)
        ));
        FlightRecorder::update(
            &mut model,
            Msg::Loaded(Err(ApiError::network("down"))),
            &ctx,
        );
        assert!(model.read.loaded().is_some(), "the last good trace stays");
        assert_eq!(model.notice.as_deref(), Some("down"));
    }

    #[test]
    fn no_instance_is_said_not_read() {
        let (model, cmd) = FlightRecorder::init(&ScreenCtx::default());
        assert!(cmd.into_requests().is_empty());
        assert!(model.notice.is_some());
    }

    #[test]
    fn selection_links_an_event_to_its_workflow_node_and_escape_clears_it() {
        let ctx = ctx();
        let mut model = Model {
            read: Remote::Loaded(trace()),
            ..Model::default()
        };
        FlightRecorder::update(&mut model, Msg::EventSelected("e1".into()), &ctx);
        assert_eq!(model.selected_event_id.as_deref(), Some("e1"));
        assert_eq!(model.selected_node_id.as_deref(), Some("a"));
        FlightRecorder::update(&mut model, Msg::KeyNav(KeyNav::Escape), &ctx);
        assert!(model.selected_event_id.is_none());
        assert!(model.selected_node_id.is_none());
    }

    #[test]
    fn keyboard_moves_through_the_filtered_events_only() {
        let ctx = ctx();
        let mut model = Model {
            read: Remote::Loaded(trace()),
            ..Model::default()
        };
        // RUN_START / RUN_END classify as Unknown, so filter to that kind.
        model.kind_filters = vec![EventKind::Unknown];
        FlightRecorder::update(&mut model, Msg::KeyNav(KeyNav::Down), &ctx);
        assert_eq!(model.selected_event_id.as_deref(), Some("e1"));
        FlightRecorder::update(&mut model, Msg::KeyNav(KeyNav::Down), &ctx);
        assert_eq!(model.selected_event_id.as_deref(), Some("e2"));
    }

    #[test]
    fn a_tag_click_becomes_the_search_query_and_filter_counts_the_view() {
        let ctx = ctx();
        let mut model = Model {
            read: Remote::Loaded(trace()),
            ..Model::default()
        };
        assert_eq!(filtered_ids(&model).len(), 2);
        FlightRecorder::update(&mut model, Msg::QueryChanged("run".into()), &ctx);
        assert_eq!(filtered_ids(&model).len(), 2, "both events read Run");
        FlightRecorder::update(&mut model, Msg::QueryChanged("nothing".into()), &ctx);
        assert!(filtered_ids(&model).is_empty());
        FlightRecorder::update(&mut model, Msg::FiltersCleared, &ctx);
        assert_eq!(filtered_ids(&model).len(), 2);
    }
}
