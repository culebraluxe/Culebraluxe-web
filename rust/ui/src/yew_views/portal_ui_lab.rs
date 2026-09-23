//! /portal/design-lab — one comparison surface for the portal UI vocabulary.
//!
//! This screen is deliberately local: no read model, no credentials, no server effect. It is the safe place to compare
//! native Yew controls against the preserved React/TypeScript gallery and the Framer interaction experiments before a
//! pattern is promoted into product screens.

use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;
use yew::TargetCast;

use crate::model::Msg;
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct UiLabProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LabTab {
    Yew,
    React,
    Motion,
}

impl LabTab {
    fn label(self) -> &'static str {
        match self {
            Self::Yew => "Yew / Rust",
            Self::React => "React / TypeScript",
            Self::Motion => "Motion / Framer",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LabModel {
    tab: LabTab,
    query: String,
    density: &'static str,
    notifications: bool,
    role: String,
    dialog_open: bool,
    command_open: bool,
    page: usize,
    selected_client: Option<String>,
}

impl Default for LabModel {
    fn default() -> Self {
        Self {
            tab: LabTab::Yew,
            query: String::new(),
            density: "comfortable",
            notifications: true,
            role: "Broker".into(),
            dialog_open: false,
            command_open: false,
            page: 0,
            selected_client: None,
        }
    }
}

pub enum LabMsg {
    TabSelected(LabTab),
    QueryChanged(String),
    DensityChanged(&'static str),
    NotificationsChanged(bool),
    RoleChanged(String),
    DialogChanged(bool),
    CommandChanged(bool),
    PageChanged(usize),
    ClientSelected(Option<String>),
    Reset,
}

fn reduce(model: &mut LabModel, msg: LabMsg) {
    match msg {
        LabMsg::TabSelected(tab) => model.tab = tab,
        LabMsg::QueryChanged(query) => model.query = query,
        LabMsg::DensityChanged(density) => model.density = density,
        LabMsg::NotificationsChanged(enabled) => model.notifications = enabled,
        LabMsg::RoleChanged(role) => model.role = role,
        LabMsg::DialogChanged(open) => model.dialog_open = open,
        LabMsg::CommandChanged(open) => model.command_open = open,
        LabMsg::PageChanged(page) => model.page = page.min(1),
        LabMsg::ClientSelected(client) => model.selected_client = client,
        LabMsg::Reset => *model = LabModel::default(),
    }
}

pub struct UiLab {
    lab: LabModel,
}

impl Component for UiLab {
    type Message = LabMsg;
    type Properties = UiLabProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            lab: LabModel::default(),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        reduce(&mut self.lab, msg);
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("design-lab").expect("design-lab screen exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                <div class="space-y-4">
                    { self.hero(ctx) }
                    { self.tabs(ctx) }
                    {
                        match self.lab.tab {
                            LabTab::Yew => self.yew_lab(ctx),
                            LabTab::React => self.react_lab(),
                            LabTab::Motion => self.motion_lab(),
                        }
                    }
                </div>
            </PortalShell>
        }
    }
}

impl UiLab {
    fn hero(&self, ctx: &Context<Self>) -> Html {
        let reset = ctx.link().callback(|_: MouseEvent| LabMsg::Reset);
        html! {
            <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="flex flex-col gap-4 p-5 lg:flex-row lg:items-end lg:justify-between">
                    <div>
                        <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold)]">
                            {"TECH · Component proving ground"}
                        </p>
                        <h1 class="mt-1 font-serif text-3xl font-light text-[var(--portal-navy)]">
                            {"UI Lab"}
                        </h1>
                        <p class="mt-2 max-w-3xl text-sm font-light leading-6 text-black/55">
                            {"One surface to compare the SaaS vocabulary before promoting it into production. Yew owns the screen; React and motion experiments stay bounded until they earn adoption."}
                        </p>
                    </div>
                    <button
                        type="button"
                        onclick={reset}
                        class="inline-flex min-h-9 items-center justify-center rounded-md border border-[var(--portal-border)] bg-white/40 px-3 text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-navy)] transition hover:border-[var(--portal-gold)] hover:bg-white/70"
                    >
                        {"Reset demos"}
                    </button>
                </div>
            </section>
        }
    }

    fn tabs(&self, ctx: &Context<Self>) -> Html {
        html! {
            <nav class="portal-glass-panel flex flex-wrap gap-1 rounded-[var(--portal-panel-radius)] p-1.5" aria-label="UI lab technology">
                { self.tab_button(ctx, LabTab::Yew) }
                { self.tab_button(ctx, LabTab::React) }
                { self.tab_button(ctx, LabTab::Motion) }
            </nav>
        }
    }

    fn tab_button(&self, ctx: &Context<Self>, tab: LabTab) -> Html {
        let active = self.lab.tab == tab;
        let onclick = ctx.link().callback(move |_: MouseEvent| LabMsg::TabSelected(tab));
        html! {
            <button
                type="button"
                {onclick}
                aria-pressed={active.to_string()}
                class={classes!(
                    "rounded-md", "px-4", "py-2", "text-xs", "font-medium", "transition",
                    if active {
                        "bg-[var(--portal-navy)] text-white shadow-sm"
                    } else {
                        "text-[var(--portal-navy-soft)] hover:bg-white/50 hover:text-[var(--portal-navy)]"
                    }
                )}
            >
                { tab.label() }
            </button>
        }
    }
}

impl UiLab {
    fn yew_lab(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div class="space-y-4">
                { self.metrics() }
                <div class="grid gap-4 xl:grid-cols-[1.05fr_1.45fr]">
                    { self.controls(ctx) }
                    { self.data_grid_preview() }
                </div>
                <div class="grid gap-4 xl:grid-cols-2">
                    { self.feedback_states() }
                    { self.dashboard_preview() }
                </div>
                { self.analytics_preview() }
                <div class="grid gap-4 xl:grid-cols-2">
                    { self.workflow_timeline() }
                    { self.kanban_preview() }
                </div>
                { self.interaction_patterns(ctx) }
                { self.stack_matrix() }
                { self.overlays(ctx) }
            </div>
        }
    }

    fn metrics(&self) -> Html {
        let metrics = [
            ("Foundation", "Yew 0.23", "Native MVI"),
            ("Controls", "PatternFly", "Mature SaaS"),
            ("Data grid", "rs-grid", "Virtualized"),
            ("Charts", "Charming", "ECharts"),
        ];
        html! {
            <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="grid grid-cols-2 lg:grid-cols-4">
                    { for metrics.into_iter().map(|(label, value, hint)| html! {
                        <div class="border-b border-r border-[var(--portal-border)] px-4 py-3 last:border-r-0 lg:border-b-0">
                            <p class="text-[9px] font-medium uppercase tracking-[0.15em] text-[var(--portal-blue-gray)]">{ label }</p>
                            <p class="mt-1 font-serif text-xl font-light text-[var(--portal-navy)]">{ value }</p>
                            <p class="mt-1 text-[10px] font-light text-black/40">{ hint }</p>
                        </div>
                    }) }
                </div>
            </section>
        }
    }

    fn controls(&self, ctx: &Context<Self>) -> Html {
        let on_query = ctx.link().callback(|event: InputEvent| {
            let input: HtmlInputElement = event.target_unchecked_into();
            LabMsg::QueryChanged(input.value())
        });
        let on_role = ctx.link().callback(|event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            LabMsg::RoleChanged(select.value())
        });
        let on_notifications = ctx.link().callback(|event: Event| {
            let input: HtmlInputElement = event.target_unchecked_into();
            LabMsg::NotificationsChanged(input.checked())
        });

        html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
                { section_heading("Forms & actions", "Native Yew controls with reducer-owned demo state.") }

                <div class="mt-4 space-y-4">
                    <label class="block">
                        <span class="text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                            {"Search clients"}
                        </span>
                        <input
                            type="search"
                            value={self.lab.query.clone()}
                            oninput={on_query}
                            placeholder="Try: Marina"
                            class="mt-1.5 w-full rounded-md border border-[var(--portal-border)] bg-white/55 px-3 py-2.5 text-sm text-[var(--portal-navy)] outline-none transition placeholder:text-black/30 focus:border-[var(--portal-gold)] focus:bg-white"
                        />
                    </label>

                    <label class="block">
                        <span class="text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                            {"Portal role"}
                        </span>
                        <select
                            value={self.lab.role.clone()}
                            onchange={on_role}
                            class="mt-1.5 w-full rounded-md border border-[var(--portal-border)] bg-white/55 px-3 py-2.5 text-sm text-[var(--portal-navy)] outline-none focus:border-[var(--portal-gold)]"
                        >
                            <option value="Broker">{"Broker"}</option>
                            <option value="Agent">{"Agent"}</option>
                            <option value="Transaction Coordinator">{"Transaction Coordinator"}</option>
                        </select>
                    </label>

                    <div>
                        <span class="text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                            {"Density"}
                        </span>
                        <div class="mt-1.5 inline-flex rounded-md border border-[var(--portal-border)] bg-white/35 p-1">
                            { self.density_button(ctx, "compact", "Compact") }
                            { self.density_button(ctx, "comfortable", "Comfortable") }
                            { self.density_button(ctx, "relaxed", "Relaxed") }
                        </div>
                    </div>

                    <label class="flex items-center justify-between gap-4 rounded-md border border-[var(--portal-border)] bg-white/35 px-3 py-2.5">
                        <div>
                            <p class="text-sm font-medium text-[var(--portal-navy)]">{"Notifications"}</p>
                            <p class="text-[11px] font-light text-black/45">{"Example boolean setting."}</p>
                        </div>
                        <input
                            type="checkbox"
                            checked={self.lab.notifications}
                            onchange={on_notifications}
                            class="h-4 w-4 accent-[var(--portal-gold)]"
                        />
                    </label>

                    <div class="flex flex-wrap gap-2 pt-1">
                        <button type="button" class="rounded-md bg-[var(--portal-navy)] px-3 py-2 text-xs font-medium text-white transition hover:bg-[var(--portal-navy-soft)]">
                            {"Primary action"}
                        </button>
                        <button type="button" class="rounded-md border border-[var(--portal-gold)] px-3 py-2 text-xs font-medium text-[var(--portal-navy)] transition hover:bg-[var(--portal-gold)]/10">
                            {"Secondary"}
                        </button>
                        <button type="button" class="rounded-md px-3 py-2 text-xs font-medium text-[var(--portal-archive)] transition hover:bg-red-50">
                            {"Destructive"}
                        </button>
                    </div>
                </div>
            </section>
        }
    }

    fn density_button(&self, ctx: &Context<Self>, density: &'static str, label: &'static str) -> Html {
        let active = self.lab.density == density;
        let onclick = ctx.link().callback(move |_: MouseEvent| LabMsg::DensityChanged(density));
        html! {
            <button
                type="button"
                {onclick}
                class={classes!(
                    "rounded", "px-2.5", "py-1.5", "text-[10px]", "font-medium", "transition",
                    if active { "bg-white text-[var(--portal-navy)] shadow-sm" } else { "text-black/45 hover:text-[var(--portal-navy)]" }
                )}
            >
                { label }
            </button>
        }
    }

    fn data_grid_preview(&self) -> Html {
        let rows = [
            ("CL-1042", "Marina Soto", "Seller", "Active", "$1.85M"),
            ("CL-1038", "Alicia Rivera", "Buyer", "Follow-up", "$925K"),
            ("CL-1031", "David Chen", "Seller", "Active", "$2.40M"),
            ("CL-1024", "Elena Cruz", "Buyer", "New", "$1.20M"),
            ("CL-1017", "Tomás Vélez", "Investor", "Active", "$3.10M"),
        ];
        let query = self.lab.query.trim().to_lowercase();
        let pad = match self.lab.density {
            "compact" => "py-1.5",
            "relaxed" => "py-3.5",
            _ => "py-2.5",
        };

        html! {
            <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="flex items-center justify-between gap-3 border-b border-[var(--portal-border)] px-4 py-3">
                    <div>
                        <h2 class="font-serif text-lg font-light text-[var(--portal-panel-heading)]">{"Data grid pattern"}</h2>
                        <p class="text-[10px] font-light text-black/40">{"Native preview now; rs-grid is the serious large-row candidate."}</p>
                    </div>
                    <span class="rounded-full border border-[var(--portal-gold)]/50 px-2 py-1 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)]">
                        {"5 rows"}
                    </span>
                </div>
                <div class="overflow-x-auto">
                    <table class="w-full min-w-[620px] text-left">
                        <thead>
                            <tr class="border-b border-[var(--portal-border)] bg-white/25 text-[9px] font-medium uppercase tracking-[0.13em] text-[var(--portal-blue-gray)]">
                                <th class="px-4 py-2">{"ID"}</th>
                                <th class="px-4 py-2">{"Client"}</th>
                                <th class="px-4 py-2">{"Type"}</th>
                                <th class="px-4 py-2">{"Status"}</th>
                                <th class="px-4 py-2 text-right">{"Value"}</th>
                            </tr>
                        </thead>
                        <tbody>
                            { for rows.into_iter()
                                .filter(|row| query.is_empty() || row.1.to_lowercase().contains(&query))
                                .map(|row| html! {
                                    <tr class="border-b border-[var(--portal-border)] last:border-b-0 hover:bg-white/35">
                                        <td class={classes!("px-4", pad, "text-[11px]", "font-mono", "text-black/40")}>{ row.0 }</td>
                                        <td class={classes!("px-4", pad, "text-sm", "font-medium", "text-[var(--portal-navy)]")}>{ row.1 }</td>
                                        <td class={classes!("px-4", pad, "text-xs", "text-black/55")}>{ row.2 }</td>
                                        <td class={classes!("px-4", pad)}>{ status_badge(row.3) }</td>
                                        <td class={classes!("px-4", pad, "text-right", "font-serif", "text-sm", "text-[var(--portal-navy)]")}>{ row.4 }</td>
                                    </tr>
                                }) }
                        </tbody>
                    </table>
                </div>
            </section>
        }
    }

    fn feedback_states(&self) -> Html {
        html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
                { section_heading("States & feedback", "The boring states are part of the product vocabulary too.") }
                <div class="mt-4 space-y-3">
                    <div class="rounded-md border border-emerald-300/60 bg-emerald-50/70 px-3 py-2.5">
                        <p class="text-xs font-medium text-emerald-900">{"Saved"}</p>
                        <p class="text-[11px] text-emerald-800/70">{"The canonical record is current."}</p>
                    </div>
                    <div class="rounded-md border border-amber-300/70 bg-amber-50/70 px-3 py-2.5">
                        <p class="text-xs font-medium text-amber-950">{"Needs attention"}</p>
                        <p class="text-[11px] text-amber-900/70">{"One required relationship is missing."}</p>
                    </div>
                    <div class="rounded-md border border-[var(--portal-border)] bg-white/30 px-4 py-5 text-center">
                        <p class="font-serif text-base font-light text-[var(--portal-navy)]">{"Nothing here yet"}</p>
                        <p class="mt-1 text-[11px] font-light text-black/45">{"A useful empty state explains what creates the first record."}</p>
                    </div>
                </div>
            </section>
        }
    }

    fn dashboard_preview(&self) -> Html {
        html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
                { section_heading("Dashboard tile pattern", "Hadrone is the candidate only when drag/resize is genuinely useful.") }
                <div class="mt-4 grid grid-cols-2 gap-3">
                    { metric_tile("Active listings", "7", "+2 this month") }
                    { metric_tile("Pipeline", "$4.8M", "11 opportunities") }
                    { metric_tile("Tasks today", "12", "3 need attention") }
                    { metric_tile("Close rate", "68%", "Trailing 90 days") }
                </div>
            </section>
        }
    }


    fn analytics_preview(&self) -> Html {
        let points = [
            ("Apr", 42_i32, 31_i32),
            ("May", 58, 37),
            ("Jun", 51, 34),
            ("Jul", 67, 41),
            ("Aug", 73, 48),
            ("Sep", 82, 54),
        ];
        html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
                { section_heading("Analytics pattern", "Native CSS proof now; Charming replaces the renderer when the crate lands on main.") }
                <div class="mt-4 grid gap-4 lg:grid-cols-[1.5fr_0.7fr]">
                    <div class="rounded-md border border-[var(--portal-border)] bg-white/30 p-4">
                        <div class="flex h-44 items-end gap-3">
                            { for points.into_iter().map(|(month, pipeline, closed)| html! {
                                <div class="flex min-w-0 flex-1 flex-col items-center gap-2">
                                    <div class="flex h-32 w-full items-end justify-center gap-1.5">
                                        <div
                                            class="w-[38%] rounded-t bg-[var(--portal-navy)]"
                                            style={format!("height:{}%;", pipeline)}
                                            title={format!("{month} pipeline {pipeline}")}
                                        />
                                        <div
                                            class="w-[38%] rounded-t bg-[var(--portal-gold)]"
                                            style={format!("height:{}%;", closed)}
                                            title={format!("{month} closed {closed}")}
                                        />
                                    </div>
                                    <span class="text-[9px] font-medium uppercase tracking-[0.08em] text-black/40">{ month }</span>
                                </div>
                            }) }
                        </div>
                        <div class="mt-3 flex gap-4 border-t border-[var(--portal-border)] pt-3 text-[10px] text-black/45">
                            <span class="flex items-center gap-1.5"><i class="h-2 w-2 rounded-sm bg-[var(--portal-navy)]"></i>{"Pipeline"}</span>
                            <span class="flex items-center gap-1.5"><i class="h-2 w-2 rounded-sm bg-[var(--portal-gold)]"></i>{"Closed"}</span>
                        </div>
                    </div>
                    <div class="grid grid-cols-2 gap-3 lg:grid-cols-1">
                        { metric_tile("Pipeline velocity", "18 days", "−3 days vs prior period") }
                        { metric_tile("Avg. deal value", "$1.42M", "+8.4% trailing 90 days") }
                        { metric_tile("Forecast", "$6.2M", "Weighted open pipeline") }
                    </div>
                </div>
            </section>
        }
    }

    fn workflow_timeline(&self) -> Html {
        let steps = [
            ("Listing agreement", "Complete", "Sep 16", true),
            ("Photography", "Complete", "Sep 18", true),
            ("MLS preparation", "In progress", "Sep 22", false),
            ("Launch campaign", "Queued", "Sep 24", false),
            ("Broker open", "Planned", "Sep 26", false),
        ];
        html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
                { section_heading("Timeline / process", "A compact SaaS process vocabulary for deals, projects and workflows.") }
                <div class="mt-4 space-y-0">
                    { for steps.into_iter().enumerate().map(|(index, (title, state, when, complete))| html! {
                        <div class="grid grid-cols-[1.4rem_1fr_auto] gap-3">
                            <div class="flex flex-col items-center">
                                <span class={classes!(
                                    "mt-1", "h-3", "w-3", "rounded-full", "border-2",
                                    if complete {
                                        "border-[var(--portal-gold)] bg-[var(--portal-gold)]"
                                    } else if state == "In progress" {
                                        "border-[var(--portal-navy)] bg-white"
                                    } else {
                                        "border-black/20 bg-white/60"
                                    }
                                )}></span>
                                if index < 4 {
                                    <span class="min-h-8 w-px flex-1 bg-[var(--portal-border)]"></span>
                                }
                            </div>
                            <div class="pb-4">
                                <p class="text-sm font-medium text-[var(--portal-navy)]">{ title }</p>
                                <p class="text-[10px] font-light text-black/45">{ state }</p>
                            </div>
                            <span class="pt-0.5 text-[10px] font-light text-black/40">{ when }</span>
                        </div>
                    }) }
                </div>
            </section>
        }
    }

    fn kanban_preview(&self) -> Html {
        let columns = [
            ("Active", vec![("Prepare MLS package", "High"), ("Review seller copy", "Normal")]),
            ("Backlog", vec![("Broker open invite", "Normal"), ("Portal launch report", "Low")]),
            ("Closed", vec![("Photography booked", "Done"), ("Listing signed", "Done")]),
        ];
        html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
                { section_heading("Kanban pattern", "Native static proof; production TECH Cockpit keeps SVAR as a bounded island.") }
                <div class="mt-4 grid gap-3 md:grid-cols-3">
                    { for columns.into_iter().map(|(column, cards)| html! {
                        <div class="rounded-md border border-[var(--portal-border)] bg-white/25 p-2.5">
                            <div class="mb-2 flex items-center justify-between">
                                <h3 class="text-[10px] font-semibold uppercase tracking-[0.13em] text-[var(--portal-navy)]">{ column }</h3>
                                <span class="rounded-full bg-white/60 px-1.5 py-0.5 text-[9px] tabular-nums text-black/45">{ cards.len() }</span>
                            </div>
                            <div class="space-y-2">
                                { for cards.into_iter().map(|(title, priority)| html! {
                                    <article class="rounded-md border border-[var(--portal-border)] bg-white/70 p-2.5 shadow-sm">
                                        <p class="text-xs font-medium text-[var(--portal-navy)]">{ title }</p>
                                        <p class="mt-2 text-[9px] font-medium uppercase tracking-[0.1em] text-black/35">{ priority }</p>
                                    </article>
                                }) }
                            </div>
                        </div>
                    }) }
                </div>
            </section>
        }
    }

    fn interaction_patterns(&self, ctx: &Context<Self>) -> Html {
        let open_dialog = ctx.link().callback(|_: MouseEvent| LabMsg::DialogChanged(true));
        let open_command = ctx.link().callback(|_: MouseEvent| LabMsg::CommandChanged(true));
        let previous = ctx.link().callback(|_: MouseEvent| LabMsg::PageChanged(0));
        let next = ctx.link().callback(|_: MouseEvent| LabMsg::PageChanged(1));
        let rows = if self.lab.page == 0 {
            vec![
                ("Marina Soto", "Seller", "Culebra", "Active"),
                ("Alicia Rivera", "Buyer", "Vieques", "Follow-up"),
                ("David Chen", "Seller", "San Juan", "Active"),
            ]
        } else {
            vec![
                ("Elena Cruz", "Buyer", "Dorado", "New"),
                ("Tomás Vélez", "Investor", "Culebra", "Active"),
                ("Sofía Morales", "Seller", "Condado", "Review"),
            ]
        };

        html! {
            <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--portal-border)] px-4 py-3">
                    { section_heading("Overlay, command & pagination", "Patterns that usually expose whether a component kit is production-ready.") }
                    <div class="flex flex-wrap gap-2">
                        <button type="button" onclick={open_command} class="rounded-md border border-[var(--portal-border)] bg-white/50 px-3 py-2 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)] hover:border-[var(--portal-gold)]">
                            {"Command palette"}
                        </button>
                        <button type="button" onclick={open_dialog} class="rounded-md bg-[var(--portal-navy)] px-3 py-2 text-[10px] font-medium uppercase tracking-[0.12em] text-white hover:bg-[var(--portal-navy-soft)]">
                            {"Open dialog"}
                        </button>
                    </div>
                </div>

                <div class="grid lg:grid-cols-[1.4fr_0.8fr]">
                    <div class="overflow-x-auto border-b border-[var(--portal-border)] lg:border-b-0 lg:border-r">
                        <table class="w-full min-w-[540px] text-left">
                            <thead>
                                <tr class="bg-white/25 text-[9px] font-medium uppercase tracking-[0.12em] text-black/40">
                                    <th class="px-4 py-2">{"Client"}</th>
                                    <th class="px-4 py-2">{"Type"}</th>
                                    <th class="px-4 py-2">{"Market"}</th>
                                    <th class="px-4 py-2">{"Status"}</th>
                                    <th class="px-4 py-2 text-right">{"Action"}</th>
                                </tr>
                            </thead>
                            <tbody>
                                { for rows.into_iter().map(|(name, kind, market, status)| {
                                    let selected = self.lab.selected_client.as_deref() == Some(name);
                                    let name_for_click = name.to_string();
                                    let onclick = ctx.link().callback(move |_: MouseEvent| LabMsg::ClientSelected(Some(name_for_click.clone())));
                                    html! {
                                        <tr class={classes!("border-t", "border-[var(--portal-border)]", if selected { "bg-[var(--portal-blue-pale)]/50" } else { "hover:bg-white/35" })}>
                                            <td class="px-4 py-2.5 text-sm font-medium text-[var(--portal-navy)]">{ name }</td>
                                            <td class="px-4 py-2.5 text-xs text-black/55">{ kind }</td>
                                            <td class="px-4 py-2.5 text-xs text-black/55">{ market }</td>
                                            <td class="px-4 py-2.5">{ status_badge(status) }</td>
                                            <td class="px-4 py-2.5 text-right">
                                                <button type="button" {onclick} class="text-[10px] font-medium uppercase tracking-[0.1em] text-[var(--portal-navy-soft)] hover:text-[var(--portal-navy)]">
                                                    {"Inspect"}
                                                </button>
                                            </td>
                                        </tr>
                                    }
                                }) }
                            </tbody>
                        </table>
                        <div class="flex items-center justify-between border-t border-[var(--portal-border)] px-4 py-3">
                            <span class="text-[10px] font-light text-black/40">{ format!("Page {} of 2", self.lab.page + 1) }</span>
                            <div class="flex gap-1.5">
                                <button type="button" onclick={previous} disabled={self.lab.page == 0} class="rounded border border-[var(--portal-border)] px-2.5 py-1.5 text-[10px] disabled:opacity-30">{"Previous"}</button>
                                <button type="button" onclick={next} disabled={self.lab.page == 1} class="rounded border border-[var(--portal-border)] px-2.5 py-1.5 text-[10px] disabled:opacity-30">{"Next"}</button>
                            </div>
                        </div>
                    </div>

                    <aside class="p-4">
                        <p class="text-[9px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)]">{"Inspector"}</p>
                        if let Some(client) = self.lab.selected_client.as_deref() {
                            <h3 class="mt-1 font-serif text-xl font-light text-[var(--portal-navy)]">{ client }</h3>
                            <dl class="mt-4 space-y-3 text-xs">
                                <div><dt class="text-[9px] uppercase tracking-[0.12em] text-black/35">{"Relationship"}</dt><dd class="mt-0.5 text-black/65">{"Two-way · 14 months"}</dd></div>
                                <div><dt class="text-[9px] uppercase tracking-[0.12em] text-black/35">{"Last touch"}</dt><dd class="mt-0.5 text-black/65">{"Yesterday · iMessage"}</dd></div>
                                <div><dt class="text-[9px] uppercase tracking-[0.12em] text-black/35">{"Next action"}</dt><dd class="mt-0.5 text-black/65">{"Review listing strategy"}</dd></div>
                            </dl>
                        } else {
                            <p class="mt-3 text-sm font-light leading-6 text-black/45">{"Select a row to prove the master/detail pattern without navigating away."}</p>
                        }
                    </aside>
                </div>
            </section>
        }
    }

    fn overlays(&self, ctx: &Context<Self>) -> Html {
        let close_dialog = ctx.link().callback(|_: MouseEvent| LabMsg::DialogChanged(false));
        let close_command = ctx.link().callback(|_: MouseEvent| LabMsg::CommandChanged(false));

        html! {
            <>
                if self.lab.dialog_open {
                    <div class="fixed inset-0 z-[90] flex items-center justify-center bg-black/35 p-4">
                        <div class="w-full max-w-md rounded-xl border border-white/40 bg-white p-5 shadow-2xl">
                            <p class="text-[9px] font-medium uppercase tracking-[0.15em] text-[var(--portal-gold)]">{"Dialog pattern"}</p>
                            <h2 class="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">{"Archive this draft?"}</h2>
                            <p class="mt-2 text-sm font-light leading-6 text-black/55">{"A production component needs focus management, escape handling and accessible labeling. This native shell is the visual target for the library comparison."}</p>
                            <div class="mt-5 flex justify-end gap-2">
                                <button type="button" onclick={close_dialog.clone()} class="rounded-md border border-[var(--portal-border)] px-3 py-2 text-xs text-[var(--portal-navy)]">{"Cancel"}</button>
                                <button type="button" onclick={close_dialog} class="rounded-md bg-[var(--portal-navy)] px-3 py-2 text-xs font-medium text-white">{"Archive"}</button>
                            </div>
                        </div>
                    </div>
                }
                if self.lab.command_open {
                    <div class="fixed inset-0 z-[95] flex items-start justify-center bg-black/30 p-4 pt-[12vh]">
                        <div class="w-full max-w-xl overflow-hidden rounded-xl border border-white/40 bg-white shadow-2xl">
                            <div class="flex items-center gap-3 border-b border-[var(--portal-border)] px-4 py-3">
                                <span class="text-black/35">{"⌘"}</span>
                                <input autofocus=true placeholder="Type a command…" class="min-w-0 flex-1 bg-transparent text-sm text-[var(--portal-navy)] outline-none" />
                                <button type="button" onclick={close_command} class="rounded border border-[var(--portal-border)] px-2 py-1 text-[9px] text-black/45">{"ESC"}</button>
                            </div>
                            <div class="p-2">
                                { command_row("Open Clients", "⌘ 1") }
                                { command_row("Create Listing Project", "⌘ N") }
                                { command_row("Jump to Story Board", "G S") }
                                { command_row("Open Cabinet", "G C") }
                            </div>
                        </div>
                    </div>
                }
            </>
        }
    }

    fn stack_matrix(&self) -> Html {
        let stack = [
            ("Culebra Native", "Foundation", "Shell, tokens, MVI and product-specific controls", "Keep"),
            ("PatternFly Yew", "Controls", "Forms, drawers, toolbars, pagination and mature SaaS patterns", "Compare"),
            ("rs-grid-yew", "Grid", "Virtualized sorting/filter/edit/clipboard for serious datasets", "Use for hard grids"),
            ("Charming", "Charts", "Rust API over ECharts for production chart vocabulary", "Chart candidate"),
            ("yew-hooks", "Utility", "Debounce, browser behavior and reusable interaction hooks", "Utility"),
            ("yew_icons", "Icons", "Shared Lucide vocabulary with the React portal", "Utility"),
            ("Hadrone", "Dashboard", "Drag/resize dashboard layout experiments", "Lab first"),
            ("TailYew", "Controls", "Lighter visual alternative to PatternFly", "Compare"),
        ];

        html! {
            <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="border-b border-[var(--portal-border)] px-4 py-3">
                    { section_heading("Rust SaaS stack", "The lab compares capabilities; adoption happens pattern by pattern, not crate by crate.") }
                </div>
                <div class="grid md:grid-cols-2 xl:grid-cols-4">
                    { for stack.into_iter().map(|(name, role, purpose, call)| html! {
                        <article class="border-b border-r border-[var(--portal-border)] p-4">
                            <div class="flex items-start justify-between gap-2">
                                <div>
                                    <p class="text-[9px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)]">{ role }</p>
                                    <h3 class="mt-1 font-serif text-lg font-light text-[var(--portal-navy)]">{ name }</h3>
                                </div>
                                <span class="rounded-full bg-[var(--portal-blue-pale)] px-2 py-1 text-[8px] font-medium uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">
                                    { call }
                                </span>
                            </div>
                            <p class="mt-2 text-[11px] font-light leading-5 text-black/50">{ purpose }</p>
                        </article>
                    }) }
                </div>
            </section>
        }
    }
}

impl UiLab {
    fn react_lab(&self) -> Html {
        html! {
            <div class="space-y-4">
                <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5">
                    <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold)]">{"Preserved reference"}</p>
                    <h2 class="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">{"React / TypeScript component gallery"}</h2>
                    <p class="mt-3 max-w-3xl text-sm font-light leading-6 text-black/55">
                        {"The original portal component gallery is mounted below unchanged as a bounded React island. Yew still owns this route and tab state."}
                    </p>
                </section>
                <div
                    id="ui-lab-react-island"
                    class="min-h-[20rem]"
                    aria-label="React and TypeScript UI component gallery"
                />
            </div>
        }
    }

    fn motion_lab(&self) -> Html {
        html! {
            <div class="space-y-4">
                <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5">
                    <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold)]">{"Interaction reference"}</p>
                    <h2 class="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">{"Motion / Framer ideas"}</h2>
                    <p class="mt-3 max-w-3xl text-sm font-light leading-6 text-black/55">
                        {"The existing Framer-inspired MVI lab is mounted below as a bounded React island: motion profiles, view transitions, image drift, hotspots, tilt, command palette and reusable core components."}
                    </p>
                </section>
                <div
                    id="ui-lab-motion-island"
                    class="min-h-[28rem] overflow-hidden rounded-[var(--portal-panel-radius)]"
                    aria-label="Motion and Framer interaction gallery"
                />
            </div>
        }
    }
}

fn section_heading(title: &'static str, subtitle: &'static str) -> Html {
    html! {
        <div>
            <h2 class="font-serif text-lg font-light text-[var(--portal-panel-heading)]">{ title }</h2>
            <p class="mt-0.5 text-[10px] font-light text-black/40">{ subtitle }</p>
        </div>
    }
}

fn metric_tile(label: &'static str, value: &'static str, hint: &'static str) -> Html {
    html! {
        <div class="rounded-md border border-[var(--portal-border)] bg-white/35 p-3">
            <p class="text-[9px] font-medium uppercase tracking-[0.13em] text-[var(--portal-blue-gray)]">{ label }</p>
            <p class="mt-1 font-serif text-xl font-light text-[var(--portal-navy)]">{ value }</p>
            <p class="mt-1 text-[9px] font-light text-black/40">{ hint }</p>
        </div>
    }
}

fn status_badge(status: &'static str) -> Html {
    let tone = match status {
        "Active" => "border-emerald-300/70 bg-emerald-50 text-emerald-800",
        "Follow-up" => "border-amber-300/70 bg-amber-50 text-amber-900",
        _ => "border-sky-300/70 bg-sky-50 text-sky-800",
    };
    html! {
        <span class={classes!("inline-flex", "rounded-full", "border", "px-2", "py-0.5", "text-[9px]", "font-medium", tone)}>
            { status }
        </span>
    }
}


fn command_row(label: &'static str, shortcut: &'static str) -> Html {
    html! {
        <button type="button" class="flex w-full items-center justify-between rounded-md px-3 py-2.5 text-left text-sm text-[var(--portal-navy)] transition hover:bg-[var(--portal-blue-pale)]/60">
            <span>{ label }</span>
            <span class="text-[9px] font-medium uppercase tracking-[0.1em] text-black/35">{ shortcut }</span>
        </button>
    }
}
