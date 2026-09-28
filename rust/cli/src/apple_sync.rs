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
        "messages-intake" => {
            let dir = path_arg(args, 1, "messages-intake")?;
            let options = crate::apple_messages::IntakeOptions {
                evidence_only: args.iter().any(|arg| arg == "--evidence-only"),
                refresh: args.iter().any(|arg| arg == "--refresh"),
            };
            crate::apple_messages::intake_messages(dir, options).await
        }
        // Apple Mail: envelope-index intake into `l_applemail`, then the one promotion pass that
        // reads a landing table. See `apple_mail` for the chain and its privacy boundary.
        "mail-intake" => crate::apple_mail::mail_intake(&args[1..]).await,
        "mail-promote" => crate::apple_mail::mail_promote(&args[1..]).await,
        other => Err(io::Error::other(format!("unknown apple-sync command: {other}")).into()),
    }
}

/// The repository this process is operating on.
///
/// The working directory was the old rule, and it was a trap: `repo_root()` answers "where is this repository",
/// and a process started in `rust/` is in the repository too. Every `forge` command that needs a database calls
/// `load_env()` below, so running one from a subdirectory searched `<subdir>/.env.local`, found nothing, and
/// reported the database as unreachable — on 2026-09-28 that read as "DEV is down" for a night while the only
/// fault was the directory the command was typed in. `git rev-parse --show-toplevel` is the answer that does not
/// depend on where the shell happens to be.
pub(crate) fn repo_root() -> PathBuf {
    std::env::var("CULEBRALUXE_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|_| crate::forge::repo_root())
}

/// `.env.local` carries DATABASE_URL_DEV / DATABASE_URL_PROD, and it is the repository's file: it is loaded by
/// absolute path from `repo_root()`, never from the working directory.
pub(crate) fn load_env() {
    let path = repo_root().join(".env.local");
    let _ = dotenvy::from_path(&path);
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

fn calendar_source_message_id(raw: &Value) -> Option<String> {
    if let Some(series_id) = text(raw, "calendarItemIdentifier") {
        let occurrence = text(raw, "occurrenceDate");
        let recurring = raw
            .get("recurring")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || occurrence.is_some();
        if recurring {
            let occurrence = occurrence.or_else(|| text(raw, "startAt"))?;
            return Some(format!("{series_id}|{occurrence}"));
        }
        return Some(series_id);
    }

    text(raw, "sourceMessageId")
        .or_else(|| text(raw, "eventIdentifier"))
        .or_else(|| {
            Some(format!(
                "{}|{}",
                text(raw, "eventIdentifier")?,
                text(raw, "startAt")?
            ))
        })
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
        let source_message_id = calendar_source_message_id(raw);
        let (Some(source_message_id), Some(start_at), Some(end_at)) = (
            source_message_id,
            text(raw, "startAt"),
            text(raw, "endAt"),
        ) else {
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_landing_identity_survives_moves_and_occurrence_id_changes() {
        let one_time_before = json!({
            "sourceMessageId": "old-event|2026-09-27T09:00:00Z",
            "eventIdentifier": "old-event",
            "calendarItemIdentifier": "stable-item",
            "startAt": "2026-09-27T09:00:00Z",
            "recurring": false
        });
        let one_time_after = json!({
            "sourceMessageId": "new-event|2026-09-27T11:00:00Z",
            "eventIdentifier": "new-event",
            "calendarItemIdentifier": "stable-item",
            "startAt": "2026-09-27T11:00:00Z",
            "recurring": false
        });
        assert_eq!(
            calendar_source_message_id(&one_time_before),
            calendar_source_message_id(&one_time_after),
            "one-time identity is independent of eventIdentifier and moved start"
        );
        assert_eq!(
            calendar_source_message_id(&one_time_after).as_deref(),
            Some("stable-item")
        );

        let occurrence_before = json!({
            "eventIdentifier": "occ-old",
            "calendarItemIdentifier": "series-1",
            "occurrenceDate": "2026-10-05T13:00:00Z",
            "startAt": "2026-10-05T13:00:00Z",
            "recurring": true
        });
        let occurrence_after = json!({
            "eventIdentifier": "occ-new",
            "calendarItemIdentifier": "series-1",
            "occurrenceDate": "2026-10-05T13:00:00Z",
            "startAt": "2026-10-05T15:00:00Z",
            "recurring": true,
            "detached": true
        });
        assert_eq!(
            calendar_source_message_id(&occurrence_before),
            calendar_source_message_id(&occurrence_after),
            "recurring identity is series plus original occurrence date"
        );
        assert_eq!(
            calendar_source_message_id(&occurrence_after).as_deref(),
            Some("series-1|2026-10-05T13:00:00Z")
        );
    }
}
