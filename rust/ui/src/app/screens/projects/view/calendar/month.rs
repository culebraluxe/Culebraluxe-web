//! The calendar's month view and its chips.

#[allow(unused_imports)]
use super::*;

pub(super) fn month_view(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    chips: &[CalendarChip],
    on_msg: &Callback<Msg>,
) -> Html {
    let days = crate::calendar::month_cells(&projects.calendar_cursor);
    html! {
        <div class="flex h-full min-h-0 flex-col">
            <div class="grid shrink-0 grid-cols-7 border-b border-[var(--portal-navy)]/15 bg-white/45">
                { for ["Sun","Mon","Tue","Wed","Thu","Fri","Sat"].into_iter().map(|label| html! {
                    <div class="px-2 py-1.5 text-center text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy)]/70">{ label }</div>
                }) }
            </div>
            <div class="grid min-h-0 flex-1 grid-cols-7 grid-rows-6">
                { for days.into_iter().map(|day| {
                    let is_today = day.date == projects.calendar_today;
                    let day_events = chips.iter().filter(|event| event.date == day.date).collect::<Vec<_>>();
                    // A cell fits three chips; when there are more, show two and say how many more.
                    let shown = if day_events.len() > 3 { 2 } else { day_events.len() };
                    let extra = day_events.len() - shown;
                    html! {
                        <div
                            key={day.date.clone()}
                            class={classes!(
                                "min-h-0","overflow-hidden","border-b","border-r","border-[var(--portal-navy)]/15","p-1",
                                (!day.in_month).then_some("bg-[var(--portal-navy)]/[0.07]"),
                                is_today.then_some("bg-[var(--portal-gold)]/[0.09]")
                            )}
                        >
                            <div class="mb-0.5 flex items-center justify-between">
                                <span class={classes!(
                                    "flex","h-5","w-5","items-center","justify-center","rounded-full","text-[12px]",
                                    if is_today { "bg-[var(--portal-gold)] font-semibold text-[var(--portal-navy)]" }
                                    else if day.in_month { "font-medium text-[var(--portal-navy)]" } else { "text-[var(--portal-navy)]/30" }
                                )}>
                                    { day.day }
                                </span>
                            </div>
                            <div class="space-y-0.5">
                                { for day_events.iter().take(shown).map(|event| month_chip(model, projects, event, on_msg)) }
                                if extra > 0 {
                                    <div class="px-1 text-[10px] font-semibold text-[var(--portal-navy)]">
                                        { format!("+{extra} more") }
                                    </div>
                                }
                            </div>
                        </div>
                    }
                }) }
            </div>
        </div>
    }
}

pub(super) fn month_chip(
    model: &Vm<'_>,
    projects: &PortalProjectsPage,
    event: &&CalendarChip,
    on_msg: &Callback<Msg>,
) -> Html {
    let tooltip = event
        .time
        .as_ref()
        .map(|time| format!("{time} · {}", event.title))
        .unwrap_or_else(|| event.title.clone());
    html! {
        <button
            type="button"
            key={event.id.clone()}
            title={tooltip}
            onclick={select_event(event, on_msg)}
            class={classes!(
                "block","w-full","truncate","rounded","border","px-1.5","py-[2px]","text-left","text-[10px]","leading-tight","shadow-sm",
                event.tone(),
                selected(projects, event).then_some("ring-1 ring-[var(--portal-gold)]"),
                pending(model, event).then_some("opacity-70")
            )}
        >
            if event.recurring {
                <span class="mr-1" aria-label="Recurring">{"↻"}</span>
            }
            if let Some(time) = event.time.as_ref() {
                <span class="mr-1 font-semibold">{ time }</span>
            }
            { event.title.clone() }
        </button>
    }
}
