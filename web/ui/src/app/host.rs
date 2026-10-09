//! `ScreenHost<S>` — the one component that runs any `Screen`. Written once; every screen gets it.
//!
//! It holds `S::Model`, calls `S::init` / `S::update`, hands every returned `Cmd` to the executor, and draws `S::view`.
//!
//! STALE ANSWERS ARE DROPPED HERE, ONCE. Every message carries the host's generation. When the same screen is reopened
//! for another record (the context changed), the generation moves on, so an answer to the previous record's request
//! cannot land on the new one. Screens do not guard against this themselves, because they cannot forget to.

use yew::prelude::*;
use yew_router::prelude::RouterScopeExt;

use crate::app::exec;
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::host_logic::{answer_lands, classify_change, CtxView, UrlChange};

/// Mount a screen. The registry stores `mount::<S>` for each screen on the trait.
pub fn mount<S: Screen>(ctx: ScreenCtx) -> Html {
    html! { <ScreenHost<S> ctx={ctx} /> }
}

#[derive(Properties, PartialEq)]
pub struct HostProps {
    pub ctx: ScreenCtx,
}

pub struct ScreenHost<S: Screen> {
    model: S::Model,
    generation: u64,
}

pub struct HostMsg<M> {
    generation: u64,
    msg: M,
}

/// A borrow of the host's pure context view for one `ScreenCtx`. The staleness rule and the change
/// classification live in `crate::host_logic` (dependency-free, tested on the host); this is the only
/// place that adapts the browser-gated context type into that view.
fn view_of(ctx: &ScreenCtx) -> CtxView<'_> {
    CtxView {
        path: &ctx.path,
        id: ctx.id.as_deref(),
        actor: &ctx.actor,
        query: &ctx.query,
        grants: ctx.grants.as_ref(),
    }
}

impl<S: Screen> ScreenHost<S> {
    fn run(&self, ctx: &Context<Self>, cmd: crate::app::cmd::Cmd<S::Msg>) {
        let generation = self.generation;
        let deliver = ctx.link().callback(move |msg| HostMsg { generation, msg });
        exec::run(cmd, &deliver, ctx.link().navigator().as_ref());
    }
}

impl<S: Screen> Component for ScreenHost<S> {
    type Message = HostMsg<S::Msg>;
    type Properties = HostProps;

    fn create(ctx: &Context<Self>) -> Self {
        let (model, cmd) = S::init(&ctx.props().ctx);
        let host = Self {
            model,
            generation: 1,
        };
        host.run(ctx, cmd);
        host
    }

    fn update(&mut self, ctx: &Context<Self>, message: Self::Message) -> bool {
        if !answer_lands(self.generation, message.generation) {
            return false;
        }
        let cmd = S::update(&mut self.model, message.msg, &ctx.props().ctx);
        self.run(ctx, cmd);
        true
    }

    /// The URL changed while this screen is mounted. Another record (path or id) starts the screen over and retires
    /// the old generation, so the previous record's answers are dropped. Only the query changing (a tab, a selection)
    /// keeps the state and asks the screen what to do.
    fn changed(&mut self, ctx: &Context<Self>, old: &Self::Properties) -> bool {
        let (new, old) = (&ctx.props().ctx, &old.ctx);
        let cmd = match classify_change(&view_of(old), &view_of(new)) {
            UrlChange::Same => return false,
            UrlChange::AnotherRecord => {
                self.generation += 1;
                let (model, cmd) = S::init(new);
                self.model = model;
                cmd
            }
            UrlChange::Query => S::url_changed(&mut self.model, new),
            UrlChange::Grants => crate::app::cmd::Cmd::none(),
        };
        self.run(ctx, cmd);
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let generation = self.generation;
        let link = Link::new(ctx.link().callback(move |msg| HostMsg { generation, msg }));
        S::view(&self.model, &ctx.props().ctx, &link)
    }
}
