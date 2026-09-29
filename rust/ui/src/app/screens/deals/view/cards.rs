//! The deal workspace's summary cards: health, property and client.

#[allow(unused_imports)]
use super::*;

pub(super) fn deal_health_card(health: &crate::model::PortalDealHealth) -> Html {
    let band = match health.band.as_str() {
        "attention" => "Needs attention",
        "watch" => "Watch",
        "closed" => "Closed",
        _ => "Ready",
    };
    let band_class = match health.band.as_str() {
        "attention" => "bg-red-100 text-red-800",
        "watch" => "bg-amber-100 text-amber-800",
        "closed" => "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]",
        _ => "bg-[var(--portal-success-pale)] text-[var(--portal-success)]",
    };
    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="flex flex-wrap items-center justify-between gap-4 border-b border-[var(--portal-panel-border)] px-5 py-4">
                <div>
                    <p class="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">{"Closing health"}</p>
                    <div class="mt-1 flex items-baseline gap-3">
                        <span class="font-serif text-4xl font-light text-[var(--portal-navy)]">{ format!("{}%", health.score) }</span>
                        <span class={format!("rounded-full px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.12em] {band_class}")}>{ band }</span>
                    </div>
                    <p class="mt-1 text-xs font-light text-black/40">
                        { format!("{} of {} checks ready · derived from the current deal facts", health.ready_count, health.total_count) }
                    </p>
                </div>
            </div>
            <div class="divide-y divide-[var(--portal-panel-border)]">
                { for health.signals.iter().map(|signal| html! {
                    <div class="grid gap-2 px-5 py-3 sm:grid-cols-[26px_180px_1fr]">
                        <div class={classes!(
                            "flex", "h-6", "w-6", "items-center", "justify-center", "rounded-full", "text-[11px]", "font-medium",
                            if signal.ready { "bg-[var(--portal-success-pale)] text-[var(--portal-success)]" }
                            else if signal.severity == "attention" { "bg-red-100 text-red-800" }
                            else { "bg-amber-100 text-amber-800" }
                        )}>
                            { if signal.ready { "✓" } else { "!" } }
                        </div>
                        <div class="text-sm font-medium text-[var(--portal-navy)]">{ signal.label.clone() }</div>
                        <div class="text-xs font-light leading-5 text-black/50">{ signal.detail.clone() }</div>
                    </div>
                }) }
            </div>
        </section>
    }
}

pub(super) fn property_card(property: Option<&crate::model::PortalDealWorkspaceProperty>) -> Html {
    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="border-b border-[var(--portal-panel-border)] px-5 py-4">
                <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Property"}</h2>
            </div>
            <div class="px-5 py-4">
                if let Some(property) = property {
                    <a href={format!("/portal/property-admin/{}", property.id)}
                        class="font-serif text-xl font-light text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]">
                        { property.name.clone() }
                    </a>
                    <p class="mt-2 text-xs font-light text-black/45">
                        { property.location.clone().unwrap_or_else(|| "—".into()) }
                    </p>
                    <p class="mt-2 text-xs font-light text-black/45">
                        { property_descriptor(property) }
                    </p>
                } else {
                    <p class="text-sm font-light text-black/40">{"No property on record."}</p>
                }
            </div>
        </section>
    }
}

pub(super) fn client_card(client: Option<&crate::model::PortalDealWorkspaceClient>) -> Html {
    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="border-b border-[var(--portal-panel-border)] px-5 py-4">
                <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Client"}</h2>
            </div>
            <div class="px-5 py-4">
                if let Some(client) = client {
                    <a href={format!("/portal/clients/{}", client.id)}
                        class="font-serif text-xl font-light text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]">
                        { client.display_name.clone() }
                    </a>
                    if let Some(email) = client.email.as_ref() {
                        <p class="mt-2 text-xs font-light text-black/45">{ email.clone() }</p>
                    }
                    if let Some(phone) = client.phone.as_ref() {
                        <p class="mt-1 text-xs font-light text-black/45">{ phone.clone() }</p>
                    }
                } else {
                    <p class="text-sm font-light text-black/40">{"No client on record."}</p>
                }
            </div>
        </section>
    }
}
