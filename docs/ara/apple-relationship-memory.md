# Apple Relationship Memory — durable operator process

End-to-end: real Apple Messages history for actual clients (e.g. Ami) becomes a
Cloze-style relationship timeline in the Clients Contact History panel.

```
local Apple chat.db
  -> Swift export package (apple-messages-export)                 [Mac, FDA]
  -> scripts/apple-messages-intake.ts <dev|prod>                  [server/app]
     - ODS relationship-evidence upsert
     - reconciliation to canonical Person
     - canonical interaction materialization (exact_linked only)
     - client read-model refresh
  -> Clients Contact History timeline (conversation bursts)
```

The Mac is the trusted acquisition point: Apple protects `chat.db`, so only a
process running under a Full-Disk-Access-granted terminal can read it. The
downstream intake command is environment-agnostic and reusable once a valid
export package exists.

## 1. Repair a stale snapshot (one command, Chris)

The current snapshot was generated before the Swift INTEGER-timestamp fix, so
every `dateISO` is null (aggregate counts exist, detailed history is empty).
Repair ONLY the existing GUID population from `chat.db` (never adds/removes
GUIDs, never changes row count):

```sh
# From a Terminal with Full Disk Access, inside the repo root:
cd apple-messages-export && swift run apple-messages-export \
  --repair ../public/upload/data/apple-messages-export/messages.jsonl
```

Prints: source rows before, GUID count before/after, repaired count, missing
GUIDs, `dateISO` non-null after, duplicate GUIDs, and the
`GUID set before == after` proof.

## 2. Intake / materialize (DEV or PROD)

```sh
# DEV:
node --env-file=.env.local --import tsx scripts/apple-messages-intake.ts dev

# PROD (DATABASE_URL_PROD required; refuses to fall back):
DATABASE_URL_PROD=postgres://... node --env-file=.env.local \
  --import tsx scripts/apple-messages-intake.ts prod
```

Replay-safe: the second run inserts zero new interactions
(`source_system`=`apple_messages` + GUID replay key).

## Contract notes

- **Bounded preview**: each message interaction persists a ≤160-char one-line
  memory cue (`interaction.summary`); never full transcripts / attachment
  dumps. No-text messages get a neutral `Message` / `Attachment` label.
- **Conversation bursts**: Contact History groups per-client + per-channel
  messages into moments (≤30 min gap = one burst) with start/end time, count,
  in/out, two-way direction, and a deterministic latest non-empty preview.
- **Attribution safety**: only `review_state = exact_linked` handles with a
  canonical person materialize; group-chat GUIDs are excluded from
  person-specific materialization and reported.
- **Observed counts** (e.g. Ami 4,844) are derived from real source messages
  and are never edited to make the UI look right.
