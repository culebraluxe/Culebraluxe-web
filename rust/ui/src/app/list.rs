//! The list building block — the "ListScreen" behaviour every list screen composes rather than re-implements.
//!
//! A list screen holds a `ListState` and wraps its messages (`Msg::List(ListMsg)`). `ListState::update` handles typing
//! (with a pause before searching), paging, and says when the screen should reload. The screen owns only what is its
//! own: which endpoint to call and how a row looks. The selected row lives in the URL (`?selected=`), so it can be
//! linked, reloaded and gone back to.

use yew::prelude::*;

use crate::app::cmd::Cmd;
use crate::app::template;

/// How long typing must pause before the search runs.
pub const SEARCH_PAUSE_MS: u32 = 300;

#[derive(Debug, Clone, PartialEq)]
pub struct ListState {
    pub search: String,
    /// 1-based.
    pub page: i64,
    /// Bumped on every keystroke; a pause timer carries the value it was started with, so only the last one counts.
    typing: u64,
}

impl Default for ListState {
    fn default() -> Self {
        Self {
            search: String::new(),
            page: 1,
            typing: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ListMsg {
    Typed(String),
    /// Typing paused; the token says which keystroke started this timer.
    Paused(u64),
    /// Move by this many pages.
    Page(i64),
}

/// What the screen should do after a list message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListChange {
    Nothing,
    /// The search or page changed: read the list again.
    Reload,
}

impl ListState {
    pub fn update(&mut self, msg: ListMsg, pages: i64) -> (ListChange, Cmd<ListMsg>) {
        match msg {
            ListMsg::Typed(text) => {
                self.search = text;
                self.typing += 1;
                (
                    ListChange::Nothing,
                    Cmd::after(SEARCH_PAUSE_MS, ListMsg::Paused(self.typing)),
                )
            }
            ListMsg::Paused(token) if token == self.typing => {
                self.page = 1;
                (ListChange::Reload, Cmd::none())
            }
            ListMsg::Paused(_) => (ListChange::Nothing, Cmd::none()),
            ListMsg::Page(delta) => {
                let next = (self.page + delta).clamp(1, pages.max(1));
                if next == self.page {
                    return (ListChange::Nothing, Cmd::none());
                }
                self.page = next;
                (ListChange::Reload, Cmd::none())
            }
        }
    }
}

/// Pages needed for `total` rows at `page_size`, never fewer than one.
pub fn pages(total: i64, page_size: i64) -> i64 {
    let size = page_size.max(1);
    ((total + size - 1) / size).max(1)
}

/// The standard list column: a count, a search box, the rows, and paging.
pub fn rail(
    title: &str,
    total: i64,
    state: &ListState,
    pages: i64,
    loading: bool,
    empty: &str,
    rows: Html,
    has_rows: bool,
    on: &Callback<ListMsg>,
) -> Html {
    let oninput = {
        let on = on.clone();
        Callback::from(move |event: InputEvent| {
            on.emit(ListMsg::Typed(template::input_value(&event)))
        })
    };
    let step = |delta: i64| {
        let on = on.clone();
        Callback::from(move |_: MouseEvent| on.emit(ListMsg::Page(delta)))
    };
    const PAGER: &str = "text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30";
    html! {
        <aside class="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]" data-list-rail="">
            <div class="shrink-0 border-b border-[var(--portal-panel-border)] p-2.5">
                <div class="mb-2 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                    { format!("{title} \u{b7} {total}") }
                </div>
                <input type="search" oninput={oninput} value={state.search.clone()} placeholder="Search\u{2026}"
                    class="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-2.5 py-1.5 text-sm font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]" />
            </div>
            <div class="min-h-0 flex-1 overflow-y-auto">
                if has_rows {
                    { rows }
                } else {
                    <p class="px-3 py-6 text-sm font-light text-black/40">{ if loading { "Loading\u{2026}" } else { empty } }</p>
                }
            </div>
            <div class="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--portal-panel-border)] px-2 py-1.5">
                <button type="button" onclick={step(-1)} disabled={state.page <= 1} class={PAGER}>{"\u{2190} Prev"}</button>
                <span class="text-[10px] font-light text-black/40">{ format!("{} / {pages}", state.page) }</span>
                <button type="button" onclick={step(1)} disabled={state.page >= pages} class={PAGER}>{"Next \u{2192}"}</button>
            </div>
        </aside>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_last_pause_searches_and_it_returns_to_page_one() {
        let mut list = ListState {
            page: 3,
            ..ListState::default()
        };
        let (change, first) = list.update(ListMsg::Typed("al".into()), 9);
        assert_eq!(change, ListChange::Nothing);
        assert!(matches!(
            first,
            Cmd::After {
                msg: ListMsg::Paused(1),
                ..
            }
        ));
        list.update(ListMsg::Typed("ale".into()), 9);
        assert_eq!(
            list.update(ListMsg::Paused(1), 9).0,
            ListChange::Nothing,
            "a superseded pause does nothing"
        );
        assert_eq!(list.update(ListMsg::Paused(2), 9).0, ListChange::Reload);
        assert_eq!((list.search.as_str(), list.page), ("ale", 1));
    }

    #[test]
    fn paging_stays_inside_the_pages_and_only_reloads_on_a_move() {
        let mut list = ListState::default();
        assert_eq!(
            list.update(ListMsg::Page(-1), 3).0,
            ListChange::Nothing,
            "already on the first page"
        );
        assert_eq!(list.update(ListMsg::Page(1), 3).0, ListChange::Reload);
        list.update(ListMsg::Page(5), 3);
        assert_eq!(list.page, 3, "clamped to the last page");
        assert_eq!(pages(0, 50), 1);
        assert_eq!(pages(101, 50), 3);
    }
}
