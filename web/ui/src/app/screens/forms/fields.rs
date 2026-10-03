//! One template field as a control, and its grid span.

#[allow(unused_imports)]
use super::*;

pub(super) fn field_control(
    field: &FormTemplateField,
    values: &BTreeMap<String, String>,
    link: &Link<Msg>,
) -> Html {
    let value = values.get(&field.name).cloned().unwrap_or_default();
    let label = if field.required {
        format!("{} *", field.label)
    } else {
        field.label.clone()
    };
    let span = field_span_class(field);

    let control = match field.field_type.as_str() {
        "textarea" => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| Msg::FieldChanged {
                name: name.clone(),
                value: crate::app::exec::textarea_value(&event),
            });
            html! {
                <textarea
                    rows={2}
                    value={value}
                    oninput={changed}
                    class={format!("{INPUT_CLASS} h-auto min-h-12 resize-y py-1.5 leading-6")}
                />
            }
        }
        "select" => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| Msg::FieldChanged {
                name: name.clone(),
                value: crate::app::exec::select_value(&event),
            });
            html! {
                <select value={value} oninput={changed} class={INPUT_CLASS}>
                    <option value="">{"—"}</option>
                    {
                        for field.options.iter().map(|option| html! {
                            <option value={option.clone()}>{ option }</option>
                        })
                    }
                </select>
            }
        }
        "money" => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| Msg::FieldChanged {
                name: name.clone(),
                value: crate::app::exec::input_value(&event)
                    .replace('$', "")
                    .replace(',', ""),
            });
            html! {
                <input
                    inputmode="decimal"
                    value={format_money(&value)}
                    oninput={changed}
                    class={INPUT_CLASS}
                />
            }
        }
        "date" => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| Msg::FieldChanged {
                name: name.clone(),
                value: crate::app::exec::input_value(&event),
            });
            html! {
                <input
                    type="date"
                    value={value}
                    oninput={changed}
                    class={format!("{INPUT_CLASS} appearance-auto [color-scheme:light]")}
                />
            }
        }
        _ => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| Msg::FieldChanged {
                name: name.clone(),
                value: crate::app::exec::input_value(&event),
            });
            html! {
                <input type="text" value={value} oninput={changed} class={INPUT_CLASS} />
            }
        }
    };

    html! {
        <label class={format!("{span} min-w-0")}>
            <span class={format!("{LABEL_CLASS} block min-h-[1rem]")}>{ label }</span>
            { control }
        </label>
    }
}

pub(super) fn field_span_class(field: &FormTemplateField) -> &'static str {
    if field.field_type == "textarea" {
        return "col-span-6";
    }
    if matches!(field.field_type.as_str(), "date" | "money" | "select") {
        return "col-span-2";
    }
    let haystack = format!("{} {}", field.name, field.label).to_lowercase();
    if ["name", "property", "location", "address"]
        .iter()
        .any(|needle| haystack.contains(needle))
    {
        "col-span-3"
    } else {
        "col-span-2"
    }
}
