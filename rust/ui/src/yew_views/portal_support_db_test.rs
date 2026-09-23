//! `/portal/db-test` — the database diagnostic, as a screen rather than a JSON dump.
//!
//! WHAT THE PRE-CUTOVER PAGE WAS: a `<main>` with a heading and a `<pre>` holding
//! `JSON.stringify({ connected: true, clientCount, clients }, null, 2)`. It answered its question — is the database
//! reachable, and does the client table have rows — by printing the raw records, including every client's budget,
//! preferences and interests.
//!
//! WHAT IT IS NOW: the same three answers, presented. Connected or not, how many clients, and who they are, in a table of
//! the identity columns a diagnostic needs. Nothing about a client that a diagnostic does not need is on this screen, and
//! nothing on it is invented.
//!
//! NO CONTROLS. It reads, it reports, and it changes nothing: a diagnostic that can write is a diagnostic that can be the
//! incident.

use yew::prelude::*;

use crate::model::{Msg, PortalDbTest, PortalDbTestClient};
use crate::yew_views::portal_shell::PortalShell;

/// The light surface the portal's read-only pages use, since this screen is not one of Accounting's navy panels.
const PANEL: &str = "portal-glass-panel portal-glass-panel-soft rounded-[var(--portal-panel-radius)]";

#[derive(Properties, PartialEq)]
pub struct DbTestProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct DbTest;

impl Component for DbTest {
    type Message = ();
    type Properties = DbTestProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("db-test").expect("the DB Test screen is in the registry");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { self.body(&props.model) }
            </PortalShell>
        }
    }
}

impl DbTest {
    fn body(&self, model: &crate::model::Model) -> Html {
        let read = model
            .page
            .as_ref()
            .and_then(|page| page.portal.as_ref())
            .and_then(|portal| portal.support.as_ref())
            .and_then(|support| support.db_test.clone());
        html! {
            <div class="space-y-6">
                <div>
                    <p class="text-xs font-light uppercase tracking-[0.28em] text-black/40">{"Support"}</p>
                    <h1 class="mt-3 font-serif text-4xl font-light leading-[1.1]">{"Database Test"}</h1>
                    <p class="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
                        {"A read-only check that the portal's database is reachable and that the client table answers. It \
                          writes nothing and changes nothing."}
                    </p>
                </div>
                { self.report(read) }
            </div>
        }
    }
}

impl DbTest {
    /// The three answers, or the explicit statement that there is not one yet.
    ///
    /// A read that has not arrived is NOT the same as a database with no clients, and this says which of the two it is: the
    /// screen is a diagnostic, and conflating "nothing yet" with "nothing there" is the one thing it must never do.
    fn report(&self, read: Option<PortalDbTest>) -> Html {
        let Some(read) = read else {
            return html! {
                <section class={classes!(PANEL, "p-10", "text-center")}>
                    <p class="text-sm font-light text-black/40">{"Reading the database…"}</p>
                </section>
            };
        };
        html! {
            <>
                <section class="grid gap-4 sm:grid-cols-2">
                    { self.metric(
                        "Connection",
                        if read.connected { "Connected" } else { "Unavailable" },
                        "The read answered, which is what this checks.",
                    ) }
                    { self.metric(
                        "Clients on record",
                        &read.client_count.to_string(),
                        "Rows returned by the client read.",
                    ) }
                </section>
                { self.clients(&read.clients) }
            </>
        }
    }

    fn metric(&self, label: &str, value: &str, hint: &str) -> Html {
        html! {
            <div class={classes!(PANEL, "p-6")}>
                <p class="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">{ label }</p>
                <p class="mt-2 font-serif text-3xl font-light text-[var(--portal-navy)]">{ value }</p>
                <p class="mt-1 text-[11px] font-light text-black/40">{ hint }</p>
            </div>
        }
    }

    /// The clients themselves: who they are and how to reach them. An empty answer is reported as empty, with the count
    /// above it already saying zero, so the two cannot disagree.
    fn clients(&self, clients: &[PortalDbTestClient]) -> Html {
        if clients.is_empty() {
            return html! {
                <section class={classes!(PANEL, "p-10", "text-center")}>
                    <p class="text-sm font-light text-black/40">
                        {"The client read answered and returned no rows."}
                    </p>
                </section>
            };
        }
        html! {
            <section class={classes!(PANEL, "overflow-hidden")}>
                <div class="overflow-x-auto">
                    <table class="w-full min-w-[720px] text-left text-sm">
                        <thead>
                            <tr class="border-b border-[var(--portal-border)] text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                                <th class="px-4 py-3">{"Client"}</th>
                                <th class="px-4 py-3">{"Role"}</th>
                                <th class="px-4 py-3">{"Status"}</th>
                                <th class="px-4 py-3">{"Email"}</th>
                                <th class="px-4 py-3">{"Phone"}</th>
                            </tr>
                        </thead>
                        <tbody>
                            { for clients.iter().map(|client| html! {
                                <tr class="border-b border-[var(--portal-border)] last:border-b-0">
                                    <td class="px-4 py-3 font-serif text-base font-light">{ client.display_name.clone() }</td>
                                    <td class="px-4 py-3 text-black/60">{ client.role.clone() }</td>
                                    <td class="px-4 py-3 text-black/60">{ client.status.clone() }</td>
                                    <td class="px-4 py-3 text-black/60">
                                        { client.email.clone().unwrap_or_else(|| "—".to_string()) }
                                    </td>
                                    <td class="px-4 py-3 text-black/60">
                                        { client.phone.clone().unwrap_or_else(|| "—".to_string()) }
                                    </td>
                                </tr>
                            }) }
                        </tbody>
                    </table>
                </div>
            </section>
        }
    }
}

