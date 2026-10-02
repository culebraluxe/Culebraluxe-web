# The proposal lane — workers with no local git and no local build

This file is the contract for a model that cannot run the gate: GPT on a remote box with a hard timeout, Grok reading a
tree he cannot write, any future worker that can produce good code but cannot run `cargo`. It exists because **Rust's
exit condition is "it compiles, the tests run, the UI is clean", and that exit condition can only be evaluated on a
machine that can run the build.** A worker without one cannot finish a slice, only start one — so the work stops at the
worst possible moment, holding a branch nobody is allowed to look at.

Why this exists, from the record. On 2026-10-02 a session crashed holding 19 commits on
`forge-service-finish-20261002`. It was on `origin` the whole time — pullable, reviewable, deployable — and it was
still lost for hours, because rule 1 had taught everyone that branches are not a place where work lives. That worker
had broken rule 1 *while trying to obey rule 6* ("never hold work back"): the rules gave him no legal way to hand off a
partial slice, so a branch was the rational choice. Three models worked simultaneously that night and got far; what
failed was the **last mile of total integration**, which is exactly the part that needs a machine that can build.

## The four roles

| role | who | owns | never does |
| --- | --- | --- | --- |
| **Author** | GPT, Grok | turning one slice into one *proposal* | touches `main`; needs git at all |
| **Relay** | Cline, Claude (VS Code, local git, local toolchain) | applying the proposal, running the gate, returning a receipt, landing what passes | invents the author's design; hides that it finished something |
| **Owner** | the captain | picking the slice, arming gates, deciding policy (baselines, triage, deploys) | — |
| **Checker** | CI (`gates.yml`) | the final receipt | — |

**Integration is never the author's job.** The last mile belongs to whoever can run the build. This is not a demotion;
it is the one division of labour that makes a no-local-build worker productive instead of dangerous.

## The slice

A slice is *the largest unit of work that can be finished inside one author window and verified without a full build*.
It is sized by **gate cost, not feature size**.

A slice must be: one crate, one concern · ≤ ~150 changed lines · no new dependency (a `Cargo.toml` without
`rust/Cargo.lock` is what refused every push in the house on 2026-10-02) · no file over the 800-line cap · none of the
four deferred trees · no wasm build · no live database · not integration of the author's own earlier slice.

### The menu — pick a shape, know its cost before you start

| shape | example | what proves it | relay cost |
| --- | --- | --- | --- |
| pure function + unit test | a parser, a formatter, a validator | `cargo test -p <crate> --lib <name>` | seconds |
| type / mapping change | a struct field + its serde or SQL mapping | `cargo check -p <crate>` + the crate's tests | a minute |
| error mapping | a new `ApiError`/`DbFailure` arm with its message | `cargo check` + the error test | a minute |
| test-only | a missing case on an existing function | the suite | a minute |
| mechanical move | move-only split of one grandfathered file | `git diff -M` shows renames only | minutes |
| docs / packet | a packet, a triage row, a handoff | `pnpm forge:packet-lint` | seconds |
| UI wiring | one screen's view/assembly | `cargo check -p ui --features wasm --target wasm32-unknown-unknown` | **heavy — prefer relay-owned** |

**Out of scope for this lane:** cross-crate refactors, async runtime wiring, migrations, dependency bumps, anything
inside the four deferred trees, anything needing the wasm target to be *correct*, and integration of your own work.

### The pit-stop rule

The window is a budget, and the last fifth of it belongs to the proposal, not to more code.

- **Declare the slice before writing.** If it does not fit on the menu, it is two slices.
- **Never leave the tree mid-refactor.** Two files where one does not compile by construction is zero landed.
- **Stop at the last boundary that still compiles by construction** and emit the proposal with
  `status: partial` — naming exactly what is unverified. A partial proposal is a deliverable, not a failure; it is how
  the next window starts warm instead of re-reading the repository.
- **The anti-pattern to kill is "keep going until the window closes."** That is the behaviour that strands work.

## The proposal (what the author hands over)

`docs/agent/proposals/<SLICE-ID>/` containing:

- `PROPOSAL.md` — id · shape (from the menu) · **base sha** · files and line ranges touched · intent in one paragraph ·
  what is verified and how · what is **not** verified · window used · `status: complete|partial`.
- the **changed files** — not a patch. Copy each changed file whole into `proposed/`, preserving its path under the
  crate.
- `NOTES.md` — optional: what the next slice should be, and anything you had to guess.

**The relay builds the patch, not the author.** This is the one inversion that removes almost all of the pain, for both
GPT and Grok:

```
relay: git archive <base-sha> | tar -x -C base/          # the stamped tree handed to the author
author: edits base/, returns the changed files
relay: diff -ruN base/ proposed/ > change.patch           # the relay is the author of the diff
relay: git apply --check change.patch && git apply change.patch
```

So the author needs no git, no remotes, no branch names, and cannot be defeated by CRLF or trailing whitespace — his zip
and GPT's file drop become the *same* contract. A proposal is not a branch; it is a numbered, self-describing artifact
that survives the crash that ends the window.

## The receipt (the return channel — this is the half that makes it work)

After applying, the relay writes `RECEIPT.md` **into the same directory** and commits it:

- base sha · apply result · `cargo check -p <crate>` / `cargo test` / `rustfmt --check` output **verbatim and whole** —
  never a summary, because the compiler's words are the specification;
- the gate step that failed, the smallest next action, and either `LANDED <sha>` or `BLOCKED <reason>`;
- the relay's own fixes, itemised (`fmt`, a moved import, a renamed type) — the relay may finish a slice; it may **not**
  hide that it did.

The author's next window starts from **the receipt**, not from re-reading the repository. That is what turns an
interrupted worker into a resumable one, and it is why a timed-out window costs one slice instead of the whole night.

`docs/agent/proposals/LEDGER.md` gets one append-only row per attempt: date · id · shape · window used · outcome ·
relay minutes. Those columns are how the owner sizes the menu: if relay minutes creep, the slices are too big.

## What this does to rule 1

Rule 1 exists so that no work can hide. A proposal plus a receipt is **more** visible than a branch: the artifact is a
file, so it survives the crash; it names its own verification state instead of implying readiness; and the ledger
counts it. A branch can be forgotten. A proposal with no receipt is a visible open loop with an owner.

So when `main` is green behind its ratchets, rule 1 stops being load-bearing and the honest version — *work exists when
it is a proposal, a branch, or a commit on `main`* — can replace it. Until then this lane is how the remote models work.

## Anti-patterns this kills, each one from the record

- a branch stranded by a crash while everyone believed branches were forbidden (19 commits, 2026-10-02);
- "drops code and steps back" — unowned files in someone's worktree (257 uncommitted files in a detached
  `Culebraluxe-web-claude`) → *drops a proposal instead*;
- a dependency change without its lockfile;
- the zip → directory → `git patch` → hand-fix loop for one-off Grok work, replaced by one contract for every
  git-less worker;
- the last mile of integration attempted by a model that cannot run the build.

*Owner: this is a proposal. Rewrite it. When it is ratified, `AGENTS.md` points here and the relay script enforces the
`PROPOSAL.md`/`RECEIPT.md` shape.*
