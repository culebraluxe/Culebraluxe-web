//! Offers: the comparison, each offer, and the offer form.

#[allow(unused_imports)]
use super::*;

pub(super) fn offers_card(
    model: &Vm<'_>,
    workspace: &crate::model::PortalDealWorkspace,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-5 py-4">
                <div>
                    <p class="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">{"Offer Room"}</p>
                    <h2 class="mt-0.5 font-serif text-xl font-light text-[var(--portal-navy)]">{"Offers & counters"}</h2>
                    <p class="mt-1 text-xs font-light text-black/40">
                        {"Compare money, financing, deposit, inspection, credits and proposed closing without choosing for the seller."}
                    </p>
                </div>
                <span class="text-xs font-light text-black/35">{ workspace.offers.len() }</span>
            </div>

            if workspace.offers.is_empty() {
                <p class="px-5 py-6 text-sm font-light text-black/40">{"No offers on record for this deal."}</p>
            } else {
                <div class="overflow-x-auto border-b border-[var(--portal-panel-border)]">
                    <table class="w-full min-w-[920px] border-collapse text-left">
                        <thead>
                            <tr class="bg-white/20">
                                { table_head("Amount") }
                                { table_head("Financing") }
                                { table_head("Deposit") }
                                { table_head("Inspection") }
                                { table_head("Credits") }
                                { table_head("Closing") }
                                { table_head("Status") }
                            </tr>
                        </thead>
                        <tbody>
                            { for workspace.offers.iter().map(offer_compare_row) }
                        </tbody>
                    </table>
                </div>
                <div>
                    { for workspace.offers.iter().map(|offer| offer_row(model, offer, on_msg, busy)) }
                </div>
            }

            if workspace.client.is_some() {
                <div class="border-t border-[var(--portal-panel-border)] bg-white/20 px-5 py-5">
                    <p class="mb-3 text-[10px] font-medium uppercase tracking-[0.14em] text-black/40">{"New offer terms"}</p>
                    { offer_form(model, None, "Submit offer", on_msg, busy) }
                </div>
            }
        </section>
    }
}

pub(super) fn offer_compare_row(offer: &crate::model::PortalDealWorkspaceOffer) -> Html {
    let inspection = offer
        .inspection_days
        .map(|days| format!("{days} days"))
        .unwrap_or_else(|| "—".into());
    html! {
        <tr class="border-t border-[var(--portal-panel-border)]">
            <td class="px-4 py-3 font-serif text-lg font-light text-[var(--portal-navy)]">{ format_currency(Some(offer.amount)) }</td>
            <td class="px-4 py-3 text-sm font-light text-black/60">{ offer.financing_type.clone().unwrap_or_else(|| "—".into()) }</td>
            <td class="px-4 py-3 text-sm font-light tabular-nums text-black/60">{ format_currency(offer.deposit_amount) }</td>
            <td class="px-4 py-3 text-sm font-light text-black/60">{ inspection }</td>
            <td class="px-4 py-3 text-sm font-light tabular-nums text-black/60">{ format_currency(offer.seller_credits) }</td>
            <td class="px-4 py-3 text-sm font-light text-black/60">{ offer.proposed_closing_date.clone().unwrap_or_else(|| "—".into()) }</td>
            <td class="px-4 py-3 text-[10px] font-light uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">{ title_case(&offer.status) }</td>
        </tr>
    }
}

pub(super) fn offer_row(
    model: &Vm<'_>,
    offer: &crate::model::PortalDealWorkspaceOffer,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    let withdraw = {
        let on_msg = on_msg.clone();
        let id = offer.id.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::DealWorkspaceWithdrawOfferRequested {
                offer_id: id.clone(),
            })
        })
    };
    let reject = {
        let on_msg = on_msg.clone();
        let id = offer.id.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::DealWorkspaceRejectOfferRequested {
                offer_id: id.clone(),
            })
        })
    };
    html! {
        <div class="border-b border-[var(--portal-panel-border)] px-5 py-4 last:border-b-0">
            <div class="flex flex-wrap items-center gap-2">
                <span class="font-serif text-2xl font-light text-[var(--portal-navy)]">{ format_currency(Some(offer.amount)) }</span>
                <span class="rounded-full bg-[var(--portal-blue-pale)] px-2.5 py-1 text-[9px] uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">{ title_case(&offer.status) }</span>
                if offer.is_counter {
                    <span class="rounded-full border border-[var(--portal-panel-border)] px-2.5 py-1 text-[9px] uppercase tracking-[0.1em] text-black/45">{"Counter"}</span>
                }
            </div>
            <p class="mt-2 text-xs font-light text-black/45">
                { offer.person_name.clone().unwrap_or_else(|| "—".into()) }
                {" · Submitted "}
                { offer.submitted_at_label.clone() }
                if let Some(responded) = offer.responded_at_label.as_ref() {
                    {" · Responded "}
                    { responded.clone() }
                }
                if let Some(expires) = offer.expires_at_label.as_ref() {
                    {" · Expires "}
                    { expires.clone() }
                }
            </p>
            if let Some(contingencies) = offer.contingencies.as_ref().filter(|value| !value.trim().is_empty()) {
                <p class="mt-2 text-sm font-light text-black/55">{ contingencies.clone() }</p>
            }
            if let Some(note) = offer.note.as_ref() {
                <p class="mt-2 text-sm font-light text-black/55">{ note.clone() }</p>
            }
            if offer.status == "submitted" {
                <div class="mt-3 flex flex-wrap gap-2">
                    <button type="button" onclick={withdraw} disabled={busy} class={small_action_class()}>{"Withdraw"}</button>
                    <button type="button" onclick={reject} disabled={busy} class={small_action_class()}>{"Reject"}</button>
                </div>
                <details class="mt-4">
                    <summary class="cursor-pointer text-[10px] font-medium uppercase tracking-[0.13em] text-[var(--portal-navy-soft)]">{"Counter offer"}</summary>
                    <div class="mt-3">
                        { offer_form(model, Some(offer.id.clone()), "Submit counter", on_msg, busy) }
                    </div>
                </details>
            }
        </div>
    }
}

pub(super) fn offer_form(
    model: &Vm<'_>,
    parent_offer_id: Option<String>,
    label: &'static str,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    let key = parent_offer_id.clone().unwrap_or_else(|| "root".into());
    let amount = draft(&model.deal_workspace.offer_amounts, &key);
    let financing = draft(&model.deal_workspace.offer_financing, &key);
    let deposit = draft(&model.deal_workspace.offer_deposits, &key);
    let inspection = draft(&model.deal_workspace.offer_inspection_days, &key);
    let credits = draft(&model.deal_workspace.offer_seller_credits, &key);
    let closing = draft(&model.deal_workspace.offer_closing_dates, &key);
    let contingencies = draft(&model.deal_workspace.offer_contingencies, &key);
    let expiration = draft(&model.deal_workspace.offer_expirations, &key);

    let amount_change = {
        let on_msg = on_msg.clone();
        let key = key.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceOfferAmountChanged {
                key: key.clone(),
                value: crate::app::exec::input_value(&event),
            })
        })
    };
    let term_input = |field: &'static str| {
        let on_msg = on_msg.clone();
        let key = key.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceOfferTermChanged {
                key: key.clone(),
                field,
                value: crate::app::exec::input_value(&event),
            })
        })
    };
    let text_area = {
        let on_msg = on_msg.clone();
        let key = key.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceOfferTermChanged {
                key: key.clone(),
                field: "contingencies",
                value: crate::app::exec::textarea_value(&event),
            })
        })
    };
    let submit = {
        let on_msg = on_msg.clone();
        let parent_offer_id = parent_offer_id.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::DealWorkspaceSubmitOfferRequested {
                parent_offer_id: parent_offer_id.clone(),
            })
        })
    };
    html! {
        <div class="space-y-3">
            <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
                <input type="number" min="1" step="1" value={amount} oninput={amount_change}
                    placeholder="Offer amount" class={field_class()} />
                <input value={financing} oninput={term_input("financing")}
                    placeholder="Financing (Cash, Conventional…)" class={field_class()} />
                <input type="number" min="0" step="1" value={deposit} oninput={term_input("deposit")}
                    placeholder="Deposit" class={field_class()} />
                <input type="number" min="0" max="365" step="1" value={inspection} oninput={term_input("inspectionDays")}
                    placeholder="Inspection days" class={field_class()} />
                <input type="number" min="0" step="1" value={credits} oninput={term_input("sellerCredits")}
                    placeholder="Seller credits" class={field_class()} />
                <input type="date" value={closing} oninput={term_input("closingDate")} class={field_class()} />
                <input type="datetime-local" value={expiration} oninput={term_input("expiresAt")} class={field_class()} />
            </div>
            <textarea value={contingencies} oninput={text_area} rows="2"
                placeholder="Contingencies / notes that affect comparison" class="block min-h-20 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 py-2 text-sm font-light outline-none focus:border-[var(--portal-navy)]" />
            <div class="flex justify-end">
                <button type="button" onclick={submit} disabled={busy || model.deal_workspace.offer_amounts.get(&key).is_none_or(|value| value.trim().is_empty())}
                    class="min-h-10 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[9px] font-medium uppercase tracking-[0.12em] text-white disabled:opacity-35">
                    { label }
                </button>
            </div>
        </div>
    }
}

pub(super) fn draft(values: &std::collections::BTreeMap<String, String>, key: &str) -> String {
    values.get(key).cloned().unwrap_or_default()
}
