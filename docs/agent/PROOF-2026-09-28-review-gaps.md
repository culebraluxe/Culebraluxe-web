# Proof — the review-gap pass, at `c08db41c` (2026-09-28)

Raw output, not a claim. Taken at `c08db41c` (origin/main) in the working checkout.

The gaps this closes were named in a 78/100 code review: CI red in ways nobody was looking at,
four security items, TypeScript still on the release path, over-long functions, and warnings.
What is **not** closed is stated at the bottom, because a proof that only lists wins is a press release.

## CI — both workflows green on `main`, for the first time in this pass

```
gates                        https://github.com/culebraluxe/Culebraluxe-web/actions/runs/36515824063  success  c08db41c
rust-client-cutover-check    https://github.com/culebraluxe/Culebraluxe-web/actions/runs/36515824055  success  c08db41c
```

Before this pass, `static gates` died on a useless regex escape and then on gitleaks (10 unread
false positives), and the Rust job's history-reader test could not see a `git log` because the
checkout was shallow. Each was fixed rather than skipped; see `6a03f802`, `93344aaf`, `d7756e46`.

## `cargo test -p db -p server -p forge -p workflow`

```
252 passed; 0 failed; 0 ignored     (across the four crates, --no-fail-fast, exit 0)
```

## `cargo check --workspace --all-targets`

Exit 0. The only warnings left are the engine's (`forge`), which is another agent's tree and is
deliberately excluded from the `-D warnings` gate rather than silenced:

```
warning: `forge` (lib) generated 8 warnings (run `cargo fix --lib -p forge` to apply 4 suggestions)
warning: `forge` (test "forge_runtime") generated 9 warnings (run `cargo fix --test "forge_runtime" -p forge` to apply 8 suggestions)
warning: `forge` (lib test) generated 7 warnings (7 duplicates)
```

## `pnpm ui:check` — the artifact the deploy ships

```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.19s
EXIT=0
```

## `pnpm broken:ts:sweep` — the dead-TS ledger

```
scanned                        : 250 files
cannot load                    : 172
loads, but lazy target gone     : 14
marked "// ⚠ BROKEN ON PURPOSE"    : 186

the tree and the inventory agree
```

## `cargo test -p cli --bin cli smoke`

```
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## `pnpm smoke:prod` — and the finding it produced

```
Production smoke — https://www.culebraluxe.com
  FAIL  build-info is a real build stamp
        status 404, unexpected body:
  ok    home page renders
        200, 1KB, marker "CulebraLuxe" present
  ok    buyers Yew shell renders
        200, Rust/Yew mount present
  ok    buyers production inventory loads
        200, 5 public listing(s)
  ok    property detail media contract is renderable by Rust
        200, hero nested in property record, 43 gallery image(s)
  FAIL  Rust API is ready on PROD database
        status 404, target <none>, not ready

smoke:prod — 4/6 checks passed, live sha unknown
```

**Two release-gate checks cannot pass, and the port did not cause it.** `/api/build-info` and
`/api/rust-ready` are Next-app routes that went with the port; the only surviving caller anywhere in
the repo is this smoke (the sole other references are a stale local `.vercel/output`). Confirmed with
plain `curl` so the verdict does not depend on code I wrote. It also means `--expect-head`, which
compares the live sha against HEAD, can never pass — the sha it reads comes from `/api/build-info`.
Fixing it is a production surface decision: give the Rust server a build stamp and a readiness
answer, or point the smoke at routes that exist. Recorded in `docs/agent/DEV-OPS-RELEASE.md` §5.

## What is NOT done in this pass

1. **Long functions (review item: over-long functions).** Still open — and the review's sizes do not
   reproduce. Measured at `c08db41c` by taking each `fn` from its signature to the next item at the
   same or lower indent (brace counting is not usable here: these bodies contain SQL literals with
   braces in them):

   ```
   683  rust/server/src/api/portal_bridge/forms_write.rs:6   forms_write
   379  rust/server/src/tech.rs:138                          command
   150  rust/core/domain/src/deal_portal.rs:107              derive_deal_health
   105  rust/server/src/signature/mod.rs:241                 send
    37  rust/server/src/forms/mod.rs:222                     update_instance
   ```

   Two are genuinely too big (`forms_write`, `tech::command`) and are not touched by this pass. The
   review's other three figures (439, 339, 336) are 4× to 12× the real sizes; the nearest function to
   "forms update" that exists (`update_instance`) is 37. A reviewer's number is evidence, not an order,
   and this one is wrong — so no refactor was scheduled against it.
2. **The rest of the live TypeScript.** `prod-smoke.ts` (the release path) is ported and deleted.
   Still live: `forge:silent-failure-gate` and `scripts/agent-scheduler.mjs`.
3. **Blanket `use super::*`.** Not cleaned. Measured, not guessed: **290 files** under `rust/` carry
   at least one (`grep -rl 'use super::\*' rust --include='*.rs'`), which is a bigger number than the
   review implied and the reason it was left for a pass of its own — each one needs its imports read
   before it is narrowed, and a mechanical rewrite of 290 files is not a review-gap fix.
4. **`server` and `cli` are excluded from the `-D warnings` CI step** because they link `forge`;
   that exclusion ends when the engine's 8 warnings do, and the exclusion is named in `gates.yml`
   rather than hidden.
5. **A stray `.gitleaksignore 2` file** (macOS duplicate, 16509 bytes, 2026-09-18) exists in the
   working directory. Reported, not deleted, because it is not mine and deleting a security config
   someone may be editing is a two-writer problem. `git status` stays clean because it is ignored.
