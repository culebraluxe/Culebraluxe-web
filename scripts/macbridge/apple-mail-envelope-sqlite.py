#!/usr/bin/env python3
"""Bounded, read-only Apple Mail Envelope Index metadata extractor.

Purpose:
  * never enumerate Mail.app messages through Apple Events;
  * read a consistent snapshot of Mail's local Envelope Index;
  * return one bounded Inbox/Sent page for one account;
  * use keyset pagination on (occurred_at, ROWID);
  * emit metadata only (no bodies, snippets, attachments, or raw MIME).

The caller supplies the Mail.app account UUID. This script never mutates Mail.
"""
from __future__ import annotations

import argparse
import calendar
import json
import os
import re
import sqlite3
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import quote, unquote

CORE_DATA_EPOCH = 978_307_200
MAIL_ROOT = Path.home() / "Library" / "Mail"
MAILBOX_URL = re.compile(r"^[a-zA-Z][a-zA-Z0-9+.-]*://([^/]+)/(.*)$")
INBOX_LEAVES = {"inbox"}
SENT_LEAVES = {"sent", "sent mail", "sent messages", "sent items"}
MESSAGE_ID_HEADER = re.compile(r"<([^<>]+)>")


def fail(message: str, code: int = 2) -> "NoReturn":
    print(json.dumps({"ok": False, "error": message}), file=sys.stderr)
    raise SystemExit(code)


def newest_mail_root() -> Path:
    override = os.environ.get("APPLE_MAIL_VERSION_DIR", "").strip()
    if override:
        root = Path(override).expanduser()
        if not root.is_dir():
            fail(f"APPLE_MAIL_VERSION_DIR does not exist: {root}")
        return root
    # Probe with iterdir rather than is_dir(): under TCC a protected path reports
    # "not a directory" from is_dir(), which would misreport a permissions problem
    # as a missing Mail store. iterdir raises the real errno.
    try:
        versions = [p for p in MAIL_ROOT.iterdir() if p.is_dir() and re.fullmatch(r"V\d+", p.name)]
    except PermissionError:
        fail(
            f"Cannot read {MAIL_ROOT}: Operation not permitted. "
            "Grant Full Disk Access to the terminal/VS Code process running this command "
            "(System Settings > Privacy & Security > Full Disk Access), then fully restart it."
        )
    except (FileNotFoundError, NotADirectoryError):
        fail(f"Apple Mail data directory not found: {MAIL_ROOT}")
    if not versions:
        fail(f"No V* Apple Mail data directory found under {MAIL_ROOT}")
    return max(versions, key=lambda p: int(p.name[1:]))


def open_live_readonly(path: Path) -> sqlite3.Connection:
    try:
        conn = sqlite3.connect(f"file:{quote(str(path))}?mode=ro", uri=True, timeout=10)
        conn.execute("PRAGMA query_only=ON")
        conn.execute("PRAGMA busy_timeout=10000")
        return conn
    except sqlite3.Error as exc:
        fail(
            f"Cannot read Apple Mail Envelope Index at {path}: {exc}. "
            "Grant Full Disk Access to the terminal/VS Code process running this command."
        )


def make_snapshot(live_path: Path, out_path: Path) -> None:
    """Create a consistent SQLite snapshot using the online backup API.

    Opening the source read-only lets SQLite incorporate the active WAL correctly.
    The long-running query then runs against our private snapshot, not Mail's live DB.
    """
    source = open_live_readonly(live_path)
    try:
        target = sqlite3.connect(out_path)
        try:
            source.backup(target, pages=2048, sleep=0.01)
            target.commit()
        finally:
            target.close()
    finally:
        source.close()


def columns(db: sqlite3.Connection, table: str) -> set[str]:
    return {str(row[1]) for row in db.execute(f"PRAGMA table_info({table})")}


def require_schema(db: sqlite3.Connection) -> dict[str, set[str]]:
    required_tables = {"messages", "mailboxes", "addresses", "subjects", "recipients"}
    existing = {
        str(row[0]) for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")
    }
    missing_tables = sorted(required_tables - existing)
    if missing_tables:
        fail(f"Unsupported Apple Mail Envelope Index schema; missing tables: {', '.join(missing_tables)}")
    schema = {name: columns(db, name) for name in required_tables}
    required_message_cols = {"mailbox", "date_received", "date_sent", "sender", "subject"}
    missing_cols = sorted(required_message_cols - schema["messages"])
    if missing_cols:
        fail(f"Unsupported Apple Mail messages schema; missing columns: {', '.join(missing_cols)}")
    return schema


def parse_mailbox_url(url: str) -> tuple[str, str] | None:
    match = MAILBOX_URL.match(url or "")
    if not match:
        return None
    return unquote(match.group(1)), unquote(match.group(2))


def leaf(path: str) -> str:
    return path.rstrip("/").split("/")[-1].strip().lower()


def resolve_mailboxes(db: sqlite3.Connection, account_id: str) -> tuple[list[int], list[int], dict[int, str]]:
    rows = db.execute("SELECT ROWID, url FROM mailboxes WHERE url IS NOT NULL").fetchall()
    inbox: list[int] = []
    sent: list[int] = []
    names: dict[int, str] = {}
    seen_accounts: set[str] = set()
    for rowid, url in rows:
        parsed = parse_mailbox_url(str(url))
        if not parsed:
            continue
        authority, path = parsed
        seen_accounts.add(authority)
        if authority.lower() != account_id.lower():
            continue
        names[int(rowid)] = path
        lname = leaf(path)
        if lname in INBOX_LEAVES:
            inbox.append(int(rowid))
        if lname in SENT_LEAVES:
            sent.append(int(rowid))
    if not inbox or not sent:
        available = sorted({a for a in seen_accounts if a})
        fail(
            "Could not resolve required Inbox/Sent mailboxes for Mail account id "
            f"{account_id}. inbox={len(inbox)} sent={len(sent)}; "
            f"known account ids={available[:12]}"
        )
    return inbox, sent, names


def detect_epoch(db: sqlite3.Connection, mailbox_ids: list[int]) -> str:
    placeholders = ",".join("?" for _ in mailbox_ids)
    sample = db.execute(
        f"SELECT MAX(MAX(COALESCE(date_received,0), COALESCE(date_sent,0))) "
        f"FROM messages WHERE mailbox IN ({placeholders})",
        mailbox_ids,
    ).fetchone()[0]
    value = float(sample or 0)
    # Current Unix timestamps are ~1.8B; Core Data timestamps are ~0.8B.
    return "coredata" if 0 < value < 1_200_000_000 else "unix"


def unix_to_store(ts: float, epoch: str) -> float:
    return ts - CORE_DATA_EPOCH if epoch == "coredata" else ts


def store_to_iso(ts: float | int | None, epoch: str) -> str | None:
    if ts is None:
        return None
    value = float(ts)
    if value <= 0:
        return None
    unix = value + CORE_DATA_EPOCH if epoch == "coredata" else value
    try:
        return datetime.fromtimestamp(unix, tz=timezone.utc).isoformat().replace("+00:00", "Z")
    except (OSError, OverflowError, ValueError):
        return None


def months_ago(months: int) -> datetime:
    now = datetime.now(timezone.utc)
    year = now.year
    month = now.month - months
    while month <= 0:
        year -= 1
        month += 12
    day = min(now.day, calendar.monthrange(year, month)[1])
    return now.replace(year=year, month=month, day=day)


# Discrete, non-overlapping windows: "0-1" is the last month, "1-3" is the two
# months before that, and so on outward to a year. Each band is half-open,
# [since, before), so no record can land in two bands and none can fall between
# them. Reading band by band means each pass reads only its own mail instead of
# re-reading everything newer, which is why the checkpoint is keyed per band.
BANDS: dict[str, tuple[int, int]] = {
    "0-1": (1, 0),
    "1-3": (3, 1),
    "3-6": (6, 3),
    "6-12": (12, 6),
}
BAND_CHOICES = tuple(BANDS)


def band_window(band: str) -> tuple[datetime, datetime | None]:
    outer, inner = BANDS[band]
    since = months_ago(outer)
    before = months_ago(inner) if inner else None
    return since, before


def normalize_message_id_header(value: Any) -> str | None:
    """The RFC Message-ID, without the angle brackets Mail stores it in.

    Mail keeps it in message_global_data.message_id_header, for example
    "<2F48AB70-F8A4-405F-AD33-E8C16F00A32D@gmail.com>". The replay identity strips
    the brackets, which is what the rows already in l_applemail hold, so the two
    agree. messages.message_id is an INTEGER and is never an identity.
    """
    text = str(value or "").strip()
    if not text:
        return None
    match = MESSAGE_ID_HEADER.search(text)
    if match:
        return match.group(1).strip() or None
    return text.split()[0] if text.split() else None


def has_message_id_header(db: sqlite3.Connection) -> bool:
    tables = {str(row[0]) for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    if "message_global_data" not in tables:
        return False
    return "message_id_header" in columns(db, "message_global_data")


def build_page_query(
    inbox_ids: list[int], sent_ids: list[int], has_header: bool
) -> tuple[str, list[int]]:
    all_ids = inbox_ids + sent_ids
    all_ph = ",".join("?" for _ in all_ids)
    sent_ph = ",".join("?" for _ in sent_ids)
    occurred_expr = (
        f"CASE WHEN m.mailbox IN ({sent_ph}) "
        "THEN COALESCE(NULLIF(m.date_sent,0), m.date_received) "
        "ELSE COALESCE(NULLIF(m.date_received,0), m.date_sent) END"
    )
    # The RFC Message-ID lives in message_global_data, keyed by
    # messages.global_message_id. messages.message_id is an INTEGER, shared by the
    # same mail sitting in different mailboxes, and matches nothing Mail's
    # AppleEvent bridge reports - using it as an identity double-lands every
    # message, which is exactly what it did.
    if has_header:
        header_join = "LEFT JOIN message_global_data g ON g.ROWID = m.global_message_id"
        header_expr = "g.message_id_header"
    else:
        header_join = ""
        header_expr = "NULL"
    sql = f"""
        WITH base AS (
            SELECT
                m.ROWID AS rowid,
                m.mailbox AS mailbox_id,
                {occurred_expr} AS occurred_store,
                {header_expr} AS message_id_header,
                COALESCE(a.address, '') AS sender_address,
                COALESCE(a.comment, '') AS sender_name,
                COALESCE(s.subject, '') AS subject
            FROM messages m
            LEFT JOIN addresses a ON m.sender = a.ROWID
            LEFT JOIN subjects s ON m.subject = s.ROWID
            {header_join}
            WHERE m.mailbox IN ({all_ph})
              {{deleted_filter}}
        )
        SELECT * FROM base
        WHERE occurred_store >= ?
          AND (? IS NULL OR occurred_store < ?)
          AND (
              ? IS NULL
              OR occurred_store < ?
              OR (occurred_store = ? AND rowid < ?)
          )
        ORDER BY occurred_store DESC, rowid DESC
        LIMIT ?
    """
    return sql, [*sent_ids, *all_ids]


def recipient_map(db: sqlite3.Connection, rowids: list[int]) -> dict[int, dict[str, list[dict[str, Any]]]]:
    if not rowids:
        return {}
    placeholders = ",".join("?" for _ in rowids)
    rec_cols = columns(db, "recipients")
    position_expr = "COALESCE(r.position,0)" if "position" in rec_cols else "0"
    rows = db.execute(
        f"""
        SELECT r.message, r.type, COALESCE(a.address,''), COALESCE(a.comment,'')
        FROM recipients r
        JOIN addresses a ON r.address = a.ROWID
        WHERE r.message IN ({placeholders})
        ORDER BY r.message, {position_expr}
        """,
        rowids,
    ).fetchall()
    out: dict[int, dict[str, list[dict[str, Any]]]] = {}
    for message, rtype, address, name in rows:
        bucket = out.setdefault(int(message), {"to": [], "cc": [], "bcc": []})
        key = {0: "to", 1: "cc", 2: "bcc"}.get(int(rtype or 0))
        if not key:
            continue
        bucket[key].append({"address": address or None, "name": name or None})
    return out


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--account", required=True, help="Configured mailbox address, for reporting only")
    parser.add_argument("--account-id", required=True, help="Mail.app account UUID / mailbox URL authority")
    parser.add_argument("--band", choices=BAND_CHOICES, required=True, help="Discrete window: 0-1, 1-3, 3-6, or 6-12 months")
    parser.add_argument("--limit", type=int, default=500)
    parser.add_argument("--cursor-date", type=float)
    parser.add_argument("--cursor-rowid", type=int)
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()

    limit = max(1, min(int(args.limit), 1000))
    root = newest_mail_root()
    live_db = root / "MailData" / "Envelope Index"
    if not live_db.exists():
        fail(f"Apple Mail Envelope Index not found: {live_db}")

    with tempfile.TemporaryDirectory(prefix="culebraluxe-mail-index-") as tmp:
        snapshot = Path(tmp) / "Envelope Index.snapshot"
        make_snapshot(live_db, snapshot)
        db = sqlite3.connect(snapshot)
        db.row_factory = sqlite3.Row
        db.execute("PRAGMA query_only=ON")
        schema = require_schema(db)
        inbox_ids, sent_ids, mailbox_names = resolve_mailboxes(db, args.account_id)
        all_ids = inbox_ids + sent_ids
        epoch = detect_epoch(db, all_ids)
        since_iso, before_iso = band_window(args.band)
        since_store = unix_to_store(since_iso.timestamp(), epoch)
        before_store = unix_to_store(before_iso.timestamp(), epoch) if before_iso else None

        has_message_id = "message_id" in schema["messages"]
        has_header = has_message_id_header(db)
        sql, prefix = build_page_query(inbox_ids, sent_ids, has_header)
        sql = sql.replace(
            "{deleted_filter}",
            "AND COALESCE(m.deleted,0)=0" if "deleted" in schema["messages"] else "",
        )
        cursor_date = args.cursor_date
        cursor_rowid = args.cursor_rowid
        params: list[Any] = [*prefix, since_store, before_store, before_store]
        params.extend([cursor_date, cursor_date, cursor_date, cursor_rowid, limit])
        rows = db.execute(sql, params).fetchall()
        recips = recipient_map(db, [int(r["rowid"]) for r in rows])

        records: list[dict[str, Any]] = []
        for row in rows:
            rowid = int(row["rowid"])
            mailbox_id = int(row["mailbox_id"])
            kind = "sent" if mailbox_id in sent_ids else "inbox"
            sender_addr = str(row["sender_address"] or "")
            sender_name = str(row["sender_name"] or "")
            sender = f"{sender_name} <{sender_addr}>" if sender_name and sender_addr else (sender_addr or sender_name or None)
            recipient_data = recips.get(rowid, {"to": [], "cc": [], "bcc": []})
            record = {
                "mailbox": kind,
                "mailboxName": mailbox_names.get(mailbox_id, kind),
                "localId": rowid,
                "messageId": normalize_message_id_header(row["message_id_header"]),
                "occurredAt": store_to_iso(row["occurred_store"], epoch),
                "sender": sender,
                "to": recipient_data["to"],
                "cc": recipient_data["cc"],
                "bcc": recipient_data["bcc"],
                "subject": str(row["subject"] or "") or None,
            }
            records.append(record)

        next_cursor = None
        if rows:
            last = rows[-1]
            next_cursor = {
                "date": float(last["occurred_store"]),
                "rowid": int(last["rowid"]),
            }
        complete = len(rows) < limit
        result = {
            "ok": True,
            "source": "apple_mail_envelope_index",
            "account": args.account.lower(),
            "accountId": args.account_id,
            "mailVersion": root.name,
            "database": str(live_db),
            "band": args.band,
            "since": since_iso.isoformat().replace("+00:00", "Z"),
            "before": before_iso.isoformat().replace("+00:00", "Z") if before_iso else None,
            "pageSize": limit,
            "epoch": epoch,
            "mailboxes": {
                "inbox": [mailbox_names[i] for i in inbox_ids],
                "sent": [mailbox_names[i] for i in sent_ids],
            },
            "records": records,
            "nextCursor": next_cursor,
            "complete": complete,
            "verify": bool(args.verify),
        }
        print(json.dumps(result, separators=(",", ":"), ensure_ascii=False))


if __name__ == "__main__":
    main()
