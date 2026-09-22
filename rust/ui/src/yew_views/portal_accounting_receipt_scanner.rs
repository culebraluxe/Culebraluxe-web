//! `/portal/accounting/receipt-scanner` — the receipt workflow, demonstrated.
//!
//! FAKE V1, AND IT SAYS SO ON THE SCREEN. There is no OCR here and none is implied: the live component cycled four fixed
//! demonstration receipts, the operator reviewed one, and saving it recorded an expense. That behaviour is preserved
//! exactly, including the amber notice that tells the reader the extraction is a deterministic seed — a screen that implied
//! recognition it does not do would be worse than one that admits the demonstration.
//!
//! WHAT IS NOT HERE, deliberately: `OCR/receipt-ocr.swift` and its Apple Vision extraction. Wiring a macOS executable into
//! the web path is a deployment decision with its own architecture, not a screen's business, and real OCR is a separate
//! story. Nothing in this file reads a file's contents, calls a vendor, or reaches outside the browser.
//!
//! STATE IS THE REDUCER'S, including the drag state: whether a file is over the surface, what it is called, which seed is
//! next, the draft under review and its category are all on the model, so the same demonstration can be stepped through by
//! messages alone.

use yew::prelude::*;

use crate::format::format_money;
use crate::model::{Msg, ScannerDraft};
use crate::yew_views::portal_accounting_shell::{AccountingShell, GlassPanel};
use crate::yew_views::portal_shell::PortalShell;

/// The four steps the workflow strip shows, in the order the live screen showed them.
const WORKFLOW_STEPS: [(&str, &str); 4] = [
    ("Receipt image", "Scan or upload"),
    ("AI/OCR extracts", "Vendor · Date · Amount · Category"),
    ("Lisa reviews", "Confirm + add context"),
    ("Create expense", "Saved to Expenses"),
];

/// The categories the review step offers. These are the same nine strings Rust validates against, spelled here because a
/// `<select>` needs its options in the browser.
const EXPENSE_CATEGORIES: [&str; 9] = [
    "Marketing & Advertising",
    "Professional Fees",
    "Office",
    "Insurance",
    "MLS & Memberships",
    "Travel & Entertainment",
    "Merchant / Bank Fees",
    "Property / Deal Expense",
    "Other",
];

/// The read-only field styling the live draft used for what it "extracted".
const READONLY: &str = "mt-1 w-full rounded-md border border-white/15 bg-white/5 px-3 py-2 text-sm text-white";
const LABEL: &str = "block text-[11px] font-medium uppercase tracking-wide text-white/60";

#[derive(Properties, PartialEq)]
pub struct ScannerProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct Scanner;

impl Component for Scanner {
    type Message = ();
    type Properties = ScannerProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("accounting-receipt-scanner")
            .expect("the receipt scanner screen is in the registry");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                <AccountingShell eyebrow="Accounting" title="Receipt Scanner">
                    { self.body(&props.model, &props.on_msg) }
                </AccountingShell>
            </PortalShell>
        }
    }
}

impl Scanner {
    fn body(&self, model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
        html! {
            <div class="space-y-4">
                { self.workflow_strip() }
                { self.drop_surface(model, on_msg) }
                if model.accounting.scanner.draft.is_some() {
                    { self.draft(model, on_msg) }
                }
            </div>
        }
    }

    /// The four steps, with an arrow between them — the strip the live screen opened with.
    fn workflow_strip(&self) -> Html {
        html! {
            <div class="flex flex-wrap items-center gap-2">
                { for WORKFLOW_STEPS.iter().enumerate().map(|(index, (label, note))| html! {
                    <div class="flex items-center gap-2">
                        <div class="rounded-[var(--portal-panel-radius)] border border-white/10 bg-white/[0.05] px-3 py-2">
                            <p class="text-[10px] font-medium uppercase tracking-[0.16em] text-[var(--portal-gold)]">
                                { format!("Step {}", index + 1) }
                            </p>
                            <p class="text-xs text-white/80">{ *label }</p>
                            <p class="text-[10px] text-white/40">{ *note }</p>
                        </div>
                        if index + 1 < WORKFLOW_STEPS.len() {
                            <span class="text-white/25">{"→"}</span>
                        }
                    </div>
                }) }
            </div>
        }
    }

    /// The drop surface.
    ///
    /// THE DRAG STATE IS THE REDUCER'S, and the browser's drag events are only messages. A drop takes the file's NAME and
    /// nothing else: this screen does not read the file, because there is nothing on the other side of that read yet — the
    /// extraction below is the demonstration's, and pretending to inspect an image would be a lie the reader cannot check.
    fn drop_surface(&self, model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
        let dragging = model.accounting.scanner.dragging;
        let file_name = model.accounting.scanner.file_name.clone();
        let on_drag_over = {
            let on_msg = on_msg.clone();
            Callback::from(move |event: DragEvent| {
                event.prevent_default();
                on_msg.emit(Msg::ScannerDragging(true));
            })
        };
        let on_drag_leave = {
            let on_msg = on_msg.clone();
            Callback::from(move |_: DragEvent| on_msg.emit(Msg::ScannerDragging(false)))
        };
        let on_drop = {
            let on_msg = on_msg.clone();
            Callback::from(move |event: DragEvent| {
                event.prevent_default();
                on_msg.emit(Msg::ScannerDragging(false));
                if let Some(name) = event
                    .data_transfer()
                    .and_then(|transfer| transfer.files())
                    .and_then(|files| files.get(0))
                    .map(|file| file.name())
                {
                    on_msg.emit(Msg::ScannerFileChosen(name));
                }
            })
        };
        let scan = {
            let on_msg = on_msg.clone();
            Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ScannerScanned))
        };
        html! {
            <GlassPanel title="Scan / Upload Receipt">
                <div ondragover={on_drag_over} ondragleave={on_drag_leave} ondrop={on_drop}
                    class={classes!("flex", "flex-col", "items-center", "justify-center",
                        "rounded-[var(--portal-panel-radius)]", "border-2", "border-dashed", "px-6", "py-12",
                        "text-center", "transition",
                        if dragging { "border-[var(--portal-gold)] bg-[var(--portal-gold)]/10" }
                        else { "border-white/15 bg-white/[0.02]" })}>
                    <div class="text-3xl">{"🖼️"}</div>
                    <p class="mt-3 text-sm text-white/80">
                        { if file_name.is_empty() { "Drag & drop a receipt image here".to_string() } else { file_name } }
                    </p>
                    <p class="mt-1 max-w-md text-[11px] font-light text-white/45">
                        {"Drop an image to attach it to this demo. For this prototype pass the surface is mostly visual — real \
                          OCR arrives in a later story."}
                    </p>
                    <button type="button" onclick={scan}
                        class="mt-4 rounded-md bg-[var(--portal-gold)] px-5 py-2.5 text-xs font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy-deep)] transition hover:brightness-110">
                        {"Scan Receipt"}
                    </button>
                </div>
                <p class="mt-3 flex items-center gap-1.5 text-[11px] text-amber-300/90">
                    <span>{"•"}</span>
                    {"Prototype preview — extraction is a deterministic demo seed, not OCR."}
                </p>
            </GlassPanel>
        }
    }
}


impl Scanner {
    /// The reviewed draft, and the control that saves it.
    ///
    /// THE VENDOR AND THE AMOUNT ARE READ-ONLY, as the live draft showed them: what the demonstration "extracted" is not the
    /// operator's to retype, it is theirs to accept or to correct at the source. The category is editable, which the live
    /// select was not — its `onChange` did nothing, so a control that looked like a choice was a decoration. A review step
    /// whose point is "confirm the extraction" has to let the one field a reviewer most often changes be changed.
    ///
    /// THE NOTE BESIDE THE BUTTON IS SAVED. It is the receipt's memo, shown here as the context the reviewer is confirming,
    /// and it travels with the vendor, amount, date and category when the expense is recorded — so the row in the book can
    /// say what the receipt was for. Making it editable is a smaller step than it looks (a message and an input) and worth
    /// doing if the workflow strip's "add context" is meant literally; it is not done here because the review step's job in
    /// V1 is to confirm what was read, and the notes in the seeds are already the context.
    fn draft(&self, model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
        let Some(draft) = model.accounting.scanner.draft.clone() else {
            return Html::default();
        };
        let change = {
            let on_msg = on_msg.clone();
            Callback::from(move |event: Event| {
                let select: web_sys::HtmlSelectElement = event.target_unchecked_into();
                on_msg.emit(Msg::ScannerCategoryChanged(select.value()));
            })
        };
        let submit = {
            let on_msg = on_msg.clone();
            Callback::from(move |event: SubmitEvent| {
                event.prevent_default();
                on_msg.emit(Msg::ScannerSubmitted);
            })
        };
        let pending = model.accounting.submitting;
        html! {
            <GlassPanel title="Extracted Draft — Review Before Saving">
                <form onsubmit={submit} class="space-y-3">
                    <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
                        <label class={LABEL}>
                            {"Vendor"}
                            <input readonly={true} value={draft.vendor.clone()} class={READONLY} />
                        </label>
                        <label class={LABEL}>
                            {"Date"}
                            <input readonly={true} type="date" value={draft.expense_on.clone()} class={READONLY} />
                        </label>
                        <label class={LABEL}>
                            {"Amount"}
                            <input readonly={true} value={draft.amount.clone()} class={READONLY} />
                        </label>
                        <label class={LABEL}>
                            {"Category"}
                            <select onchange={change}
                                class="mt-1 w-full rounded-md border border-white/15 bg-[#0b1220] px-3 py-2 text-sm text-white focus:border-[var(--portal-gold)] focus:outline-none">
                                { for EXPENSE_CATEGORIES.iter().map(|category| html! {
                                    <option value={*category} selected={draft.category == *category}>{ *category }</option>
                                }) }
                            </select>
                        </label>
                    </div>
                    <div class="flex flex-wrap items-center gap-3">
                        <button type="submit" disabled={pending}
                            class="rounded-md bg-[var(--portal-gold)] px-4 py-2 text-xs font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy-deep)] transition hover:brightness-110 disabled:opacity-50">
                            { if pending { "Creating…" } else { "Create Expense" } }
                        </button>
                        <span class="text-xs font-medium text-white/70">
                            { format!("{} · {}", draft.memo, format_money(&draft.amount)) }
                        </span>
                        if let Some(notice) = model.accounting.notice.clone() {
                            <span class={classes!("text-xs", if notice.ok { "text-emerald-300" } else { "text-rose-300" })}>
                                { if notice.ok { "Saved to Expenses.".to_string() } else { notice.message } }
                            </span>
                        }
                    </div>
                </form>
            </GlassPanel>
        }
    }
}

