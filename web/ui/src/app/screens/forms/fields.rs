//! One template field as a control, and its grid span.

#[allow(unused_imports)]
use super::*;

pub(super) fn field_control(
    field: &FormTemplateField,
    values: &BTreeMap<String, String>,
    money_editing: Option<&str>,
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
            let changed = link.callback(move |event: Event| Msg::FieldChanged {
                name: name.clone(),
                value: crate::app::exec::select_value(&event),
            });
            // `selected` is what makes the saved choice show on load; `value` on a <select> is applied before its
            // options exist and the last option wins.
            html! {
                <select onchange={changed} class={INPUT_CLASS}>
                    <option value="" selected={value.is_empty()}>{"—"}</option>
                    {
                        for field.options.iter().map(|option| html! {
                            <option value={option.clone()} selected={*option == value}>{ option }</option>
                        })
                    }
                </select>
            }
        }
        "money" => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| Msg::MoneyTyped {
                name: name.clone(),
                value: crate::app::exec::input_value(&event)
                    .replace('$', "")
                    .replace(',', ""),
            });
            let blurred = link.callback(|_: FocusEvent| Msg::MoneyBlur);
            let shown = if money_editing == Some(field.name.as_str()) {
                value.clone()
            } else {
                model::forms_format::format_money(&value)
            };
            html! {
                <input
                    inputmode="decimal"
                    value={shown}
                    oninput={changed}
                    onblur={blurred}
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
        "email" => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| Msg::FieldChanged {
                name: name.clone(),
                value: crate::app::exec::input_value(&event),
            });
            html! {
                <input type="email" inputmode="email" value={value} oninput={changed} class={INPUT_CLASS} />
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
