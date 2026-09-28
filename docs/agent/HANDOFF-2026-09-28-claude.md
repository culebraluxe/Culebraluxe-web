# Session handoff — 2026-09-28 (Claude, worktree `Culebraluxe-web-claude2`)

Read this first in the next session, then `AGENTS.md` (House Rules) and `docs/agent/MEMORY.md`.

## How to work with the owner (short)

- Do exactly what is asked; ask one short numbered question if unsure. Never substitute a design.
- Every production step (migrations, loads, deploys) is stated first and waits for a yes. The owner runs deletes and
  anything the tool blocks; give them the exact command or SQL.
- Before any production DELETE, list the cascades (`pg_constraint confdeltype = 'c'`).
- Never touch Vercel environment variables. Never commit `.env.local`.
- Zero TypeScript. Safari must work. All work lands on `main` (rebase, then push).
- Identity order: catastro, then phone, then email, then name in either word order.
- Run the CLI from the **repo root** — it reads `.env.local` from the current directory. Run from `rust/` it fails
  with a misleading `DatabaseUnavailable during db.connect`. (That is also why the last handoff said "DEV down"; DEV
  was up.)

## Just landed — card-size photos (commit fe400426)

Listing cards were downloading 1.6–2.7 MB web copies. Each photo now also gets a CARD copy (1200 px, quality 82,
about 120–250 KB); the public route takes `?size=card|thumb`, falling back to web; public photos cache for a year.
Photos stay in the vault `media` table (bytea) — object storage was evaluated and not adopted.

| Step | DEV | PROD |
| --- | --- | --- |
| Migration `252_media_card_copy.sql` (allows kind `card`) | applied | **owner to run** |
| Backfill `cli media-cards <target>` (safe to repeat) | 165 made, 1 AVIF skipped | **owner to run** |
| Deploy | — | **after** the migration: new uploads write a card row and PROD rejects it without 252 |

PROD, from the repo root:

```
./rust/target/debug/cli db-tool apply db/migrations/252_media_card_copy.sql prod
./rust/target/debug/cli media-cards prod     # repeat until "done: 0 card copies"
```

Not verified: the new server end to end in Safari/WebKit. The photo query was checked directly against DEV.

## Stories — open work, in the owner's priority order

1. **MEDIA-CARD-PROD-01** — finish the table above; spot-check a listing page in Safari (card photos load, sizes
   ~200 KB in Web Inspector).
2. **ENG-HARDENING-01** — owner to decide each: raise PROD restore history (now 6 h,
   `history_retention_seconds = 21600`); `cargo audit` + the 45 Dependabot alerts (4 critical); CI gate; rate limits;
   DB statement timeouts; a restore drill.
3. **ENG-FORGE-SECRET-HISTORY-01** — check git history for committed credentials.
4. **STORYBOARD-CLEANUP-01** — owner runs the 14 storyboard deletes (SQL given in-session); recon of the ~50 Forge
   stories flipped to Rust, with DeepSeek.
5. **TECH-FLIGHT-RECORDER-01** — packet is written and ready: `docs/agent/packets/TECH-FLIGHT-RECORDER-01.md`.
6. **PROJECTS-DOCS-02** — Projects Documents pane: "Record signed" / "Mark signed" / "signed copy to come" shipped;
   the owner will send copy text.
7. **PROJECTS-TREE-01** — Projects left-tree redesign, with the owner (smaller fonts).
8. Later, owner to scope: Contracts vs Workflows; Seller Strategy; Financials; Support pass; Marketing redesign
   (for GPT).

## Shipped this session (for reference)

Safari-safe chunked, resumable photo uploads; delete photo / make hero; portrait orientation; acres for lot size;
editable Person on Records; Forms Grok fill, mic, Apple Share; Mux video upload + player; Gantt N+1 and broken-link
fix; House Rules + pre-push hook; listing projects (Solar 6, Crown Paradise, Zoni Bluff, Horizon Bay) with dated
steps; FIND catastro merges the parcel record; golden-person merge (migration 228); contracts linked by catastro;
Forms root-cause fix; `db/` moved out of `legacy/`.
