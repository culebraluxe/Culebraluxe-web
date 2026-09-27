use chrono::{Duration, Utc};
use db::{CalendarDao, Database, DomainEventOutboxDao, OutboxDelivery, WbsDao};
use domain::{AppleReminderLanding, CalendarLandingEvent};
use serde_json::{json, Value};
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

const CALENDAR_CREATE: &str = "apple.calendar.create.requested";
const CALENDAR_UPDATE: &str = "apple.calendar.update.requested";
const REMINDER_UPSERT: &str = "apple.reminder.upsert.requested";
const SUB_CALENDAR_CREATE: &str = "apple-gateway-calendar-create-v2";
const SUB_CALENDAR_UPDATE: &str = "apple-gateway-calendar-update-v1";
const SUB_REMINDER: &str = "apple-gateway-reminder-v2";

pub async fn dispatch(args: &[String]) -> Result<(), Box<dyn Error>> {
    load_env();
    match args.first().map(String::as_str).unwrap_or_default() {
        "drain" => drain().await,
        "calendar-intake" => intake_calendar(path_arg(args, 1, "calendar-intake")?).await,
        "reminder-intake" => intake_reminders(path_arg(args, 1, "reminder-intake")?).await,
        other => Err(io::Error::other(format!("unknown apple-sync command: {other}")).into()),
    }
}

fn repo_root() -> PathBuf {
    std::env::var("CULEBRALUXE_REPO")
        .map(PathBuf::from)
        .or_else(|_| std::env::current_dir())
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn load_env() {
    let _ = dotenvy::from_path(repo_root().join(".env.local"));
}

fn path_arg<'a>(
    args: &'a [String],
    index: usize,
    command: &str,
) -> Result<&'a Path, Box<dyn Error>> {
    args.get(index).map(Path::new).ok_or_else(|| {
        io::Error::other(format!("apple-sync {command} requires a snapshot path")).into()
    })
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn optional_text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

async fn intake_calendar(path: &Path) -> Result<(), Box<dyn Error>> {
    let parsed: Value = serde_json::from_slice(&fs::read(path)?)?;
    let items = parsed
        .as_array()
        .ok_or_else(|| io::Error::other("Calendar snapshot is not an array"))?;
    let dao = CalendarDao::new(Database::connect_from_env().await?);
    let mut upserted = 0usize;
    let mut rejected = 0usize;

    for raw in items {
        let provider = text(raw, "eventIdentifier");
        let start = text(raw, "startAt");
        let source_message_id = text(raw, "sourceMessageId")
            .or_else(|| Some(format!("{}|{}", provider.as_deref()?, start.as_deref()?)));
        let (Some(source_message_id), Some(start_at), Some(end_at)) =
            (source_message_id, start, text(raw, "endAt"))
        else {
            rejected += 1;
            continue;
        };

        dao.upsert_landing_event(&CalendarLandingEvent {
            source_account: text(raw, "sourceAccount").unwrap_or_else(|| "apple-calendar".into()),
            source_message_id,
            title: optional_text(raw, "title").unwrap_or_default(),
            start_at,
            end_at,
            all_day: raw.get("allDay").and_then(Value::as_bool).unwrap_or(false),
            location: optional_text(raw, "location"),
            raw: raw.clone(),
        })
        .await?;
        upserted += 1;
    }

    println!(
        "{}",
        json!({
            "source": "apple_eventkit",
            "received": items.len(),
            "upserted": upserted,
            "rejected": rejected,
        })
    );
    Ok(())
}

async fn intake_reminders(path: &Path) -> Result<(), Box<dyn Error>> {
    let parsed: Value = serde_json::from_slice(&fs::read(path)?)?;
    let items = parsed
        .as_array()
        .ok_or_else(|| io::Error::other("Reminders snapshot is not an array"))?;
    let dao = WbsDao::new(Database::connect_from_env().await?);
    let mut upserted = 0usize;
    let mut rejected = 0usize;

    for raw in items {
        let Some(source_message_id) = text(raw, "reminderIdentifier") else {
            rejected += 1;
            continue;
        };
        dao.upsert_apple_reminder_landing(&AppleReminderLanding {
            source_account: text(raw, "sourceAccount").unwrap_or_else(|| "apple-reminders".into()),
            source_message_id,
            external_id: optional_text(raw, "externalIdentifier"),
            list_name: optional_text(raw, "listName"),
            title: optional_text(raw, "title"),
            notes: optional_text(raw, "notes"),
            start_at: optional_text(raw, "startAt"),
            due_at: optional_text(raw, "dueAt"),
            completed: raw
                .get("completed")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            completed_at: optional_text(raw, "completedAt"),
            priority: raw
                .get("priority")
                .and_then(Value::as_i64)
                .and_then(|value| i32::try_from(value).ok()),
            raw: raw.clone(),
        })
        .await?;
        upserted += 1;
    }

    println!(
        "{}",
        json!({
            "source": "apple_eventkit_reminders",
            "received": items.len(),
            "upserted": upserted,
            "rejected": rejected,
        })
    );
    Ok(())
}

async fn drain() -> Result<(), Box<dyn Error>> {
    let outbox = DomainEventOutboxDao::new(Database::connect_from_env().await?);
    for (id, route) in [
        (SUB_CALENDAR_CREATE, CALENDAR_CREATE),
        (SUB_CALENDAR_UPDATE, CALENDAR_UPDATE),
        (SUB_REMINDER, REMINDER_UPSERT),
    ] {
        outbox.register_subscription(id, route, 5, 30).await?;
    }

    let subscriptions = vec![
        SUB_CALENDAR_CREATE.to_string(),
        SUB_CALENDAR_UPDATE.to_string(),
        SUB_REMINDER.to_string(),
    ];
    let worker = format!("apple-gateway:{}", Uuid::new_v4());
    let deliveries = outbox
        .claim_batch(
            &worker,
            &subscriptions,
            20,
            Utc::now() + Duration::minutes(5),
        )
        .await?;

    let mut delivered = 0usize;
    let mut failed = 0usize;
    for delivery in deliveries {
        match deliver(&delivery) {
            Ok(()) => {
                if outbox.mark_delivered(&delivery, &worker).await? {
                    delivered += 1;
                }
            }
            Err(error) => {
                failed += 1;
                let _ = outbox
                    .mark_failed(&delivery, &worker, &error.to_string())
                    .await?;
            }
        }
    }

    println!(
        "{}",
        json!({
            "source": "apple_gateway_outbound",
            "claimed": delivered + failed,
            "delivered": delivered,
            "failed": failed,
        })
    );
    if failed > 0 {
        return Err(io::Error::other("one or more Apple gateway deliveries failed").into());
    }
    Ok(())
}

fn deliver(delivery: &OutboxDelivery) -> Result<(), Box<dyn Error>> {
    let payload = command_payload(delivery)?;
    let path = std::env::temp_dir().join(format!(
        "culebraluxe-apple-command-{}.json",
        delivery.event_id
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&path)?;
    file.write_all(serde_json::to_string(&payload)?.as_bytes())?;
    drop(file);

    let output = Command::new("/usr/bin/swift")
        .arg(repo_root().join("scripts/macbridge/AppleGatewayWrite.swift"))
        .arg("--input")
        .arg(&path)
        .current_dir(repo_root())
        .output();
    let _ = fs::remove_file(&path);
    let output = output?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(io::Error::other(if detail.is_empty() {
            format!("Swift EventKit writer exited {}", output.status)
        } else {
            detail
        })
        .into());
    }
    Ok(())
}

fn command_payload(delivery: &OutboxDelivery) -> Result<Value, Box<dyn Error>> {
    let payload = &delivery.payload;
    let required = |key: &str| {
        text(payload, key).ok_or_else(|| {
            io::Error::other(format!("{} payload missing {key}", delivery.event_type))
        })
    };

    Ok(match delivery.event_type.as_str() {
        CALENDAR_CREATE => json!({
            "kind": "calendar_create",
            "commandId": delivery.event_id,
            "title": required("title")?,
            "startAt": required("startAt")?,
            "endAt": required("endAt")?,
            "allDay": payload.get("allDay").and_then(Value::as_bool).unwrap_or(false),
            "location": optional_text(payload, "location"),
            "notes": optional_text(payload, "notes"),
            "alert": payload.get("alert").and_then(Value::as_bool).unwrap_or(false),
        }),
        CALENDAR_UPDATE => json!({
            "kind": "calendar_update",
            "commandId": delivery.event_id,
            "eventId": required("eventId")?,
            "startAt": required("startAt")?,
            "endAt": required("endAt")?,
            "allDay": payload.get("allDay").and_then(Value::as_bool),
            "recurrenceScope": text(payload, "recurrenceScope").unwrap_or_else(|| "this".into()),
        }),
        REMINDER_UPSERT => json!({
            "kind": "reminder_upsert",
            "commandId": delivery.event_id,
            "wbsId": required("wbsId")?,
            "title": required("title")?,
            "dueAt": optional_text(payload, "dueAt"),
            "completed": payload.get("completed").and_then(Value::as_bool).unwrap_or(false),
            "notes": optional_text(payload, "notes"),
            "alert": payload.get("alert").and_then(Value::as_bool).unwrap_or(false),
        }),
        other => {
            return Err(io::Error::other(format!("unsupported Apple route: {other}")).into());
        }
    })
}
