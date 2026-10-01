//! Public, unlisted signer prototype — disconnected Yew/MVI.
//!
//! The eventual route is capability-token driven. For this visual pass the token is accepted only as route context;
//! no service, Vault bytes, identity proof, or persistence is touched.

use yew::prelude::*;

use crate::app::cmd::Cmd;
use crate::app::screen::{Link, Screen, ScreenCtx};

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    signer_name: String,
    initials: String,
    document_title: String,
    property: String,
    consent: bool,
    completed: bool,
    signature_style: usize,
    token_hint: String,
}

impl Default for Model {
    fn default() -> Self {
        let signer_name = "María Rivera".to_string();
        Self {
            initials: initials_for(&signer_name),
            signer_name,
            document_title: "Listing Contract".into(),
            property: "Casa Luar".into(),
            consent: false,
            completed: false,
            signature_style: 0,
            token_hint: "demo".into(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    ConsentChanged(bool),
    SignatureStyle(usize),
    Complete,
    StartOver,
}

pub struct SignDocument;

impl Screen for SignDocument {
    type Model = Model;
    type Msg = Msg;

    fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        let mut model = Model::default();
        model.token_hint = ctx
            .id
            .as_deref()
            .unwrap_or("demo")
            .chars()
            .take(12)
            .collect();
        (model, Cmd::none())
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::ConsentChanged(value) => model.consent = value,
            Msg::SignatureStyle(style) => model.signature_style = style.min(2),
            Msg::Complete if model.consent => model.completed = true,
            Msg::Complete => {}
            Msg::StartOver => {
                model.completed = false;
                model.consent = false;
            }
        }
        Cmd::none()
    }

    fn view(model: &Model, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        if model.completed {
            return completed_view(model, link);
        }

        html! {
            <main class="min-h-screen bg-[#f4f1ea] px-4 py-8 sm:px-6 lg:px-8">
                <div class="mx-auto max-w-6xl">
                    <header class="mb-5 flex flex-wrap items-end justify-between gap-4">
                        <div>
                            <p class="text-[10px] font-medium uppercase tracking-[0.24em] text-[#a88450]">{"CulebraLuxe · Secure Signing"}</p>
                            <h1 class="mt-1 font-serif text-3xl font-light text-[#041024]">{ model.document_title.clone() }</h1>
                            <p class="mt-1 text-sm font-light text-black/45">{ format!("{} · Prepared for {}", model.property, model.signer_name) }</p>
                        </div>
                        <div class="rounded-full border border-black/10 bg-white/70 px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.12em] text-black/40">
                            { format!("Private link · {}", model.token_hint) }
                        </div>
                    </header>

                    <div class="grid gap-5 lg:grid-cols-[minmax(0,1fr)_21rem]">
                        <section class="overflow-hidden rounded-xl border border-black/10 bg-[#dad7d0] shadow-sm">
                            <div class="flex items-center justify-between border-b border-black/10 bg-white/75 px-4 py-2.5">
                                <span class="text-[10px] font-medium uppercase tracking-[0.14em] text-black/45">{"Document preview"}</span>
                                <span class="text-[10px] font-light text-black/35">{"Page 6 of 6 · demo"}</span>
                            </div>
                            <div class="p-3 sm:p-6">
                                { fake_pdf(model) }
                            </div>
                        </section>

                        <aside class="self-start rounded-xl border border-black/10 bg-white p-5 shadow-sm lg:sticky lg:top-6">
                            <p class="text-[10px] font-medium uppercase tracking-[0.16em] text-[#a88450]">{"Your signature"}</p>
                            <h2 class="mt-1 font-serif text-2xl font-light text-[#041024]">{ "Review and sign" }</h2>
                            <p class="mt-2 text-sm font-light leading-6 text-black/50">
                                {"Review the agreement. CulebraLuxe will place your adopted signature, initials and today's signing date in your assigned fields."}
                            </p>

                            <div class="mt-5">
                                <p class="text-[9px] font-medium uppercase tracking-[0.14em] text-black/35">{"Choose appearance"}</p>
                                <div class="mt-2 space-y-2">
                                    { for (0..3).map(|style| signature_choice(model, style, link)) }
                                </div>
                            </div>

                            <div class="mt-5 rounded-lg bg-[#f7f4ed] p-3">
                                <div class="flex items-center justify-between gap-3">
                                    <span class="text-[9px] font-medium uppercase tracking-[0.12em] text-black/35">{"Initials"}</span>
                                    <span class="font-serif text-lg font-semibold italic text-[#041024]">{ model.initials.clone() }</span>
                                </div>
                                <div class="mt-2 flex items-center justify-between gap-3 border-t border-black/5 pt-2">
                                    <span class="text-[9px] font-medium uppercase tracking-[0.12em] text-black/35">{"Signing date"}</span>
                                    <span class="text-xs font-light text-black/60">{"October 1, 2026"}</span>
                                </div>
                            </div>

                            <label class="mt-5 flex cursor-pointer items-start gap-3 text-xs font-light leading-5 text-black/60">
                                <input
                                    type="checkbox"
                                    checked={model.consent}
                                    onchange={link.callback(|event: Event| {
                                        let checked = event
                                            .target_dyn_into::<web_sys::HtmlInputElement>()
                                            .map(|input| input.checked())
                                            .unwrap_or(false);
                                        Msg::ConsentChanged(checked)
                                    })}
                                    class="mt-1 h-4 w-4 accent-[#041024]"
                                />
                                <span>{"I agree to use this electronic signature and intend to sign this document."}</span>
                            </label>

                            <button
                                type="button"
                                disabled={!model.consent}
                                onclick={link.callback(|_: MouseEvent| Msg::Complete)}
                                class="mt-5 flex w-full items-center justify-center rounded-lg bg-[#041024] px-4 py-3 text-[11px] font-medium uppercase tracking-[0.16em] text-white transition hover:bg-[#0a1b38] disabled:cursor-not-allowed disabled:opacity-35"
                            >
                                {"Sign & Complete"}
                            </button>

                            <p class="mt-3 text-center text-[10px] font-light leading-4 text-black/35">
                                {"Disconnected prototype · nothing is being signed or saved yet."}
                            </p>
                        </aside>
                    </div>
                </div>
            </main>
        }
    }
}

fn fake_pdf(model: &Model) -> Html {
    html! {
        <div class="mx-auto min-h-[46rem] max-w-[46rem] bg-white px-[7%] py-[6%] shadow-lg">
            <p class="text-center font-serif text-xl font-semibold tracking-wide text-black/80">{"EXCLUSIVE LISTING AGREEMENT"}</p>
            <p class="mt-2 text-center text-[9px] uppercase tracking-[0.18em] text-black/40">{"CulebraLuxe Real Estate · Culebra, Puerto Rico"}</p>

            <div class="mt-8 space-y-3 text-[11px] font-light leading-5 text-black/65">
                <p>{"This agreement appoints CulebraLuxe Real Estate to market the property known as Casa Luar under the terms shown in the issued agreement."}</p>
                <p>{"The owner confirms the information contained in the agreement and authorizes the brokerage activities described in the preceding sections."}</p>
                <p>{"Electronic signatures and electronically delivered counterparts are intended to have the same effect as signed counterparts for this private agreement."}</p>
            </div>

            <div class="mt-14 border-t border-black/10 pt-6">
                <p class="text-[9px] font-medium uppercase tracking-[0.15em] text-black/35">{"Seller"}</p>
                <div class="mt-4 grid gap-5 sm:grid-cols-[1fr_7rem_8rem]">
                    <div>
                        <div class="flex h-14 items-end rounded-md border-2 border-[#caa36b] bg-[#fffaf0] px-3 pb-1.5">
                            { signature_text(model, "text-xl") }
                        </div>
                        <p class="mt-1 text-[8px] uppercase tracking-[0.12em] text-black/35">{"Signature"}</p>
                    </div>
                    <div>
                        <div class="flex h-14 items-end rounded-md border-2 border-[#caa36b] bg-[#fffaf0] px-3 pb-1.5 font-serif text-lg font-semibold italic text-[#041024]">
                            { model.initials.clone() }
                        </div>
                        <p class="mt-1 text-[8px] uppercase tracking-[0.12em] text-black/35">{"Initials"}</p>
                    </div>
                    <div>
                        <div class="flex h-14 items-end rounded-md border-2 border-[#caa36b] bg-[#fffaf0] px-3 pb-1.5 text-xs text-[#041024]">
                            {"Oct 1, 2026"}
                        </div>
                        <p class="mt-1 text-[8px] uppercase tracking-[0.12em] text-black/35">{"Date"}</p>
                    </div>
                </div>
                <p class="mt-2 text-[10px] font-light text-black/40">{ model.signer_name.clone() }</p>
            </div>

            <div class="mt-16 border-t border-black/10 pt-5">
                <p class="text-[9px] font-medium uppercase tracking-[0.15em] text-black/35">{"Broker"}</p>
                <div class="mt-4 h-10 border-b border-black/30">
                    <span class="font-serif text-lg italic text-black/55">{"Lisa Penfield"}</span>
                </div>
                <p class="mt-1 text-[9px] font-light text-black/35">{"Real Estate Broker License #: C-9931"}</p>
            </div>
        </div>
    }
}

fn signature_choice(model: &Model, style: usize, link: &Link<Msg>) -> Html {
    let onclick = link.callback(move |_: MouseEvent| Msg::SignatureStyle(style));
    html! {
        <button
            type="button"
            {onclick}
            class={classes!(
                "flex", "w-full", "items-center", "justify-between", "rounded-lg", "border", "px-3", "py-2.5", "text-left", "transition",
                if model.signature_style == style {
                    "border-[#caa36b] bg-[#fffaf0]"
                } else {
                    "border-black/10 bg-white hover:border-black/20"
                }
            )}
        >
            { signature_text_for(&model.signer_name, style, "text-lg") }
            if model.signature_style == style {
                <span class="text-[9px] font-medium uppercase tracking-[0.12em] text-[#a88450]">{"Selected"}</span>
            }
        </button>
    }
}

fn signature_text(model: &Model, size: &'static str) -> Html {
    signature_text_for(&model.signer_name, model.signature_style, size)
}

fn signature_text_for(name: &str, style: usize, size: &'static str) -> Html {
    let style_class = match style {
        1 => "font-serif italic tracking-wide",
        2 => "font-serif font-semibold italic",
        _ => "font-serif italic",
    };
    html! {
        <span class={classes!(size, style_class, "text-[#041024]")}>{ name.to_owned() }</span>
    }
}

fn completed_view(model: &Model, link: &Link<Msg>) -> Html {
    html! {
        <main class="flex min-h-screen items-center justify-center bg-[#f4f1ea] px-4 py-12">
            <section class="w-full max-w-xl rounded-xl border border-black/10 bg-white p-8 text-center shadow-sm">
                <div class="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-emerald-50 text-xl text-emerald-700">{"✓"}</div>
                <p class="mt-5 text-[10px] font-medium uppercase tracking-[0.2em] text-[#a88450]">{"CulebraLuxe · Secure Signing"}</p>
                <h1 class="mt-2 font-serif text-3xl font-light text-[#041024]">{"Signing complete"}</h1>
                <p class="mx-auto mt-3 max-w-md text-sm font-light leading-6 text-black/50">
                    { format!("Thank you, {}. In the connected version the completed document will be sealed, recorded and returned to the CulebraLuxe Vault.", model.signer_name) }
                </p>
                <div class="mx-auto mt-6 max-w-sm rounded-lg bg-[#f7f4ed] px-4 py-3 text-left text-xs font-light text-black/55">
                    <div class="flex justify-between gap-4"><span>{"Document"}</span><span class="font-medium text-[#041024]">{ model.document_title.clone() }</span></div>
                    <div class="mt-2 flex justify-between gap-4"><span>{"Signed as"}</span><span class="font-medium text-[#041024]">{ model.signer_name.clone() }</span></div>
                    <div class="mt-2 flex justify-between gap-4"><span>{"Date"}</span><span class="font-medium text-[#041024]">{"October 1, 2026"}</span></div>
                </div>
                <button type="button" onclick={link.callback(|_: MouseEvent| Msg::StartOver)} class="mt-6 text-[10px] font-medium uppercase tracking-[0.14em] text-[#041024] underline decoration-black/20 underline-offset-4">
                    {"Reset demo"}
                </button>
                <p class="mt-3 text-[10px] font-light text-black/30">{"Prototype only · no document was changed."}</p>
            </section>
        </main>
    }
}

fn initials_for(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signer_prototype_has_no_io_and_completes_only_after_consent() {
        let ctx = ScreenCtx {
            id: Some("demo-token".into()),
            ..ScreenCtx::default()
        };
        let (mut model, cmd) = SignDocument::init(&ctx);
        assert!(cmd.into_requests().is_empty(), "the prototype must not call an endpoint");
        assert_eq!(model.initials, "MR");

        SignDocument::update(&mut model, Msg::Complete, &ctx);
        assert!(!model.completed);

        SignDocument::update(&mut model, Msg::ConsentChanged(true), &ctx);
        SignDocument::update(&mut model, Msg::Complete, &ctx);
        assert!(model.completed);
    }
}
