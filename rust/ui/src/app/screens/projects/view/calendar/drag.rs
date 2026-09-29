//! Dragging an event: start, end, its payload, and dropping on a time or an all-day cell.

#[allow(unused_imports)]
use super::*;

pub(super) fn drag_start(
    event: &CalendarChip,
    kind: &'static str,
    on_msg: &Callback<Msg>,
) -> Callback<DragEvent> {
    let payload = serde_json::json!({
        "kind": kind,
        "occurrenceId": event.id,
        "providerEventId": event.provider_event_id,
        "providerSeriesId": event.provider_series_id,
        "startAt": event.start_at,
        "endAt": event.end_at,
        "allDay": event.all_day,
    })
    .to_string();
    let id = event.id.clone();
    let on_msg = on_msg.clone();
    Callback::from(move |event: DragEvent| {
        if let Some(data) = event.data_transfer() {
            let _ = data.set_data("text/plain", &payload);
            data.set_effect_allowed("move");
            on_msg.emit(Msg::ProjectCalendarDragStarted(id.clone()));
        }
    })
}

pub(super) fn drag_end(on_msg: &Callback<Msg>) -> Callback<DragEvent> {
    let on_msg = on_msg.clone();
    Callback::from(move |_: DragEvent| on_msg.emit(Msg::ProjectCalendarDragEnded))
}

pub(super) fn drop_payload(event: &DragEvent) -> Option<serde_json::Value> {
    let data = event.data_transfer()?;
    let raw = data.get_data("text/plain").ok()?;
    serde_json::from_str(&raw).ok()
}

pub(super) fn text_field<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

pub(super) fn timed_drop(
    on_msg: &Callback<Msg>,
    date: String,
    slot: usize,
    grid: GridSpec,
) -> Callback<DragEvent> {
    let on_msg = on_msg.clone();
    Callback::from(move |event: DragEvent| {
        event.prevent_default();
        let Some(payload) = drop_payload(&event) else {
            return;
        };
        let Some(occurrence_id) = text_field(&payload, "occurrenceId") else {
            return;
        };
        let Some(provider_event_id) = text_field(&payload, "providerEventId") else {
            return;
        };
        let Some(old_start) = text_field(&payload, "startAt") else {
            return;
        };
        let old_end = text_field(&payload, "endAt");
        let was_all_day = payload
            .get("allDay")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let Some(target) = crate::calendar::slot_timestamp(&date, slot, grid) else {
            return;
        };
        let span = if text_field(&payload, "kind") == Some("resize") {
            if was_all_day {
                None
            } else {
                crate::calendar::resize_span(old_start, &target, grid)
            }
        } else {
            crate::calendar::move_to_timed(old_start, old_end, was_all_day, &target)
        };
        let Some((start_at, end_at)) = span else {
            return;
        };
        on_msg.emit(Msg::ProjectCalendarEditRequested {
            occurrence_id: occurrence_id.to_owned(),
            provider_event_id: provider_event_id.to_owned(),
            provider_series_id: text_field(&payload, "providerSeriesId").map(str::to_owned),
            start_at,
            end_at,
            all_day: false,
        });
    })
}

pub(super) fn all_day_drop(on_msg: &Callback<Msg>, date: String) -> Callback<DragEvent> {
    let on_msg = on_msg.clone();
    Callback::from(move |event: DragEvent| {
        event.prevent_default();
        let Some(payload) = drop_payload(&event) else {
            return;
        };
        if text_field(&payload, "kind") == Some("resize") {
            return;
        }
        let Some(occurrence_id) = text_field(&payload, "occurrenceId") else {
            return;
        };
        let Some(provider_event_id) = text_field(&payload, "providerEventId") else {
            return;
        };
        let Some(old_start) = text_field(&payload, "startAt") else {
            return;
        };
        let old_end = text_field(&payload, "endAt");
        let was_all_day = payload
            .get("allDay")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let Some((start_at, end_at)) =
            crate::calendar::move_to_all_day(old_start, old_end, was_all_day, &date)
        else {
            return;
        };
        on_msg.emit(Msg::ProjectCalendarEditRequested {
            occurrence_id: occurrence_id.to_owned(),
            provider_event_id: provider_event_id.to_owned(),
            provider_series_id: text_field(&payload, "providerSeriesId").map(str::to_owned),
            start_at,
            end_at,
            all_day: true,
        });
    })
}
