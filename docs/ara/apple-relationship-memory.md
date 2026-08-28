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

## 3. OPERATIONS — canonical Apple sync (twice daily)

The normal production cycle is one durable command that runs the whole chain and
is also what the twice-daily LaunchAgent executes (no separate scheduled
implementation).

### MANUAL

```sh
./scripts/apple-sync.sh        # or: pnpm apple:sync
```

### AUTOMATIC

`com.culebraluxe.apple-sync` runs `./scripts/apple-sync.sh` (via an
FDA-grantable launcher) at **08:00 and 18:00 local time** every day.

### PIPELINE

```
~/Library/Messages/chat.db
  -> apple-messages-export (READ-ONLY, fresh export)
  -> public/upload/data/apple-messages-export/   (gitignored package)
  -> scripts/apple-messages-intake.ts prod        (ODS -> reconcile -> interaction)
  -> Client read-model refresh  (conversation-burst Contact History)
```

### LOG

```
~/Library/Logs/CulebraLuxe/apple-sync.log        (durable run log)
~/Library/Logs/CulebraLuxe/apple-sync.out.log    (launchd stdout)
~/Library/Logs/CulebraLuxe/apple-sync.err.log    (launchd stderr)
```

Each run logs start/end timestamps, exporter result, handles/messages + min/max
date, PROD intake tally (inserted / replayed / skipped group chat), errors, total
duration, and a final `status=SUCCESS|FAILED|SKIPPED`. Message bodies are never
logged.

### STATUS / INSTALL / UNINSTALL / RUN / VERIFY

```sh
./scripts/apple-sync-status.sh      # or pnpm apple:sync:status
./scripts/apple-sync-install.sh     # or pnpm apple:sync:install
./scripts/apple-sync-uninstall.sh   # or pnpm apple:sync:uninstall
./scripts/apple-sync.sh             # or pnpm apple:sync:run (immediate refresh)
pnpm apple:sync:verify-tcc          # prove the launchd binary can open chat.db
```

`install` builds the FDA-grantable Swift launcher, deploys a copy of
`apple-sync.sh` to `~/Library/Application Support/CulebraLuxe/` (outside the
TCC-protected `~/Documents`), renders/validates the plist, and bootstraps it into
launchd. A single-flight lock (`/tmp/culebraluxe-apple-sync.lock`) prevents
overlapping runs; a concurrent run logs `Apple sync already running; skipping.`
and exits 0. Stale locks (dead PID / older than 4h) are reclaimed.

### Full Disk Access (one-time, only this once)

Reading `~/Library/Messages/chat.db` requires macOS **Full Disk Access**. This
must be granted **once** by the machine owner — the schedule and all code are
complete and installed. Because macOS can only grant FDA to an executable (not
`/bin/bash`), launchd invokes a compiled launcher:

```
/Users/<you>/Library/Application Support/CulebraLuxe/apple-sync-launcher
```

**One-time step (Chris):** System Settings → Privacy & Security → **Full Disk
Access** → **+** → add the file
`~/Library/Application Support/CulebraLuxe/apple-sync-launcher` (enable it).

**Verify** after granting:

```sh
pnpm apple:sync:verify-tcc     # expect: TCC VERIFY: OK chat.db opened READ-ONLY message_count=...
```

If the launcher is ever rebuilt and redeployed, the TCC grant is re-issued
(install prints a note). After the grant, trigger a run to confirm:

```sh
launchctl kickstart gui/$(id -u)/com.culebraluxe.apple-sync
tail -f ~/Library/Logs/CulebraLuxe/apple-sync.log
```

The exporter always opens `chat.db` **READ-ONLY**; the DB's permissions are never
weakened.


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
