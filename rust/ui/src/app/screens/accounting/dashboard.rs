//! `/portal/accounting` — the Accounting dashboard, on Yew.
//!
//! PARITY WITH `components/portal/accounting/accounting-dashboard.tsx`. Four figures, then the six-month trend and the
//! month's category ring, then money in and money out, then the receipt band. Every value on it is a projection the
//! database computed; nothing here adds money up, and nothing here divides it — the shares the ring is drawn from arrive
//! as percentages, because `amount / total` in a browser is a float division of money.
//!
//! THE EMPTY STATES ARE THE LIVE ONES, word for word. A dashboard with no data is a real state on a new book, and it says
//! what is missing rather than showing four zeroes and an empty ring.

use yew::prelude::*;

use crate::format::{format_date, format_money, is_zero};
use crate::model::{PortalAccountingDashboard, PortalAccountingShare};

use super::shell::{GlassPanel, MetricCard, PnlTrendChart, Tone};
use super::{Msg, Vm};

/// The ring's slice colours, in the order the live screen used them.
const DONUT_COLOURS: [&str; 9] = [
    "#c6a15b", "#7dd3fc", "#f0abfc", "#86efac", "#fcd34d", "#fca5a5", "#a5b4fc", "#93c5fd",
    "#e2e8f0",
];

/// The two-column rail the analytics row and the money-in/money-out row share, so their edges line up at desktop widths.
const TWO_COLUMN_GRID: &str = "grid grid-cols-1 gap-4 lg:grid-cols-2";

super::accounting_screen!(Dashboard, "accounting", "Dashboard", body);

/// The view's helpers. The screen above is the shared model and reducer; this is only how it is drawn.
struct View;

fn body(model: &Vm<'_>, _on_msg: &Callback<Msg>) -> Html {
    View.body(model)
}

impl View {
    /// The whole screen, or the one thing it can honestly say without data.
    fn body(&self, model: &Vm<'_>) -> Html {
        let Some(dashboard) = model
            .book
            .and_then(|accounting| accounting.dashboard.as_ref())
        else {
            // No payload yet is not an error and not a set of zeroes: the shell above already shows whether the read is
            // still loading, so this says only that there is nothing to draw yet.
            return html! {
                <p class="text-sm font-light text-white/40">{"Loading the book…"}</p>
            };
        };
        html! {
            <div class="space-y-4">
                { self.metrics(dashboard) }
                <div class={TWO_COLUMN_GRID}>
                    { self.trend_panel(dashboard) }
                    { self.category_panel(dashboard) }
                </div>
                <div class={TWO_COLUMN_GRID}>
                    { self.receivables_panel(dashboard) }
                    { self.expenses_panel(dashboard) }
                </div>
                { self.receipt_band() }
            </div>
        }
    }
}

impl View {
    /// The four figures. Net income's tone follows its sign, and the overdue count warns only when there is something to
    /// be warned about — a red zero trains people to ignore red.
    fn metrics(&self, dashboard: &PortalAccountingDashboard) -> Html {
        let net_negative = dashboard.net_income.trim().starts_with('-');
        html! {
            <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
                <MetricCard label="Receivables" value={format_money(&dashboard.receivables_outstanding)}
                    hint="Outstanding (OPEN)" />
                <MetricCard label="Expenses This Month" value={format_money(&dashboard.expenses_this_month)}
                    hint="POSTED" />
                <MetricCard label="Net Income" value={format_money(&dashboard.net_income)}
                    hint="Realized income − posted expenses"
                    tone={if net_negative { Tone::Bad } else { Tone::Good }} />
                <MetricCard label="Open / Overdue Receivables"
                    value={format!("{} / {}", dashboard.open_count, dashboard.overdue_count)}
                    hint={format!("{} past due", dashboard.overdue_count)}
                    tone={if dashboard.overdue_count > 0 { Tone::Warn } else { Tone::Neutral }} />
            </div>
        }
    }

    /// The trend panel: the six months added up, the chart, and the line under it.
    fn trend_panel(&self, dashboard: &PortalAccountingDashboard) -> Html {
        html! {
            <GlassPanel title="P&L / Net Income Trend">
                <div class="grid grid-cols-3 gap-3 border-b border-white/10 pb-3">
                    { self.trend_total("Total Income", &dashboard.trend_income, "text-white") }
                    { self.trend_total("Total Expenses", &dashboard.trend_expenses, "text-sky-300") }
                    { self.trend_total(
                        "Net Income",
                        &dashboard.trend_net,
                        if dashboard.trend_net.trim().starts_with('-') { "text-rose-300" } else { "text-emerald-300" },
                    ) }
                </div>
                <div class="mt-2">
                    <PnlTrendChart points={dashboard.pnl_trend.clone()} />
                </div>
                <p class="mt-1 text-[9px] font-light text-white/40">{"Last 6 months · How are we doing?"}</p>
            </GlassPanel>
        }
    }

    fn trend_total(&self, label: &str, amount: &str, tone: &'static str) -> Html {
        html! {
            <div>
                <p class="text-[9px] font-medium uppercase tracking-[0.16em] text-[var(--portal-gold)]">{ label }</p>
                <p class={classes!("mt-0.5", "font-serif", "text-lg", "font-light", tone)}>{ format_money(amount) }</p>
            </div>
        }
    }
}

impl View {
    /// This month's cost by category, as a ring with its legend — or the live screen's own sentence when the month is
    /// empty, which is a real state on a quiet month and not a failure to load.
    ///
    /// THE BREAKDOWN IS THE MONTH'S P&L AGGREGATION, not a second one. The live page read it with `getPnlStatement` for
    /// the current month; the dashboard projection computes the same thing in one query, so there is no second
    /// aggregation to disagree with this one.
    fn category_panel(&self, dashboard: &PortalAccountingDashboard) -> Html {
        let categories = dashboard.expense_categories.clone();
        let total = dashboard.expenses_this_month.clone();
        html! {
            <GlassPanel title="Expenses by Category · This Month">
                if categories.is_empty() || is_zero(&total) {
                    <p class="py-6 text-sm font-light text-white/40">{"No posted expenses this month."}</p>
                } else {
                    <div class="flex items-center gap-4">
                        { self.donut(&categories, &total) }
                        <ul class="min-w-0 flex-1 space-y-1">
                            { for categories.iter().enumerate().map(|(index, category)| html! {
                                <li class="flex items-center gap-2 text-[11px]">
                                    <span class="h-2 w-2 flex-none rounded-full"
                                        style={format!("background: {}", DONUT_COLOURS[index % DONUT_COLOURS.len()])}></span>
                                    <span class="min-w-0 flex-1 truncate text-white/70">{ category.label.clone() }</span>
                                    <span class="flex-none text-[10px] text-white/40">{ format!("{}%", category.percent) }</span>
                                    <span class="flex-none text-white/85">{ format_money(&category.amount) }</span>
                                </li>
                            }) }
                        </ul>
                    </div>
                }
            </GlassPanel>
        }
    }

    /// The ring, drawn from the percentages the database computed.
    ///
    /// The stops are built by INTEGER arithmetic on those percentages, so no float division of money happens here either.
    /// Rounded percentages can leave the last stop a degree short of a full turn; a degree of a ring that is drawn from
    /// the figures printed beside it is the right thing to lose.
    fn donut(&self, categories: &[PortalAccountingShare], total: &str) -> Html {
        let mut cursor = 0_i64;
        let stops = categories
            .iter()
            .enumerate()
            .map(|(index, category)| {
                let from = cursor;
                let to = cursor + category.percent.max(0) * 360 / 100;
                cursor = to;
                format!(
                    "{} {from}deg {to}deg",
                    DONUT_COLOURS[index % DONUT_COLOURS.len()]
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        html! {
            <div class="relative h-28 w-28 flex-none rounded-full" style={format!("background: conic-gradient({stops})")}>
                <div class="absolute inset-[24%] flex flex-col items-center justify-center rounded-full bg-[var(--portal-navy-deep)]">
                    <span class="text-[9px] font-medium uppercase tracking-wide text-white/50">{"Total"}</span>
                    <span class="font-serif text-[11px] text-white/85">{ format_money(total) }</span>
                </div>
            </div>
        }
    }
}

impl View {
    /// Money in: the six most recent live receivables, with the name that identifies each one.
    ///
    /// The identifying name is the person, else the property, else the deal — the order the live screen used, because a
    /// commission belongs to a client first and to a building second.
    fn receivables_panel(&self, dashboard: &PortalAccountingDashboard) -> Html {
        let rows = dashboard.recent_activity.iter().take(6).collect::<Vec<_>>();
        html! {
            <GlassPanel title="Money In / Receivables" action={self.panel_link("/portal/accounting/receivables", "View Receivables")}>
                <div class="overflow-x-auto">
                    <table class="w-full text-left text-xs">
                        <thead>
                            <tr class="border-b border-white/10 text-[9px] font-medium uppercase tracking-[0.14em] text-white/40">
                                <th class="py-1.5 pr-3">{"Client / Property"}</th>
                                <th class="py-1.5 pr-3">{"Description"}</th>
                                <th class="py-1.5 pr-3 text-right">{"Amount"}</th>
                                <th class="py-1.5 pr-3">{"Due"}</th>
                                <th class="py-1.5">{"Status"}</th>
                            </tr>
                        </thead>
                        <tbody>
                            { for rows.iter().map(|receivable| html! {
                                <tr class="border-b border-white/[0.05]">
                                    <td class="py-2 pr-3 text-white/70">
                                        { receivable
                                            .person_name
                                            .clone()
                                            .or_else(|| receivable.property_name.clone())
                                            .or_else(|| receivable.deal_name.clone())
                                            .unwrap_or_else(|| "—".to_string()) }
                                    </td>
                                    <td class="py-2 pr-3 text-white/85">{ receivable.description.clone() }</td>
                                    <td class="py-2 pr-3 text-right text-white/90">{ format_money(&receivable.amount) }</td>
                                    <td class="py-2 pr-3 text-white/60">{ self.due_label(receivable.due_on.as_deref()) }</td>
                                    <td class="py-2">{ self.status_badge(&receivable.status) }</td>
                                </tr>
                            }) }
                            if rows.is_empty() {
                                <tr>
                                    <td colspan="5" class="py-4 text-center text-white/40">{"No receivables yet."}</td>
                                </tr>
                            }
                        </tbody>
                    </table>
                </div>
            </GlassPanel>
        }
    }

    /// A due date, or the dash the live screen put in its place — an absent deadline is a fact about the receivable.
    fn due_label(&self, due_on: Option<&str>) -> String {
        let label = format_date(due_on);
        if label.is_empty() {
            "—".to_string()
        } else {
            label
        }
    }

    /// The status pill. Three states, three colours, and no fourth — a receivable is open, paid or void.
    fn status_badge(&self, status: &str) -> Html {
        let tone = match status {
            "PAID" => "border-emerald-400/40 text-emerald-300",
            "VOID" => "border-white/20 text-white/40",
            _ => "border-[var(--portal-gold)]/50 text-[var(--portal-gold)]",
        };
        html! {
            <span class={classes!("inline-flex", "rounded-full", "border", "px-1.5", "py-0.5", "text-[9px]",
                "font-medium", "uppercase", "tracking-wide", tone)}>
                { status }
            </span>
        }
    }

    /// A panel's action: a real anchor, because the portal's URLs belong to Next rather than to this app's router.
    fn panel_link(&self, href: &str, label: &str) -> Html {
        html! {
            <a href={href.to_string()}
                class="text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)] transition hover:text-white">
                { label }
            </a>
        }
    }
}

impl View {
    /// Money out: the most recent posted expenses.
    fn expenses_panel(&self, dashboard: &PortalAccountingDashboard) -> Html {
        let rows = dashboard.recent_expenses.iter().take(6).collect::<Vec<_>>();
        html! {
            <GlassPanel title="Money Out / Expenses" action={self.panel_link("/portal/accounting/expenses", "View Expenses")}>
                <div class="overflow-x-auto">
                    <table class="w-full text-left text-xs">
                        <thead>
                            <tr class="border-b border-white/10 text-[9px] font-medium uppercase tracking-[0.14em] text-white/40">
                                <th class="py-1.5 pr-3">{"Date"}</th>
                                <th class="py-1.5 pr-3">{"Vendor"}</th>
                                <th class="py-1.5 pr-3">{"Category"}</th>
                                <th class="py-1.5 text-right">{"Amount"}</th>
                            </tr>
                        </thead>
                        <tbody>
                            { for rows.iter().map(|expense| html! {
                                <tr class="border-b border-white/[0.05]">
                                    <td class="py-2 pr-3 text-white/60">{ format_date(Some(expense.expense_on.as_str())) }</td>
                                    <td class="py-2 pr-3 text-white/85">{ expense.vendor.clone() }</td>
                                    <td class="py-2 pr-3 text-white/70">{ expense.category.clone() }</td>
                                    <td class="py-2 text-right text-sky-300">{ format_money(&expense.amount) }</td>
                                </tr>
                            }) }
                            if rows.is_empty() {
                                <tr>
                                    <td colspan="4" class="py-4 text-center text-white/40">{"No expenses yet."}</td>
                                </tr>
                            }
                        </tbody>
                    </table>
                </div>
            </GlassPanel>
        }
    }

    /// The receipt band.
    ///
    /// THE NOTE IS THE LIVE ONE, deliberately. The screen says OCR is a later pass and the scanner is a V1 prototype,
    /// because saying so is more useful than a button that implies recognition the system does not do. Nothing here
    /// reaches the Swift Vision tool, and nothing here pretends to.
    fn receipt_band(&self) -> Html {
        let primary = "inline-flex items-center justify-center rounded-md bg-[var(--portal-gold)] px-4 py-2 text-xs \
                       font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy-deep)] transition hover:brightness-110";
        let secondary = "rounded-md border border-white/20 px-3 py-2 text-[10px] font-medium uppercase \
                         tracking-[0.12em] text-white/70 transition hover:border-[var(--portal-gold)] hover:text-white";
        html! {
            <GlassPanel title="Receipt / Needs Review">
                <div class="flex flex-wrap items-center justify-between gap-3">
                    <p class="text-sm font-light text-white/70">{"Capture a receipt and turn it into an expense."}</p>
                    <div class="flex flex-wrap items-center gap-2">
                        <a href="/portal/accounting/receipt-scanner" class={primary}>{"Scan New Receipt"}</a>
                        <a href="/portal/accounting/receipt-scanner" class={secondary}>{"Go to Receipt Scanner"}</a>
                    </div>
                </div>
                <p class="mt-2 text-[10px] font-light text-white/40">
                    {"OCR is a later pass — the scanner is a V1 prototype. Later: receipts awaiting review · unknown bank \
                      classifications · unmatched vendors · uncategorized expenses."}
                </p>
            </GlassPanel>
        }
    }
}
