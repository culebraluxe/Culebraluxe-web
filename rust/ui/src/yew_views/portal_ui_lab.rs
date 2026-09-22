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
}

impl Default for LabModel {
    fn default() -> Self {
        Self {
            tab: LabTab::Yew,
            query: String::new(),
            density: "comfortable",
            notifications: true,
            role: "Broker".into(),
        }
    }
}

enum LabMsg {
    TabSelected(LabTab),
    QueryChanged(String),
    DensityChanged(&'static str),
    NotificationsChanged(bool),
    RoleChanged(String),
    Reset,
}

fn reduce(model: &mut LabModel, msg: LabMsg) {
    match msg {
        LabMsg::TabSelected(tab) => model.tab = tab,
        LabMsg::QueryChanged(query) => model.query = query,
        LabMsg::DensityChanged(density) => model.density = density,
        LabMsg::NotificationsChanged(enabled) => model.notifications = enabled,
        LabMsg::RoleChanged(role) => model.role = role,
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
                { self.stack_matrix() }
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
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5">
                <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold)]">{"Preserved reference"}</p>
                <h2 class="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">{"React / TypeScript component gallery"}</h2>
                <p class="mt-3 max-w-3xl text-sm font-light leading-6 text-black/55">
                    {"The mature TypeScript gallery stays the reference for PageHeader, panels, fields, dialog, row menus, pagination, timelines and state patterns. It is not being rewritten just to appear here."}
                </p>
                <div class="mt-4 rounded-md border border-dashed border-[var(--portal-gold)]/60 bg-white/30 p-4">
                    <p class="text-xs font-medium text-[var(--portal-navy)]">{"Next adapter"}</p>
                    <p class="mt-1 text-[11px] font-light text-black/50">
                        {"Mount the preserved React gallery as one bounded island in this tab. Yew remains the route and application owner."}
                    </p>
                </div>
            </section>
        }
    }

    fn motion_lab(&self) -> Html {
        html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5">
                <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold)]">{"Interaction reference"}</p>
                <h2 class="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">{"Motion / Framer ideas"}</h2>
                <p class="mt-3 max-w-3xl text-sm font-light leading-6 text-black/55">
                    {"Keep the useful experiments — motion profiles, view transitions, image drift, hotspots, tilt and command-palette behavior — without making Framer the owner of the portal."}
                </p>
                <div class="mt-4 flex flex-wrap items-center gap-3">
                    <a
                        href="/portal/tech/framer-ui-lab"
                        class="inline-flex min-h-9 items-center rounded-md bg-[var(--portal-navy)] px-3 text-xs font-medium text-white transition hover:bg-[var(--portal-navy-soft)]"
                    >
                        {"Open preserved Framer lab"}
                    </a>
                    <span class="text-[11px] font-light text-black/45">{"Later this becomes a bounded island in this tab."}</span>
                </div>
            </section>
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
