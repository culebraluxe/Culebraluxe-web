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
use crate::yew_views::portal_forms::{FormRecord, Forms};
use crate::yew_views::portal_projects::Projects;
use crate::yew_views::portal_seller_strategy::SellerStrategy;
use crate::yew_views::portal_ui_lab::UiLab;
use crate::yew_views::portal_tech::TechCockpit;
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
            "tech" => html! { <TechCockpit model={self.model.clone()} on_msg={on_msg} /> },
            "cabinet" => html! { <Cabinet model={self.model.clone()} on_msg={on_msg} /> },
            "activity" => html! { <Activity model={self.model.clone()} on_msg={on_msg} /> },
            // Accounting V1 — the dashboard first; the remaining four screens are added here as they are written.
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
            "clients" => html! { <Clients model={self.model.clone()} on_msg={on_msg} /> },
            "client-record" => {
                html! { <ClientRecord model={self.model.clone()} on_msg={on_msg} /> }
            }
            "deals" => html! { <Deals model={self.model.clone()} on_msg={on_msg} /> },
            "design-lab" => html! { <UiLab model={self.model.clone()} on_msg={on_msg} /> },
            "deal-record" => {
                html! { <DealRecord model={self.model.clone()} on_msg={on_msg} /> }
            }
            "forms" => html! { <Forms model={self.model.clone()} on_msg={on_msg} /> },
            "form-record" => html! { <FormRecord model={self.model.clone()} on_msg={on_msg} /> },
            "projects" => html! { <Projects model={self.model.clone()} on_msg={on_msg} /> },
            "seller-strategy" => html! { <SellerStrategy model={self.model.clone()} on_msg={on_msg} /> },
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
    if !crate::update::is_ported_portal_screen(screen.key) {
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
