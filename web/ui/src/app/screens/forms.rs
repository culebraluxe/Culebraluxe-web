//! CORE — the mature Forms workspace, ported from the legacy FormEditor to Yew/MVI.
//!
//! The screen owns interaction state only. Reads and writes are typed Endpoint -> Cmd::request
//! effects; all persistence and business operations stay in the Rust Forms, Person, Client,
//! Vault and Signature services behind the portal transport.

use std::collections::BTreeMap;

use yew::prelude::*;

use crate::app::api::{
    FormItem, FormPreview, FormPreviewResponse, FormTemplate, FormTemplateField, FormWhen,
    FormsAction, FormsBridgeResponse, FormsGrok, FormsGrokAnswer, FormsPage, FormsRead, FormsWrite,
    FormsWriteResponse,
};
use crate::app::cmd::{ApiError, Cmd};
use crate::app::screen::{Link, Screen, ScreenCtx};
mod document;
mod editor;
mod fields;
mod rail;
mod update;
#[allow(unused_imports)]
pub(super) use document::*;
#[allow(unused_imports)]
pub(super) use editor::*;
#[allow(unused_imports)]
pub(super) use fields::*;
#[allow(unused_imports)]
pub(super) use rail::*;
#[allow(unused_imports)]
pub(super) use update::*;

const PRIMARY_BUTTON: &str =
    "inline-flex min-h-8 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40";
const GHOST_BUTTON: &str =
    "inline-flex min-h-8 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-40";
const INPUT_CLASS: &str =
    "mt-1 block h-9 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-2.5 text-[13px] font-light leading-9 text-black/70 outline-none focus:border-[var(--portal-navy-soft)]";
const LABEL_CLASS: &str = "text-[9px] font-light uppercase tracking-[0.14em] text-black/40";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    page: Option<FormsPage>,
    values: BTreeMap<String, String>,
    sections: BTreeMap<String, String>,
    saved_values: BTreeMap<String, String>,
    saved_sections: BTreeMap<String, String>,
    details_text: String,
    saved_details_text: String,
    body_edited: bool,
    selected_template: String,
    session_query: String,
    grok_prompt: String,
    /// "Who is this form for?" — open with the template to create, the seller and the catastro typed.
    new_form_template: Option<String>,
    new_seller: String,
    new_catastro: String,
    /// Grok is being asked.
    grok_working: bool,
    /// The mic is listening.
    listening: bool,
    loading: bool,
    busy: bool,
    draft_saving: bool,
    dirty: bool,
    error: Option<String>,
    message: Option<String>,
    preview_uri: Option<String>,
    preview_filename: String,
    preview_loading: bool,
    generation: u32,
    /// The money field being typed in: it shows its raw digits until it loses focus, then reads `$X,XXX,XXX.XX`.
    money_editing: Option<String>,
}

#[derive(Debug)]
pub enum Msg {
    ListLoaded(Result<FormsBridgeResponse, ApiError>),
    RecordLoaded(Result<FormsBridgeResponse, ApiError>),
    OpenForm(String),
    SessionQueryChanged(String),
    TemplateSelected(String),
    NewForm,
    Created(Result<FormsWriteResponse, ApiError>),
    FieldChanged {
        name: String,
        value: String,
    },
    /// A keystroke in a money field: it shows its digits from here until it loses focus. Focus itself changes nothing,
    /// because a re-render on focus drops the selection and the first typed digit lands at the end of the old value.
    MoneyTyped {
        name: String,
        value: String,
    },
    MoneyBlur,
    DetailsChanged(String),
    AutosaveDue(u32),
    DraftSaved {
        generation: u32,
        result: Result<FormsWriteResponse, ApiError>,
    },
    PreviewDue(u32),
    Previewed(Result<FormPreviewResponse, ApiError>),
    SavePdf,
    Issued(Result<FormsWriteResponse, ApiError>),
    FillClient,
    ClientFilled(Result<FormsWriteResponse, ApiError>),
    SendSignature,
    SignatureSent(Result<FormsWriteResponse, ApiError>),
    Share,
    Shared(Result<(), ApiError>),
    Cancel,
    GrokPromptChanged(String),
    GrokGo,
    NewSellerChanged(String),
    NewCatastroChanged(String),
    NewFormCreate,
    NewFormCancel,
    GrokFilled(Result<FormsGrokAnswer, ApiError>),
    MicPressed,
    Heard(Result<String, ApiError>),
}

pub struct Forms;
pub struct FormRecord;

impl Screen for Forms {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (
            Model {
                loading: true,
                ..Model::default()
            },
            Cmd::request(FormsRead::list(None, None, None), Msg::ListLoaded),
        )
    }

    fn update(model: &mut Model, msg: Msg, ctx: &ScreenCtx) -> Cmd<Msg> {
        update(model, msg, ctx)
    }

    fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        view(model, ctx, link)
    }
}

impl Screen for FormRecord {
    type Model = Model;
    type Msg = Msg;

    fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        let Some(id) = ctx.id.clone() else {
            return (
                Model {
                    error: Some("Form id is missing.".into()),
                    ..Model::default()
                },
                Cmd::none(),
            );
        };
        (
            Model {
                loading: true,
                ..Model::default()
            },
            Cmd::request(FormsRead::record(id), Msg::RecordLoaded),
        )
    }

    fn update(model: &mut Model, msg: Msg, ctx: &ScreenCtx) -> Cmd<Msg> {
        update(model, msg, ctx)
    }

    fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        view(model, ctx, link)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_listing_agreement_is_named_by_its_seller_not_the_deals_client() {
        let item = |values: &[(&str, &str)], client: Option<&str>| FormItem {
            field_values: values
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            client_name: client.map(str::to_owned),
            ..FormItem::default()
        };
        assert_eq!(
            party_name(&item(
                &[("sellerName", "Juan A. Santa Cruz")],
                Some("James Lee")
            )),
            "Juan A. Santa Cruz"
        );
        assert_eq!(
            party_name(&item(
                &[("buyerName", "Ana"), ("sellerName", "Luis")],
                Some("James Lee")
            )),
            "Ana / Luis"
        );
        assert_eq!(
            party_name(&item(&[], Some("James Lee"))),
            "James Lee",
            "no names on the form: the linked client"
        );
        assert_eq!(party_name(&item(&[], None)), "Untitled");
    }

    #[test]
    fn a_new_form_is_for_the_seller_typed_never_the_open_forms_deal() {
        // The open form sits on a deal — the way every contract once inherited the demo deal "Sunset Point".
        let mut page = FormsPage::default();
        page.selected = Some(FormItem {
            id: "open".into(),
            template_id: "LISTING-01".into(),
            deal_id: Some("60000000-0000-4000-8000-000000000003".into()),
            person_id: Some("someone-else".into()),
            ..FormItem::default()
        });
        let mut model = Model {
            page: Some(page),
            ..Model::default()
        };
        let ctx = ScreenCtx::default();

        assert!(
            Forms::update(&mut model, Msg::NewForm, &ctx)
                .into_requests()
                .is_empty(),
            "New asks first"
        );
        assert!(
            Forms::update(&mut model, Msg::NewFormCreate, &ctx)
                .into_requests()
                .is_empty(),
            "nobody named yet"
        );

        Forms::update(
            &mut model,
            Msg::NewSellerChanged(" Julio Pimentel Ortiz ".into()),
            &ctx,
        );
        let request = Forms::update(&mut model, Msg::NewFormCreate, &ctx)
            .into_requests()
            .remove(0);
        let body = request.body.expect("a create body");
        assert_eq!(body["action"], "create");
        assert_eq!(body["sellerName"], "Julio Pimentel Ortiz");
        assert!(body["dealId"].is_null(), "no deal is inherited: {body}");
        assert!(body["personId"].is_null(), "no client is inherited: {body}");
    }

    #[test]
    fn forms_landing_asks_only_for_the_typed_forms_read() {
        let (_, cmd) = Forms::init(&ScreenCtx::default());
        let requests = cmd.into_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0]
            .path
            .starts_with("/api/portal/rust-ui/forms?screen=forms"));
    }

    #[test]
    fn preferred_form_matches_the_legacy_listing_rule() {
        let mut page = FormsPage::default();
        page.items = vec![
            FormItem {
                id: "issued".into(),
                template_id: "LISTING-01".into(),
                template_version: 4,
                active_version: 4,
                status: "issued".into(),
                ..FormItem::default()
            },
            FormItem {
                id: "draft".into(),
                template_id: "LISTING-01".into(),
                template_version: 4,
                active_version: 4,
                status: "draft".into(),
                ..FormItem::default()
            },
        ];
        assert_eq!(preferred_form_id(&page).as_deref(), Some("draft"));
    }

    #[test]
    fn editing_a_field_is_model_only_and_schedules_preview_and_autosave() {
        let mut model = Model::default();
        let cmd = update(
            &mut model,
            Msg::FieldChanged {
                name: "sellerCivilStatus".into(),
                value: "Married".into(),
            },
            &ScreenCtx::default(),
        );
        assert_eq!(
            model.values.get("sellerCivilStatus").map(String::as_str),
            Some("Married")
        );
        assert!(model.dirty);
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn switching_saved_forms_reuses_the_mounted_screen_instead_of_navigating() {
        let mut model = Model::default();
        let cmd = update(
            &mut model,
            Msg::OpenForm("form-next".into()),
            &ScreenCtx::default(),
        );
        let requests = cmd.into_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0]
            .path
            .contains("screen=form-record&scope=form-next"));
    }

    #[test]
    fn share_is_an_executor_effect_not_a_pdf_navigation() {
        let mut model = Model {
            preview_uri: Some("data:application/pdf;base64,JVBERi0=".into()),
            preview_filename: "Agreement.pdf".into(),
            ..Model::default()
        };
        let cmd = update(&mut model, Msg::Share, &ScreenCtx::default());
        assert!(matches!(cmd, Cmd::SharePdf { .. }));
    }

    #[test]
    fn money_is_shown_with_grouping_without_changing_the_saved_value() {
        assert_eq!(
            model::forms_format::format_money("2100000"),
            "$2,100,000.00"
        );
        assert_eq!(model::forms_format::format_money("425000.5"), "$425,000.50");
    }
}
