//! The Accounting module's surfaces: the navy page frame, the glass panel, the metric card and the trend chart.
//!
//! The two display forms these screens are covered in — money and dates — live in `crate::format`, outside this module,
//! because they are pure and this module is compiled only for the browser. That is what lets their rounding rules be
//! tested on an ordinary host target.
//!
//! The one place a float is allowed is `PnlTrendChart`'s geometry: a money value becomes a pixel coordinate there, and a
//! pixel is not an amount. Every figure the screen PRINTS comes from the decimal strings.

use crate::format::format_money;
use yew::prelude::*;

/// The accounting page frame: the navy surface, its eyebrow and its title.
#[derive(Properties, PartialEq)]
pub struct AccountingShellProps {
    #[prop_or_default]
    pub eyebrow: Option<AttrValue>,
    #[prop_or_default]
    pub title: Option<AttrValue>,
    #[prop_or_default]
    pub children: Children,
}

#[function_component]
pub fn AccountingShell(props: &AccountingShellProps) -> Html {
    let heading = match (props.eyebrow.clone(), props.title.clone()) {
        (None, None) => Html::default(),
        (eyebrow, title) => html! {
            <div class="mb-5">
                if let Some(eyebrow) = eyebrow {
                    <p class="text-[10px] font-medium uppercase tracking-[0.24em] text-[var(--portal-gold)]">{ eyebrow }</p>
                }
                if let Some(title) = title {
                    <h1 class="mt-1 font-serif text-2xl font-light leading-tight text-white">{ title }</h1>
                }
            </div>
        },
    };
    html! {
        <div class="rounded-[var(--portal-panel-radius)] border border-[var(--portal-gold)]/25 bg-[var(--portal-navy-deep)] p-4 text-white shadow-[0_24px_70px_rgba(3,15,35,0.45)] sm:p-5 lg:p-6">
            { heading }
            { for props.children.iter() }
        </div>
    }
}

/// A glass panel: a titled surface with an optional action on its right, which is how every block on these screens is
/// built. Having it once is what keeps the four dashboard panels on the same rails.
#[derive(Properties, PartialEq)]
pub struct GlassPanelProps {
    #[prop_or_default]
    pub title: Option<AttrValue>,
    /// The action on the right of the title bar — a link, usually.
    #[prop_or_default]
    pub action: Option<Html>,
    /// Layout classes the caller adds to the panel itself — a grid span, usually.
    ///
    /// FROM THE LIVE COMPONENT'S `className`, and it earned its place by being missed: the Expenses screen splits its row
    /// 40/60 — the ring takes two columns of five and the table the other three — and a panel with nowhere to put a span
    /// silently gives each child one column of a five-column grid, which leaves the row two-fifths empty and the table
    /// squeezed into a fifth of the width it was designed for. That is what "the table is cut off" looks like.
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Children,
}

#[function_component]
pub fn GlassPanel(props: &GlassPanelProps) -> Html {
    let has_header = props.title.is_some() || props.action.is_some();
    html! {
        <section class={classes!("rounded-[var(--portal-panel-radius)]", "border", "border-white/10",
            "bg-gradient-to-b", "from-white/[0.06]", "to-white/[0.02]", props.class.clone())}>
            if has_header {
                <div class="flex items-center justify-between gap-3 border-b border-white/10 px-4 py-3">
                    if let Some(title) = props.title.clone() {
                        <h2 class="text-[11px] font-medium uppercase tracking-[0.18em] text-white/70">{ title }</h2>
                    }
                    { props.action.clone().unwrap_or_default() }
                </div>
            }
            <div class="p-4">{ for props.children.iter() }</div>
        </section>
    }
}

/// How a figure should read: plainly, or as good news, a warning, or bad news.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    Neutral,
    Good,
    Warn,
    Bad,
}

impl Tone {
    fn class(self) -> &'static str {
        match self {
            Tone::Good => "text-emerald-300",
            Tone::Warn => "text-amber-300",
            Tone::Bad => "text-rose-300",
            Tone::Neutral => "text-white",
        }
    }
}

/// One figure with its label and its hint. The value arrives already formatted, because formatting money is a decision
/// made once in `format_money` rather than at each call site.
#[derive(Properties, PartialEq)]
pub struct MetricCardProps {
    pub label: AttrValue,
    pub value: AttrValue,
    #[prop_or_default]
    pub hint: Option<AttrValue>,
    #[prop_or_default]
    pub tone: Tone,
}

#[function_component]
pub fn MetricCard(props: &MetricCardProps) -> Html {
    html! {
        <div class="rounded-[var(--portal-panel-radius)] border border-white/10 bg-gradient-to-b from-white/[0.09] to-white/[0.03] p-4">
            <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold)]">{ props.label.clone() }</p>
            <p class={classes!("mt-2", "font-serif", "text-3xl", "font-light", props.tone.class())}>
                { props.value.clone() }
            </p>
            if let Some(hint) = props.hint.clone() {
                <p class="mt-1 text-[11px] font-light text-white/50">{ hint }</p>
            }
        </div>
    }
}

/// The six-month trend, drawn as the dependency-free SVG the live screen drew.
///
/// GEOMETRY, NOT ACCOUNTING. The amounts become pixel coordinates here, which is the one job a float is right for: a point
/// on a 120-unit-tall canvas is not an amount of money, and nothing is read back off the coordinates — the figures beside
/// the chart come from the payload's strings. The scale is symmetric about the middle line, so a month that lost money is
/// visibly below zero rather than flattened onto it.
#[derive(Properties, PartialEq)]
pub struct PnlTrendChartProps {
    pub points: Vec<crate::model::PortalAccountingTrendPoint>,
}

/// The drawn series, in the order and at the widths the live chart used.
const TREND_SERIES: [(&str, &str, f64, f64); 3] = [
    ("income", "#c6a15b", 1.5, 0.55),
    ("expenses", "#7dd3fc", 1.5, 0.55),
    // Net is drawn last and solid, so it reads on top of the two it is the difference of.
    ("net", "#ffffff", 2.5, 1.0),
];

#[function_component]
pub fn PnlTrendChart(props: &PnlTrendChartProps) -> Html {
    const WIDTH: f64 = 640.0;
    const HEIGHT: f64 = 120.0;
    const PAD: f64 = 18.0;

    let points = &props.points;
    let magnitude = |value: &str| value.trim().parse::<f64>().unwrap_or(0.0);
    let max = points
        .iter()
        .flat_map(|point| {
            [
                magnitude(&point.income).abs(),
                magnitude(&point.expenses).abs(),
                magnitude(&point.net).abs(),
            ]
        })
        .fold(1.0_f64, f64::max);

    let span = points.len().saturating_sub(1).max(1) as f64;
    let x = |index: usize| PAD + (index as f64 * (WIDTH - PAD * 2.0)) / span;
    let y = |value: f64| PAD + ((max - value) / (max * 2.0)) * (HEIGHT - PAD * 2.0);
    let value_for = |point: &crate::model::PortalAccountingTrendPoint, key: &str| {
        let raw = match key {
            "income" => point.income.as_str(),
            "expenses" => point.expenses.as_str(),
            _ => point.net.as_str(),
        };
        magnitude(raw)
    };

    html! {
        <div>
            <svg viewBox={format!("0 0 {WIDTH} {HEIGHT}")} class="h-auto w-full">
                <line x1={PAD.to_string()} x2={(WIDTH - PAD).to_string()} y1={(HEIGHT / 2.0).to_string()}
                    y2={(HEIGHT / 2.0).to_string()} stroke="rgba(255,255,255,0.12)" stroke-dasharray="3 4" />
                { for points.iter().enumerate().map(|(index, point)| html! {
                    <text x={x(index).to_string()} y={(HEIGHT - 6.0).to_string()} text-anchor="middle"
                        fill="rgba(255,255,255,0.4)" style="font-size: 9px">
                        { point.month.clone() }
                    </text>
                }) }
                { for TREND_SERIES.iter().map(|(key, colour, width, opacity)| html! {
                    <polyline points={points.iter().enumerate().map(|(index, point)| {
                            format!("{},{}", x(index), y(value_for(point, key)))
                        }).collect::<Vec<_>>().join(" ")}
                        fill="none" stroke={*colour} stroke-width={width.to_string()}
                        stroke-linejoin="round" stroke-linecap="round" opacity={opacity.to_string()} />
                }) }
            </svg>
            <div class="mt-2 flex flex-wrap items-center gap-x-4 gap-y-1 text-[10px] font-light uppercase tracking-[0.16em] text-white/50">
                <span class="flex items-center gap-1.5">
                    <span class="h-1.5 w-3 rounded-full bg-[#c6a15b]"></span>{"Income"}
                </span>
                <span class="flex items-center gap-1.5">
                    <span class="h-1.5 w-3 rounded-full bg-sky-300"></span>{"Expenses"}
                </span>
                <span class="flex items-center gap-1.5">
                    <span class="h-1.5 w-3 rounded-full bg-white"></span>{"Net"}
                </span>
                <span class="ml-auto text-white/60">
                    { format_money(points.first().map(|point| point.net.as_str()).unwrap_or("0")) }
                </span>
            </div>
        </div>
    }
}
