//! `/portal/db-test` — is the database reachable, and does the client table answer. Read-only.
//!
//! THE REFERENCE SCREEN FOR A READ. One endpoint, one `Remote`, the template's loading and failure, and a view of the
//! answer. Nothing here fetches, and nothing here decides how "loading" looks.

use yew::prelude::*;

use crate::app::api::PortalPage;
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template::{self, PANEL};
use crate::model::{PageContent, PortalDbTest, PortalDbTestClient};

pub struct DbTest;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<PortalDbTest>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PageContent, ApiError>),
}

impl Screen for DbTest {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (
            Model {
                read: Remote::Loading,
            },
            Cmd::request(PortalPage::of("db-test"), Msg::Loaded),
        )
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::Loaded(answer) => {
                // The payload is the shared page shape; this screen's answer is one field of it. An answer without that
                // field is a failure to say so, not an empty database.
                model.read = Remote::from_result(answer.and_then(|page| {
                    page.portal
                        .and_then(|portal| portal.support)
                        .and_then(|support| support.db_test)
                        .ok_or_else(|| ApiError::decode("The answer had no database test in it."))
                }));
                Cmd::none()
            }
        }
    }

    fn view(model: &Model, _ctx: &ScreenCtx, _link: &Link<Msg>) -> Html {
        html! {
            <div class="space-y-6">
                { template::portal_heading(
                    "Support",
                    "Database Test",
                    "A read-only check that the portal's database is reachable and that the client table answers. It \
                     writes nothing and changes nothing.",
                ) }
                { template::remote(&model.read, "the database", report) }
            </div>
        }
    }
}

fn report(read: &PortalDbTest) -> Html {
    html! {
        <>
            <section class="grid gap-4 sm:grid-cols-2">
                { template::metric(
                    "Connection",
                    if read.connected { "Connected" } else { "Unavailable" },
                    "The read answered, which is what this checks.",
                ) }
                { template::metric("Clients on record", &read.client_count.to_string(), "Rows returned by the client read.") }
            </section>
            { clients(&read.clients) }
        </>
    }
}

fn clients(clients: &[PortalDbTestClient]) -> Html {
    if clients.is_empty() {
        return template::empty_panel("The client read answered and returned no rows.");
    }
    let dash = |value: &Option<String>| value.clone().unwrap_or_else(|| "\u{2014}".to_string());
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
                                <td class="px-4 py-3 text-black/60">{ dash(&client.email) }</td>
                                <td class="px-4 py-3 text-black/60">{ dash(&client.phone) }</td>
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
    use serde_json::json;

    #[test]
    fn it_asks_for_its_page_and_shows_the_answer_or_the_failure() {
        let ctx = ScreenCtx::default();
        let (mut model, cmd) = DbTest::init(&ctx);
        assert!(model.read.is_loading());
        let request = cmd.into_requests().remove(0);
        assert_eq!(request.path, "/api/portal/rust-ui/page?screen=db-test&");

        let answer = json!({ "portal": { "support": { "dbTest": {
            "connected": true, "clientCount": 1, "clients": [{ "id": "c1", "displayName": "Ada", "role": "buyer", "status": "active" }]
        } } } });
        DbTest::update(&mut model, request.respond(Ok(answer)), &ctx);
        let read = model.read.loaded().expect("loaded");
        assert_eq!(
            (
                read.connected,
                read.client_count,
                read.clients[0].display_name.as_str()
            ),
            (true, 1, "Ada")
        );

        DbTest::update(&mut model, Msg::Loaded(Ok(PageContent::default())), &ctx);
        assert!(
            matches!(model.read, Remote::Failed(ref error) if error.code == "DECODE"),
            "no field is a failure, not an empty database"
        );
    }
}
