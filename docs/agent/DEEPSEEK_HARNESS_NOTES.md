# DeepSeek Harness Adapter — Implementation Notes (ENG-19)

This document records the **factual** findings from inspecting the actual
installed DeepSeek Harness (`/Users/lisapenfieldicloud.com/Documents/deepseek-harness`,
package `@deepseek-ai/dsh@0.1.1-rc.2`, source checkout at commit present on
2026-08-21). Every claim below was verified from source or by running the
harness. Nothing is invented to make the adapter contract look green.

## 1. START / INVOCATION

- The harness exposes a one-shot headless CLI:
  `dsh --profile headless "<task>"` (bin: `lib/bin.js`).
- The invoking directory is the default workspace root; the headless runner
  creates one fresh `Agent` with `cwd = process.cwd()`.
- Required config: `DEEPSEEK_API_KEY` (env or `$DSH_HOME/.credentials.yaml`);
  optional `DEEPSEEK_BASE_URL` (defaults to the public API).
- The headless profile auto-initializes on first use from shipped templates.
- Verified: `node ~/.dsh/profiles/node_modules/@deepseek-ai/dsh/lib/bin.js
  --profile headless "Reply with exactly one word: OK"` printed `OK` and exited 0.

## 2. RUN / SESSION ID

- Each headless run creates `session-<uuid>` **internally** (`randomUUID()` in
  `packages/bundle/headless/src/index.ts`).
- It is NOT printed to stdout. It is persisted as a directory:
  `$DSH_HOME/sessions/<projectKey>/session-<uuid>/session.jsonl.zstd`.
- `projectKey` escaping (verified in `session-persistence-jsonl/src/format.ts`):
  separators `/ \ :` → single `-` (runs collapsed), leading `-` stripped,
  other unsafe code units → `~XXXX`, wrapped in `--...--`.
- The parent discovers the id by listing the newest `session-*` directory for
  the workspace (before/after diff or newest mtime). Stable across process
  restart because it is a filesystem directory. Stored opaquely as
  `agent_work_item.external_run_id`.

## 3. STATUS

- Headless mode is **blocking**: one run to completion, then process exit.
  There is no mid-run polling API for the headless profile.
- Status is derived from child-process liveness (`exitCode === null` = running;
  exit = terminal). Exit code 0 = `completed`, 1 = error.
- Canonical mapping: alive → `running`; exit 0 → `success`; exit non-zero →
  `failed`; SIGTERM requested → `cancelled`.

## 4. PAUSE

- No native pause in headless mode.
- Semantics-preserving wrapper used: SIGSTOP on the child process. The process
  and its session survive; the run simply suspends. Honest and reversible.

## 5. RESUME

- No native resume API. SIGCONT continues the **same** process/session.
- Not reconstructed from persisted session state (the headless CLI always
  creates a fresh session on each invocation); resume only makes sense for the
  same live process.

## 6. CANCEL

- SIGTERM the child process. The partial session JSONL (if flushed) remains on
  disk. Canonical state (`Cancelled`) is recorded by CulebraLuxe; the harness
  session is never canonical truth.

## 7. HEARTBEAT / LIVENESS

- The harness exposes no headless progress API. CulebraLuxe's lease/heartbeat
  (work-item `updated_at` refreshed by the shared execute loop) remains
  authoritative.

## 8. WORKSPACE

- The harness agent operates headlessly in the invocation cwd (the CulebraLuxe
  repo). It has bash + filesystem tools and can run git/shell commands,
  edit files, run tests/typecheck/build, and create local commits. Operates
  fully outside VS Code/Cline (the `headless` profile mounts no Host/HTTP/browser).

## 9. MODEL / PROVIDER CONFIG

- Model selection is configured below the adapter boundary (default model via
  `$DSH_HOME/settings.yaml` / `dsh-agent-default-model`; example rosters use
  `deepseek-v4-pro` / `deepseek-v4-flash`). The adapter config resolves the
  LOGICAL `modelProfile` → provider/model; no vendor id ever appears in the
  command/Story Board vocabulary.

## 10. RESULT / EVIDENCE

- stdout = final assistant text; stderr = harness errors; exit code = 0/1.
- Session transcript = JSONL (zstd) in the session directory.
- The adapter normalizes stdout/exit/session dir into `storyboard_story_run`
  evidence; the model-created commit is read from `git log -1` (factual HEAD).

## 11. RESTART BEHAVIOR

- An existing session can be rediscovered by scanning
  `$DSH_HOME/sessions/<projectKey>/session-*` using the persisted
  `external_run_id`. Ephemeral state (in-memory stdout, live child) is not
  recoverable; canonical state always lives in CulebraLuxe tables.

## 12. ERROR / EXIT SEMANTICS

- exit 0 = completed; exit 1 = error; spawn errors report via stderr/`dsh:`.
- Timeout is controlled by the parent (bash tool timeout in the harness config,
  `timeoutMs`); the parent may also SIGTERM.

## Impedance mismatches / unsupported primitives

- **No native pause/resume** — handled with honest SIGSTOP/SIGCONT wrappers.
- **No mid-run status polling** — status is process liveness.
- **No session-id output** — id discovered from the session directory.
- **No resume-by-session-id** — each headless invocation is a fresh session.
  Resume means continuing the same live process, not replaying a stored session.

These are documented, not hidden; the CulebraLuxe canonical lifecycle always wins.
