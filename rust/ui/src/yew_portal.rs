//! The Portal application, on Yew.
//!
//! Next owns portal URLs. It passes the registry screen and optional record scope;
//! the reducer still owns opening the screen and choosing the effect.

use yew::prelude::*;

use crate::model::{Msg, Screen};
use crate::yew_views::portal_accounting_dashboard::Dashboard as AccountingDashboard;
use crate::yew_views::portal_accounting_expenses::Expenses as AccountingExpenses;
use crate::yew_views::portal_accounting_pnl::Pnl as AccountingPnl;
use crate::yew_views::portal_accounting_receipt_scanner::Scanner as AccountingReceiptScanner;
use crate::yew_views::portal_accounting_receivables::Receivables as AccountingReceivables;
use crate::yew_views::portal_activity::Activity;
use crate::yew_views::portal_cabinet::Cabinet;
use crate::yew_views::portal_clients::{ClientRecord, Clients};
use crate::yew_views::portal_cockpit::Cockpit;
use crate::yew_views::portal_deals::{DealRecord, Deals};
use crate::yew_views::portal_flight_recorder::FlightRecorder;
use crate::yew_views::portal_flight_recorder_list::FlightRecorderList;
use crate::yew_views::portal_forms::{FormRecord, Forms};
use crate::yew_views::portal_listing_media::ListingMedia;
use crate::yew_views::portal_ops::OpsWorkbench;
use crate::yew_views::portal_projects::Projects;
use crate::yew_views::portal_seller_strategy::SellerStrategy;
use crate::yew_views::portal_storyboard::Storyboard;
use crate::yew_views::portal_support_db_test::DbTest as SupportDbTest;
use crate::yew_views::portal_support_security::Security as SupportSecurity;
use crate::yew_views::portal_support_system_health::SystemHealth as SupportSystemHealth;
use crate::yew_views::portal_support_users::SecurityUsers as SupportSecurityUsers;
use crate::yew_views::portal_support_whatsapp_meta::WhatsAppMeta as SupportWhatsAppMeta;
use crate::yew_views::portal_tech::TechCockpit;
use crate::yew_views::portal_ui_lab::UiLab;
use crate::yew_views::portal_workflow_record::WorkflowRecord;
use crate::yew_views::portal_workflows::Workflows;

pub enum AppMsg {
    Ui(Msg),
}

#[derive(Properties, PartialEq, Clone)]
pub struct PortalAppProps {
    pub screen: Screen,
    pub scope: Option<String>,
}

pub struct PortalApp {
    model: crate::model::Model,
}

impl Component for PortalApp {
    type Message = AppMsg;
    type Properties = PortalAppProps;

    fn create(ctx: &Context<Self>) -> Self {
        let mut model = crate::model::Model::default();
        let effects = crate::update::update(
            &mut model,
            Msg::MountScoped {
                screen: ctx.props().screen,
                scope: ctx.props().scope.clone(),
                generation: 1,
            },
        );
        for effect in effects {
            crate::yew_effects::run(effect, &ctx.link().callback(AppMsg::Ui));
        }
        Self { model }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let AppMsg::Ui(msg) = msg;
        let effects = crate::update::update(&mut self.model, msg);
        for effect in effects {
            crate::yew_effects::run(effect, &ctx.link().callback(AppMsg::Ui));
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let on_msg = ctx.link().callback(AppMsg::Ui);
        match self.model.screen.key {
            "dashboard" => html! { <Cockpit model={self.model.clone()} on_msg={on_msg.clone()} /> },
            "tech" => html! { <TechCockpit model={self.model.clone()} on_msg={on_msg.clone()} /> },
            "trace-record" => {
                html! { <FlightRecorder model={self.model.clone()} on_msg={on_msg.clone()} /> }
            }
            "tech-flight-recorder" => {
                html! { <FlightRecorderList model={self.model.clone()} on_msg={on_msg} /> }
            }
            "cabinet" => html! { <Cabinet model={self.model.clone()} on_msg={on_msg} /> },
            "activity" => html! { <Activity model={self.model.clone()} on_msg={on_msg} /> },
            "accounting" => {
                html! { <AccountingDashboard model={self.model.clone()} on_msg={on_msg} /> }
            }
            "accounting-expenses" => {
                html! { <AccountingExpenses model={self.model.clone()} on_msg={on_msg} /> }
            }
            "accounting-receivables" => {
                html! { <AccountingReceivables model={self.model.clone()} on_msg={on_msg} /> }
            }
            "accounting-pnl" => {
                html! { <AccountingPnl model={self.model.clone()} on_msg={on_msg} /> }
            }
            "accounting-receipt-scanner" => {
                html! { <AccountingReceiptScanner model={self.model.clone()} on_msg={on_msg} /> }
            }
            // SUPPORT — a bespoke component per screen, never the generic renderer.
            "db-test" => html! { <SupportDbTest model={self.model.clone()} on_msg={on_msg} /> },
            "security" => html! { <SupportSecurity model={self.model.clone()} on_msg={on_msg} /> },
            "settings-users" => {
                html! { <SupportSecurityUsers model={self.model.clone()} on_msg={on_msg} /> }
            }
            "whatsapp-meta" => {
                html! { <SupportWhatsAppMeta model={self.model.clone()} on_msg={on_msg} /> }
            }
            "system-health" => {
                html! { <SupportSystemHealth model={self.model.clone()} on_msg={on_msg} /> }
            }
            "property-admin" => {
                html! { <OpsWorkbench model={self.model.clone()} on_msg={on_msg} /> }
            }
            "property-media" => {
                html! { <ListingMedia model={self.model.clone()} on_msg={on_msg} /> }
            }
            "clients" => html! { <Clients model={self.model.clone()} on_msg={on_msg} /> },
            "client-record" => {
                html! { <ClientRecord model={self.model.clone()} on_msg={on_msg} /> }
            }
            "deals" => html! { <Deals model={self.model.clone()} on_msg={on_msg} /> },
            "design-lab" => html! { <UiLab model={self.model.clone()} on_msg={on_msg} /> },
            "storyboard" => html! { <Storyboard model={self.model.clone()} on_msg={on_msg} /> },
            "deal-record" => {
                html! { <DealRecord model={self.model.clone()} on_msg={on_msg} /> }
            }
            "forms" => html! { <Forms model={self.model.clone()} on_msg={on_msg} /> },
            "form-record" => html! { <FormRecord model={self.model.clone()} on_msg={on_msg} /> },
            "projects" => html! { <Projects model={self.model.clone()} on_msg={on_msg} /> },
            "seller-strategy" => {
                html! { <SellerStrategy model={self.model.clone()} on_msg={on_msg} /> }
            }
            "workflows" => html! { <Workflows model={self.model.clone()} on_msg={on_msg} /> },
            "workflow-record" => {
                html! { <WorkflowRecord model={self.model.clone()} on_msg={on_msg} /> }
            }
            other => html! {
                <div class="p-6 text-sm text-destructive" role="alert">
                    { format!("Portal Yew screen '{other}' has no component.") }
                </div>
            },
        }
    }
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn portal_mount(
    element_id: &str,
    screen_key: &str,
    scope: &str,
) -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let screen = crate::model::screen(screen_key).ok_or_else(|| {
        wasm_bindgen::JsValue::from_str(&format!("ui: '{screen_key}' is not a known screen"))
    })?;
    if !crate::update::has_yew_portal_component(screen.key) {
        return Err(wasm_bindgen::JsValue::from_str(&format!(
            "ui: '{screen_key}' has no Yew component yet"
        )));
    }
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("ui: no document"))?;
    let root = document.get_element_by_id(element_id).ok_or_else(|| {
        wasm_bindgen::JsValue::from_str(&format!("ui: no element '{element_id}'"))
    })?;
    let scope = if scope.trim().is_empty() {
        None
    } else {
        Some(scope.to_string())
    };
    yew::Renderer::<PortalApp>::with_root_and_props(root, PortalAppProps { screen, scope })
        .render();
    Ok(())
}
