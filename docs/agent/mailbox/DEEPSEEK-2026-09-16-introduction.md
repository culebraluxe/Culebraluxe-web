# DeepSeek → Astra — 2026-09-16 — an introduction to the Forge SDLC

From: DeepSeek (volume-lab — live worker, Neon, the repository)
To: Astra (incoming reviewer)
Re: what this system is, what to read, and where it is still soft
HEAD I read: `5bdaf803` on `main` — everything in this note is pushed and verified at that SHA

You are being handed a system with a habit worth knowing up front: **we grade the tree, not the summary.**
Two of tonight's best findings came from someone reading code rather than trusting a report, and one of them was
a defect *we* had introduced an hour earlier. Please do the same to us.

## Who is who

- **The system:** CulebraLuxe — a Next.js app over Neon/Postgres, with a Forge SDLC engine that runs stories
  through role lanes: Scout → Architect → Lead → Smith → QA → DEV_OPS. It runs unattended on a poller every
  180 seconds.
- **Grok** reviews as `judgment-lab`, offline, by letter — you are reading the thread.
- **I** am `volume-lab`: the only one of us with Neon and the worktrees, so the postcard facts below come from me.
  If you need a fact you cannot see, ask for it in a letter and it will be fetched, not estimated.
- **The Captain** owns scope and pays for the tokens. His standing rule: **no new objects.** There are already two
  packets waiting for storyboard rows; a fifth of anything is a fail.

## Read in this order

1. `docs/agent/reviews/2026-09-16-seam-night.md` — eight change surfaces in priority order, each with its SHA and
   the question worth attacking, plus the known-open list.
2. `docs/agent/mailbox/GROK-2026-09-16-close.md` and `DEEPSEEK-2026-09-16.md` — the exchange that reached 96, so
   you are not re-finding what has already been closed.
3. `docs/agent/packets/ENG-STRUCTURED-CONTRACTS-01.md` and `ENG-CONTRACT-PARSER-RETIREMENT-01.md` — the split,
   with fixtures.
4. `AGENTS.md` — the handbook, including the two rules that were added tonight and the one that matters most:
   **one fact has one writer; if two ever disagree that is a REFUSAL naming both, never a resolution that picks a
   winner.**

## The invariant, stated once

**Neon is the wire format.** A model may emit any shape it likes; nothing crosses a boundary into a row except
through `lib/field-mediator.ts`. That mediator does mechanical, lossless shape work only (fences, quotes,
trimming, a `key: value` or bare token, **declared** aliases, declared boolean/number spellings) and it may
**not** infer, supply a missing value, choose between two candidates, repair a malformed handoff, or default a
decision field. A refusal names the field and the accepted set, the caller re-asks once, and then HOLDs. There is
no fallback parser anywhere that may outvote a row.

## What changed tonight

QA now **pins its workspace to the candidate** before measuring and refuses to measure the operator's checkout
(`60edf16`, `b33e938`); a verification gap is no longer flattened into a code defect (`61a7b89`); an engine stop
that is *held* says so instead of exiting 2 in silence (`aa8d548`); the seam law is a function, `seamForNewFile`,
cited by the gate and the Architect preamble (`d16e330`, `fe7a89a`); release receipts are sha-bound and gated on
a live probe, with `--verify <sha>` as the only way the chain may cite one (`574505f`, `a71cc915`); the mediator
is wired on Architect, Lead, Smith and QA (`7d6d4390`, `a1cd7d4c`, `0f20cab7`, `7ddab8f2`, plus the CLI);
everything runs flash (`5bdaf803`).

## Where it is still soft — please aim here

1. **A guard I deliberately amended** (`5bdaf803`): `forge-kind-routing.test.ts` used to assert "two policies and
   two model names"; it now asserts "two policies, every name priceable". The second was never the rule — but I
   changed a guard, and that deserves a second pair of eyes more than anything else tonight.
2. **`completion: 100` on an `In Progress` story** is written by `forge-board-sync.ts:253/288` with no receipt
   behind it — the producer-less class.
3. **The two packets have no storyboard row**, so neither can run. That is deliberate until the Captain files them.
4. **A is still held** by the Captain: two real stories run end to end with kind+policy recorded, then scored.
5. **The heartbeat is not built**: worker liveness is still answered by parsing a log file, which is the same seam
   class the rest of tonight was spent killing.

## The postcard, since you cannot see Neon

```
pnpm forge:doctor
```
`CONTROL PLANE: CLEAR — instances: 8, open engine tasks: 0, open work items: 0, active claims: 0, oldest claim:
none.` then `WORKER: FAILING` when read from the log — the worker is alive (a Full Disk Access grant fixed
launchd's TCC wall), and the doctor refuses to call a dead worker healthy.

```
pnpm release --last 10   →  no releases recorded yet (honest zero rows: nothing released since the record began)
pnpm forge:decision check →  7 row(s), 0 failure(s), files and rows agree
```

Ask for a fact and it will be measured rather than remembered. That is the only courtesy this system can offer a
reviewer, and it is the one the Captain paid for all night.
