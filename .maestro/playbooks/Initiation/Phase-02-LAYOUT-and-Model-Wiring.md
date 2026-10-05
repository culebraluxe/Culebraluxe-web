# Phase 02: LAYOUT Update and Muse 1.3-contributor Model Wiring

This phase makes lane-muse a first-class lane in the machine layout and wires Meta Muse Spark 1.3-contributor as its default OpenCode model. It updates `docs/agent/LAYOUT.md` to include the fourth lane in the tree diagram and lane recipe, and it configures `opencode.json` (project-level) plus the global `~/.config/opencode/opencode.json` provider definition so VS Code OpenCode actually shows meta/muse-spark-1.3-contributor as selected for forge-smith. By the end `git worktree list` shows four lanes and `opencode models` lists your chosen model as available and configured.

## Tasks

- [ ] Update `docs/agent/LAYOUT.md` to include lane-muse:
  - Read current `docs/agent/LAYOUT.md` and locate the tree block (lines showing Culebraluxe-web, lane-gpt, lane-claude, lane-deep)
  - Edit the tree to add `lane-muse` fourth worktree: `├── lane-muse/           git worktree,    branch lane/muse` with same indentation and aligned spacing
  - In lanes section, ensure recipe `git worktree add ../lane-<name> -b lane/<name> origin/main` still accurate and mention 4 lanes now exist (gpt/claude/deep/muse)
  - If there is a list of open items mentioning only 3 lanes, update count to 4
  - Run `cat docs/agent/LAYOUT.md` and save verification to `Working/Phase-02-layout.txt`, grep for `lane-muse` count must be >=2

- [ ] Wire meta/muse-spark-1.3-contributor into project opencode.json:
  - Read `opencode.json` at repo root (`/Users/Shared/dev/src/lane-muse/opencode.json`) — it currently has 10 forge agents but no top-level model field
  - Add top-level `model` field with value `meta/muse-spark-1.3-contributor` at root of JSON, preserving existing `agent`, `compaction`, `snapshot` fields
  - Ensure JSON remains valid via `cat opencode.json | python3 -m json.tool > /dev/null` check
  - If project config has a `provider` section, merge meta provider definition: include `meta` provider with `npm: @ai-sdk/openai`, `name: Meta`, `baseURL: https://api.meta.ai/v1`, `apiKey: {env:MODEL_API_KEY}`, and models map containing `muse-spark-1.3` and `muse-spark-1.3-contributor` each with name, reasoning true, context 1048576, output 131072, modalities text/image/pdf/video in and text out, options reasoningEffort high, reasoningSummary auto, include encrypted_content
  - If project config has NO provider section (current state), add minimal provider block as described or rely on global config — but still set top-level model field
  - Save model wiring result to `Working/Phase-02-project-model.txt` with `grep -A2 '"model"' opencode.json`

- [ ] Wire global OpenCode config to expose 1.3-contributor model definition:
  - Read `~/.config/opencode/opencode.json` — currently defines only `muse-spark-1.1` under provider.meta.models
  - Merge in missing model definitions for `muse-spark-1.2`, `muse-spark-1.2-contributor`, `muse-spark-1.3`, `muse-spark-1.3-contributor` using same shape as existing 1.1 entry (name Muse Spark X.Y, reasoning true, limit context 1048576 output 131072, modalities input [text,image,pdf,video] output [text], options reasoningEffort high reasoningSummary auto include encrypted_content)
  - Update top-level global `model` field from `meta/muse-spark-1.1` to `meta/muse-spark-1.3-contributor` if file exists, otherwise leave project-level setting as authority
  - Validate global JSON with `python3 -m json.tool ~/.config/opencode/opencode.json > /dev/null` and list models via `opencode models 2>&1 | grep -E "muse-spark" | tee Working/Phase-02-global-models.txt`
  - Ensure `meta/muse-spark-1.3-contributor` appears in `opencode models` output

- [ ] Verify wiring produces correct git diff and cargo still compiles:
  - Run `cd /Users/Shared/dev/src/lane-muse && git diff docs/agent/LAYOUT.md` and save to `Working/Phase-02-layout-diff.txt`
  - Run `cd /Users/Shared/dev/src/lane-muse && git diff opencode.json` and save to `Working/Phase-02-model-diff.txt`
  - Run `cargo check -p workflow -p forge --all-targets 2>&1 | tail -10` to ensure LAYOUT change did not break anything and opencode.json is still readable by Cargo workspace
  - Write summary `Working/Phase-02-summary.txt` with: LAYOUT contains lane-muse (yes/no), project opencode.json model field value, global model field value, opencode models shows 1.3-contributor (yes/no), cargo check exit status
