//! `/contact` — how to reach the office, and the enquiry form, on Yew.
//!
//! THE FORM IS BACK. It posts to `/api/rust-ui/website-intake`, which hands the fields to the same website intake
//! pipeline the React form called, so there is still one way a lead arrives. The fields, the interest chooser, the
//! property context and the sent/failed states are Rust markup driven by the reducer (`ContactFormState`).
//!
//! THE PROPERTY CONTEXT COMES FROM THE LINK. Every "Enquire" and "Book a Private Viewing" builds
//! `/contact?propertyId=...&requestType=private_viewing`; the router scopes the page by the id, the payload names the
//! property (published listings only), and the request type comes from the same query.

use yew::prelude::*;

use wasm_bindgen::JsCast;

use crate::model::{BlockItem, ContactStatus, ContactSubmission, Model, Msg, PageContent};
use crate::yew_router::query_param;
use crate::yew_views::buyers::page_hero;
use crate::yew_views::chrome::PageProps;

pub struct Contact;

impl Component for Contact {
    type Message = ();
    type Properties = PageProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        // No page, no page furniture: a hero with no words over a section with no address is not the Contact page.
        let Some(page) = ctx.props().model.page.as_ref() else {
            return Html::default();
        };
        let intro = page.hero.body.as_str();
        html! {
            <>
                { page_hero(
                    &page.hero.eyebrow,
                    &page.hero.title,
                    (!intro.is_empty()).then_some(intro),
                    page.hero.image_path.as_deref().unwrap_or("/images/coastline.png"),
                    page.hero.image_alt.as_deref().unwrap_or("The Culebra coastline at golden hour"),
                ) }
                <section id="contact" class="bg-primary px-6 py-28 text-primary-foreground md:px-12 md:py-40">
                    <div class="mx-auto grid max-w-[1600px] gap-16 md:grid-cols-12 md:gap-24">
                        <div class="reveal md:col-span-5">
                            <p class="mb-6 text-xs font-light uppercase tracking-[0.34em] text-primary-foreground/50">
                                { page.contact.eyebrow.clone() }
                            </p>
                            <h2 class="text-balance font-serif text-4xl font-light leading-[1.06] md:text-6xl">
                                { page.contact.title.clone() }
                            </h2>
                            { self.details(page) }
                        </div>
                        <div class="reveal md:col-span-7" style="--reveal-step: 1">
                            { enquiry_form(&ctx.props().model, &ctx.props().on_msg) }
                        </div>
                    </div>
                </section>
            </>
        }
    }
}

/// The request a link asked for: a private viewing, property information, or (no property) a general enquiry.
fn request_type(model: &Model) -> &'static str {
    if model
        .scope
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        return "";
    }
    match query_param("requestType").as_deref() {
        Some("property_information") => "property_information",
        _ => "private_viewing",
    }
}

/// The live form's layout and words, with its three states: the form, sending, and the thank-you.
fn enquiry_form(model: &Model, on_msg: &Callback<Msg>) -> Html {
    let state = &model.contact_form;
    let request = request_type(model);
    let property_name = model
        .page
        .as_ref()
        .and_then(|page| page.enquiry_property.clone());
    if state.status == ContactStatus::Sent {
        let note = match request {
            "private_viewing" => "Your private viewing request has been received. A member of the CulebraLuxe team will be in touch within one business day.",
            "property_information" => "Your request for property information has been received. A member of the CulebraLuxe team will respond within one business day.",
            _ => "Your note has reached us. A member of the CulebraLuxe team will respond personally within one business day.",
        };
        return html! {
            <div class="flex h-full min-h-64 flex-col items-start justify-center border-t border-primary-foreground/10 pt-10" role="status">
                <p class="font-serif text-3xl font-light md:text-4xl">{"Thank you."}</p>
                <p class="mt-4 max-w-md text-sm font-light leading-relaxed text-primary-foreground/70">{ note }</p>
            </div>
        };
    }
    let interest = if state.interest.is_empty() {
        "Buying"
    } else {
        state.interest.as_str()
    };
    let sending = state.status == ContactStatus::Sending;
    let onsubmit = {
        let on_msg = on_msg.clone();
        let scope = model.scope.clone().unwrap_or_default();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            let submission = ContactSubmission {
                name: field_value("contact-name"),
                email: field_value("contact-email"),
                message: field_value("contact-message"),
                company: field_value("contact-company"),
                request_type: request.to_string(),
                property_id: if request.is_empty() {
                    String::new()
                } else {
                    scope.trim().to_string()
                },
                service: if request.is_empty() {
                    query_param("service").unwrap_or_default()
                } else {
                    String::new()
                },
            };
            on_msg.emit(Msg::ContactSubmitted {
                submission,
                new_id: random_uuid(),
            });
        })
    };
    let label = "text-xs font-light uppercase tracking-[0.22em] text-primary-foreground/50";
    let input = "border-0 border-b border-primary-foreground/25 bg-transparent pb-3 text-sm font-light text-primary-foreground placeholder:text-primary-foreground/30 focus:border-primary-foreground focus:outline-none";
    html! {
        <>
            if !request.is_empty() {
                <div class="mb-10 border-b border-primary-foreground/10 pb-8">
                    <p class="text-xs font-light uppercase tracking-[0.34em] text-primary-foreground/50">
                        { if request == "private_viewing" { "Private Viewing" } else { "Property Information" } }
                    </p>
                    <p class="mt-3 font-serif text-2xl font-light leading-snug md:text-3xl">
                        { if request == "private_viewing" { "Request a private viewing" } else { "Request property information" } }
                        if let Some(name) = property_name {
                            <span class="mt-1 block text-primary-foreground/70">{ format!("of {name}") }</span>
                        }
                    </p>
                </div>
            }
            <form {onsubmit} class="relative flex flex-col gap-10">
                <div class="absolute -left-[9999px]" aria-hidden="true">
                    <label for="contact-company">{"Company"}</label>
                    <input id="contact-company" name="company" type="text" tabindex="-1" autocomplete="off" />
                </div>
                <div class="grid gap-10 sm:grid-cols-2">
                    <div class="flex flex-col gap-3">
                        <label for="contact-name" class={label}>{"Name"}</label>
                        <input id="contact-name" name="name" type="text" autocomplete="name" required=true class={input} />
                    </div>
                    <div class="flex flex-col gap-3">
                        <label for="contact-email" class={label}>{"Email"}</label>
                        <input id="contact-email" name="email" type="email" autocomplete="email" required=true class={input} />
                    </div>
                </div>
                if request.is_empty() {
                    <fieldset class="flex flex-col gap-4">
                        <legend class={label}>{"I am interested in"}</legend>
                        <div class="mt-4 flex flex-wrap gap-3">
                            { for ["Buying", "Selling", "Both"].into_iter().map(|option| {
                                let on_msg = on_msg.clone();
                                let chosen = option == interest;
                                html! {
                                    <button type="button" aria-pressed={chosen.to_string()}
                                        onclick={Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ContactInterestChosen(option.to_string())))}
                                        class={classes!(
                                            "border", "px-6", "py-2.5", "text-xs", "font-light", "uppercase", "tracking-[0.18em]",
                                            "transition-colors", "duration-300",
                                            if chosen { "border-primary-foreground bg-primary-foreground text-primary" }
                                            else { "border-primary-foreground/25 text-primary-foreground/70 hover:border-primary-foreground/60" }
                                        )}>
                                        { option }
                                    </button>
                                }
                            }) }
                        </div>
                    </fieldset>
                }
                <div class="flex flex-col gap-3">
                    <label for="contact-message" class={label}>{"Message"}</label>
                    <textarea id="contact-message" name="message" rows="4"
                        class={classes!("resize-none", input)}
                        placeholder="Tell us a little about what you are looking for." />
                </div>
                <button type="submit" disabled={sending}
                    class="group mt-2 inline-flex min-h-11 items-center gap-3 self-start px-4 text-xs font-light uppercase tracking-[0.24em] disabled:opacity-60">
                    { if sending { "Sending\u{2026}" } else if request == "private_viewing" { "Request viewing" } else { "Send enquiry" } }
                    <span class="inline-block h-px w-12 bg-primary-foreground transition-all duration-500 group-hover:w-20" aria-hidden="true"></span>
                </button>
                if state.status == ContactStatus::Failed {
                    <p class="text-sm text-primary-foreground/70" role="alert">{"We could not send your note. Please try again."}</p>
                }
            </form>
        </>
    }
}

/// A form control's current value, by id. The fields are uncontrolled, as they were in the live form: the reducer holds
/// the submission's state, not every keystroke.
pub(crate) fn field_value(id: &str) -> String {
    let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|doc| doc.get_element_by_id(id))
    else {
        return String::new();
    };
    if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>() {
        return input.value();
    }
    element
        .dyn_ref::<web_sys::HtmlTextAreaElement>()
        .map(|area| area.value())
        .unwrap_or_default()
}

/// `crypto.randomUUID()` — the submission id the intake pipeline requires. Empty only if the browser has no Web Crypto,
/// which the pipeline then rejects as invalid rather than accepting an id that is not unique.
fn random_uuid() -> String {
    let Some(window) = web_sys::window() else {
        return String::new();
    };
    let crypto = js_sys::Reflect::get(&window, &"crypto".into()).ok();
    crypto
        .and_then(|crypto| {
            let function = js_sys::Reflect::get(&crypto, &"randomUUID".into()).ok()?;
            let function = function.dyn_into::<js_sys::Function>().ok()?;
            function.call0(&crypto).ok()?.as_string()
        })
        .unwrap_or_default()
}

impl Contact {
    /// The office and the email, under the heading in the left column, as the live section had them.
    fn details(&self, page: &PageContent) -> Html {
        // The block's items are typed, not positional: `office` and `email` are the two the live page read, by key. A
        // missing one renders nothing rather than a label with no value under it.
        let item = |key: &str| page.contact.items.iter().find(|item| item.key == key);
        let office = item("office")
            .map(|item| address_block(item, false))
            .into_iter()
            .collect::<Vec<_>>();
        let email = item("email")
            .map(|item| address_block(item, true))
            .into_iter()
            .collect::<Vec<_>>();
        if office.is_empty() && email.is_empty() {
            return Html::default();
        }
        html! {
            <div class="mt-14 flex max-w-xl flex-col gap-8 border-t border-primary-foreground/10 pt-10">
                { for office }
                { for email }
            </div>
        }
    }
}

/// One way to reach the office: its label, and either the value or — for an email — the `mailto:` link it already was.
fn address_block(item: &BlockItem, link: bool) -> Html {
    let label = "text-xs font-light uppercase tracking-[0.2em] text-primary-foreground/45";
    let value = "mt-2 text-sm font-light text-primary-foreground/85";
    let name = item.label.clone().unwrap_or_default();
    let address = item.value.clone().unwrap_or_default();
    html! {
        <div>
            <p class={label}>{ name }</p>
            if link {
                <a href={format!("mailto:{address}")}
                    class={classes!(value, "transition-colors", "hover:text-primary-foreground")}>
                    { address }
                </a>
            } else {
                <p class={value}>{ address }</p>
            }
        </div>
    }
}

/// The words a quick enquiry is framed in, and the note that becomes the lead's message.
pub(crate) struct QuickEnquiry {
    pub eyebrow: &'static str,
    pub title: &'static str,
    pub body: &'static str,
    pub button: &'static str,
    pub sent: &'static str,
    /// What the team reads: the saved properties or searches, spelled out.
    pub message: String,
}

/// A short "leave your email" form that sends a general enquiry through the same intake pipeline as the contact page:
/// a visitor's shortlist or saved searches become a lead the team can answer, with no account needed.
pub(crate) fn quick_enquiry(model: &Model, on_msg: &Callback<Msg>, enquiry: QuickEnquiry) -> Html {
    let state = &model.contact_form;
    if state.status == ContactStatus::Sent {
        return html! {
            <div class="mt-16 border-t border-border pt-10" role="status">
                <p class="font-serif text-2xl font-light text-foreground">{"Thank you."}</p>
                <p class="mt-3 max-w-xl text-sm font-light leading-relaxed text-muted-foreground">{ enquiry.sent }</p>
            </div>
        };
    }
    let sending = state.status == ContactStatus::Sending;
    let onsubmit = {
        let on_msg = on_msg.clone();
        let message = enquiry.message.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            let submission = ContactSubmission {
                name: field_value("quick-name"),
                email: field_value("quick-email"),
                message: message.clone(),
                company: field_value("quick-company"),
                ..Default::default()
            };
            on_msg.emit(Msg::ContactSubmitted {
                submission,
                new_id: random_uuid(),
            });
        })
    };
    let input = "h-12 border-0 border-b border-border bg-transparent text-sm font-light text-foreground placeholder:text-muted-foreground focus:border-foreground focus:outline-none";
    html! {
        <div class="mt-16 grid gap-8 border-t border-border pt-10 md:grid-cols-12 md:gap-12">
            <div class="md:col-span-5">
                <p class="mb-3 text-xs font-light uppercase tracking-[0.34em] text-accent">{ enquiry.eyebrow }</p>
                <h2 class="font-serif text-3xl font-light leading-tight text-foreground">{ enquiry.title }</h2>
                <p class="mt-3 max-w-md text-sm font-light leading-relaxed text-muted-foreground">{ enquiry.body }</p>
            </div>
            <form {onsubmit} class="relative flex flex-col gap-6 md:col-span-7">
                <div class="absolute -left-[9999px]" aria-hidden="true">
                    <label for="quick-company">{"Company"}</label>
                    <input id="quick-company" name="company" type="text" tabindex="-1" autocomplete="off" />
                </div>
                <div class="grid gap-6 sm:grid-cols-2">
                    <label class="flex flex-col gap-2">
                        <span class="text-xs font-light uppercase tracking-[0.22em] text-muted-foreground">{"Name"}</span>
                        <input id="quick-name" name="name" type="text" autocomplete="name" required=true class={input} />
                    </label>
                    <label class="flex flex-col gap-2">
                        <span class="text-xs font-light uppercase tracking-[0.22em] text-muted-foreground">{"Email"}</span>
                        <input id="quick-email" name="email" type="email" autocomplete="email" required=true class={input} />
                    </label>
                </div>
                <button type="submit" disabled={sending}
                    class="group inline-flex min-h-11 items-center gap-3 self-start text-xs font-light uppercase tracking-[0.24em] text-foreground disabled:opacity-60">
                    { if sending { "Sending\u{2026}" } else { enquiry.button } }
                    <span class="inline-block h-px w-12 bg-foreground transition-all duration-500 group-hover:w-20" aria-hidden="true"></span>
                </button>
                if state.status == ContactStatus::Failed {
                    <p class="text-sm text-muted-foreground" role="alert">{"We could not send your note. Please try again."}</p>
                }
            </form>
        </div>
    }
}
