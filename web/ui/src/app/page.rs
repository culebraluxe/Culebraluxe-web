//! The page building block — `PageScreen<T>`, for a portal screen that reads its page and draws it, with no controls
//! that change anything.
//!
//! A read-only screen is a spec: which page it reads (and whether for the record in the URL), which part of the answer
//! is its own, and how that part is drawn. Reading, loading, failure, "the answer did not carry my part" and the back
//! link on a drill-in are this module's, once. A screen that grows commands gets a module of its own.

use std::marker::PhantomData;

use yew::prelude::*;

use crate::app::api::PortalScreenPage;
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::PortalPage;

pub trait PageSpec: 'static {
    /// This screen's part of the portal page.
    type Data: std::fmt::Debug + Clone + PartialEq + 'static;
    /// The `screen=` the page read names.
    const SCREEN: &'static str;
    /// What is being read, for the loading line ("Reading the workflows…").
    const NOUN: &'static str;
    /// Whether the read is about the record in the URL (`/portal/workflows/:instanceId`).
    const SCOPED: bool = false;
    /// This screen's part of the answer. `None` is a failure to say so, not an empty screen.
    fn pick(page: PortalPage) -> Option<Self::Data>;
    fn view(data: &Self::Data, ctx: &ScreenCtx) -> Html;
}

pub struct PageScreen<T: PageSpec>(PhantomData<T>);

#[derive(Debug, Clone, PartialEq)]
pub struct Model<D> {
    pub read: Remote<D>,
}

impl<D> Default for Model<D> {
    fn default() -> Self {
        Self {
            read: Remote::NotAsked,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PortalPage, ApiError>),
}

/// The read a spec asks for, given where the screen was opened.
pub fn read<T: PageSpec>(ctx: &ScreenCtx) -> PortalScreenPage {
    match (T::SCOPED, &ctx.id) {
        (true, Some(id)) => PortalScreenPage::scoped(T::SCREEN, id.clone()),
        _ => PortalScreenPage::of(T::SCREEN),
    }
}

impl<T: PageSpec> Screen for PageScreen<T> {
    type Model = Model<T::Data>;
    type Msg = Msg;

    fn init(ctx: &ScreenCtx) -> (Self::Model, Cmd<Msg>) {
        (
            Model {
                read: Remote::Loading,
            },
            Cmd::request(read::<T>(ctx), Msg::Loaded),
        )
    }

    fn update(model: &mut Self::Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        let Msg::Loaded(answer) = msg;
        model.read = Remote::from_result(answer.and_then(|page| {
            T::pick(page)
                .ok_or_else(|| ApiError::decode(format!("The answer had no {} in it.", T::NOUN)))
        }));
        Cmd::none()
    }

    fn view(model: &Self::Model, ctx: &ScreenCtx, _link: &Link<Msg>) -> Html {
        html! {
            <div class="space-y-6">
                if T::SCOPED {
                    <div>{ template::back_link(ctx) }</div>
                }
                { template::remote(&model.read, T::NOUN, |data| T::view(data, ctx)) }
            </div>
        }
    }
}
