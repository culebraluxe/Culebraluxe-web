//! The rows building block — `RowsScreen<T>`, for a screen that is a titled table of facts and nothing more.
//!
//! A table screen is a spec: its heading, its column labels, what "empty" means, and which rows read feeds it. The
//! reading, the loading and failure states and the table are this module's, once. A screen that grows controls stops
//! being a rows screen and gets a module of its own.

use std::marker::PhantomData;

use yew::prelude::*;

use crate::app::api::RowsRead;
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template::{self, PANEL};
use crate::model::Row;

pub trait RowsSpec: 'static {
    const EYEBROW: &'static str;
    const TITLE: &'static str;
    const PURPOSE: &'static str;
    /// One label per cell, in order.
    const COLUMNS: &'static [&'static str];
    /// What an answered read with no rows means, in words.
    const EMPTY: &'static str;
    fn read() -> RowsRead;
}

pub struct RowsScreen<T: RowsSpec>(PhantomData<T>);

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub rows: Remote<Vec<Row>>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<Vec<Row>, ApiError>),
    Reload,
}

impl<T: RowsSpec> Screen for RowsScreen<T> {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (
            Model {
                rows: Remote::Loading,
            },
            Cmd::request(T::read(), Msg::Loaded),
        )
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::Reload => {
                model.rows = Remote::Loading;
                Cmd::request(T::read(), Msg::Loaded)
            }
            Msg::Loaded(answer) => {
                model.rows = Remote::from_result(answer);
                Cmd::none()
            }
        }
    }

    fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        let retry = link.callback(|_: MouseEvent| Msg::Reload);
        html! {
            <div class="space-y-6">
                <div>{ template::back_link(ctx) }</div>
                { template::portal_heading(T::EYEBROW, T::TITLE, T::PURPOSE) }
                { template::remote_retry(&model.rows, "the records", retry, |rows| table::<T>(rows)) }
            </div>
        }
    }
}

fn table<T: RowsSpec>(rows: &[Row]) -> Html {
    if rows.is_empty() {
        return template::empty_panel(T::EMPTY);
    }
    html! {
        <section class={classes!(PANEL, "overflow-hidden")}>
            <div class="overflow-x-auto">
                <table class="w-full text-left text-sm">
                    <thead>
                        <tr class="border-b border-[var(--portal-border)] text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                            { for T::COLUMNS.iter().map(|label| html! { <th class="px-4 py-3">{ *label }</th> }) }
                        </tr>
                    </thead>
                    <tbody>
                        { for rows.iter().map(|row| html! {
                            <tr class="border-b border-[var(--portal-border)] last:border-b-0">
                                { for row.cells.iter().enumerate().map(|(index, cell)| html! {
                                    <td class={if index == 0 { "px-4 py-3 font-serif text-base font-light" } else { "px-4 py-3 text-black/60" }}>
                                        { cell.clone() }
                                    </td>
                                }) }
                            </tr>
                        }) }
                    </tbody>
                </table>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Spec;

    impl RowsSpec for Spec {
        const EYEBROW: &'static str = "Test";
        const TITLE: &'static str = "Test rows";
        const PURPOSE: &'static str = "Proves reload re-issues the read.";
        const COLUMNS: &'static [&'static str] = &["Name"];
        const EMPTY: &'static str = "No rows.";
        fn read() -> RowsRead {
            RowsRead::portal("test-rows")
        }
    }

    #[test]
    fn reload_reissues_the_read_after_a_failure() {
        let ctx = ScreenCtx::default();
        let (mut model, _) = RowsScreen::<Spec>::init(&ctx);
        RowsScreen::<Spec>::update(
            &mut model,
            Msg::Loaded(Err(ApiError::network("down"))),
            &ctx,
        );
        assert!(
            matches!(model.rows, Remote::Failed(_)),
            "the failure must be the state reload recovers from"
        );
        let cmd = RowsScreen::<Spec>::update(&mut model, Msg::Reload, &ctx);
        assert!(
            matches!(model.rows, Remote::Loading),
            "reload returns to loading"
        );
        assert_eq!(
            cmd.into_requests().len(),
            1,
            "reload re-issues the one read"
        );
    }
}
