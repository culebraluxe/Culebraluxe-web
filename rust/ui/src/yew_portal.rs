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
use crate::yew_views::portal_tech::TechCockpit;
use crate::yew_views::portal_ui_lab::UiLab;
use crate::yew_views::portal_workflow_record::WorkflowRecord;
use crate::yew_views::portal_workflows::Workflows;

use std::rc::Rc;

/// A portal body that has no Yew component yet, rendered as markup inside this chrome.
///
/// THE BRIDGE, AND IT IS NOT A SECOND RENDERER. Every portal screen is drawn by the one Yew app in this module: one
/// application, one chrome, one owner of the container. A screen that has not been rewritten as a component yet renders
/// the body `view::render_page` already produces, and Yew treats it as a node it owns and diffs (`VNode::VRaw`), so the
/// body updates on every message exactly as a component would — through the same reducer, from the same model.
///
/// WHAT IT COSTS, stated rather than hidden: `VRaw` replaces its markup when the body changes, so a field inside one of
/// these bodies is rebuilt on a keystroke and its caret is restored by `shell::deliver` rather than by Yew's diffing.
/// That is the string renderer's old limitation, carried one screen at a time — a screen leaves this arm by gaining a
/// component, and nothing else has to change when it does.
#[derive(Properties, PartialEq)]
pub struct StringBodyProps {
    pub html: yew::AttrValue,
}

#[function_component(StringBody)]
fn string_body(props: &StringBodyProps) -> Html {
    Html::from_html_unchecked(props.html.clone())
}

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
        // THE SEAT THE DOCUMENT LISTENERS DELIVER TO, claimed by this app while it owns the container.
        //
        // A portal body rendered as markup has no callbacks of its own — its controls are `data-*` attributes — so the
        // shell's document listeners resolve the intent and `shell::deliver` hands it to whoever owns the page. This is
        // that owner. It is claimed here, by the component that holds the model, so an intent can never reach a model
        // that has been replaced, and it is claimed again on every mount for the same reason.
        let dispatcher: Rc<dyn Fn(Msg)> = {
            let link = ctx.link().clone();
            Rc::new(move |msg: Msg| {
                link.send_message(AppMsg::Ui(msg));
            })
        };
        crate::shell::set_dispatcher(Some(dispatcher));

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
        let body = match self.model.screen.key {
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
            "property-admin" => {
                html! { <OpsWorkbench model={self.model.clone()} on_msg={on_msg} /> }
            }
            "property-media" => {
                html! { <ListingMedia model={self.model.clone()} on_msg={on_msg} /> }
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
            // EVERY OTHER PORTAL SCREEN, drawn by THIS app as markup rather than handed to a second renderer. This arm
            // used to be an error card while the screen was really painted by the string host — the second owner this
            // change removes. See `StringBody`.
            _ => html! {
                <StringBody html={yew::AttrValue::from(crate::view::render_page(&self.model))} />
            },
        };
        // The master shell draws the portal chrome around this (app/chrome.rs).
        body
    }
}
