//! OPPS Data Workbench: one selector/editor shell over typed Rust domain adapters.
//! The renderer is generic; each entity supplies field and section definitions.

use yew::prelude::*;

use crate::model::{
    PortalOpsMediaAsset, PortalOpsPerson, PortalOpsProject, PortalOpsProperty,
    PortalOpsWorkbenchPage,
};

use super::{Msg, Vm};
mod property_fields;
mod listing_fields;
mod selector;
mod editor;
mod property_people;
mod media;
mod records;
mod fields_ui;
#[allow(unused_imports)]
pub(super) use property_fields::*;
#[allow(unused_imports)]
pub(super) use listing_fields::*;
#[allow(unused_imports)]
pub(super) use selector::*;
#[allow(unused_imports)]
pub(super) use editor::*;
#[allow(unused_imports)]
pub(super) use property_people::*;
#[allow(unused_imports)]
pub(super) use media::*;
#[allow(unused_imports)]
pub(super) use records::*;
#[allow(unused_imports)]
pub(super) use fields_ui::*;


fn payload<'a>(model: &Vm<'a>) -> Option<&'a PortalOpsWorkbenchPage> {
    Some(model.data)
}

fn value(model: &Vm<'_>, key: &str) -> String {
    model.ops.form.get(key).cloned().unwrap_or_default()
}

pub(super) fn workbench(model: &Vm<'_>, on_msg: &Callback<Msg>) -> Html {
    let data = payload(model);
    let total = data.map(|data| data.total).unwrap_or(0);
    let current = data.map(|data| data.page).unwrap_or(1);
    let page_size = data.map(|data| data.page_size.max(1)).unwrap_or(50);
    let pages = ((total + page_size - 1) / page_size).max(1);
    let write_action = match model.ops.entity.as_str() {
        "person" => "person.write",
        "project" => "project.write",
        _ => "property.write",
    };
    let rail_class = if model.ops.rail_collapsed {
        "grid min-h-0 gap-3 md:h-[calc(100dvh-8.5rem)] md:grid-cols-[56px_minmax(0,1fr)]"
    } else {
        "grid min-h-0 gap-4 md:h-[calc(100dvh-8.5rem)] md:grid-cols-[240px_minmax(0,1fr)]"
    };

    html! {
        <div class="space-y-3">
            { entity_switcher(model, on_msg) }
            <div class={rail_class}>
                { selector_rail(model, on_msg, total, current, pages) }
                <main class="min-h-0 overflow-hidden">
                    <fieldset disabled={!model.can(write_action)} class="h-full min-h-0 min-w-0">
                        { editor(model, on_msg) }
                    </fieldset>
                </main>
            </div>
        </div>
    }
}
