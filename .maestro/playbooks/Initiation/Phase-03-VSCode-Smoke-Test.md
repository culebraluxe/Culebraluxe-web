# Phase 03: VS Code OpenCode Smoke Test and Forge Doctor

This phase proves lane-muse is visible to VS Code's OpenCode extension and that the Forge control plane understands the lane is healthy. It runs the T0 gate (`cargo check -p workflow`), queries `forge:doctor` and `forge:board` for a snapshot without touching PROD, and confirms the Muse model wiring from Phase 02 is picked up by OpenCode. By the end you can open VS Code, see lane-muse in the OpenCode sidebar, and know cargo + forge plumbing are green — the hello check you wanted before any real story work.

## Tasks

- [ ] Verify OpenCode sees lane-muse and Muse model:
  - Run `cd /Users/Shared/dev/src/lane-muse && opencode models 2>&1 | grep -E "muse-spark-1.3"` and save to `Working/Phase-03-models.txt`
  - Run `cat ~/.config/opencode/opencode.json | python3 -c "import json,sys; cfg=json.load(sys.stdin); print('global model:', cfg.get('model')); print('meta models:', list(cfg.get('provider',{}).get('meta',{}).get('models',{}).keys()))"` and save to `Working/Phase-03-global-config.txt`
  - Run `cat opencode.json | python3 -c "import json; cfg=json.load(open('opencode.json')); print('project model:', cfg.get('model')); print('agents:', list(cfg.get('agent',{}).keys())[:5])"` and save to `Working/Phase-03-project-config.txt`
  - Check VS Code extension presence: `ls -la ~/.vscode/extensions/ | grep -i opencode 2>&1; ls -la ~/Library/Application\ Support/Code/User/globalStorage/ 2>&1 | grep -i opencode | head` and save to `Working/Phase-03-vscode.txt`
  - Assert project model equals `meta/muse-spark-1.3-contributor` and models list contains that id

- [ ] Run T0 and scoped gates to prove lane-muse builds:
  - Execute `cd /Users/Shared/dev/src/lane-muse && cargo check -p workflow --all-targets 2>&1 | tail -30` and save to `Working/Phase-03-cargo-check-workflow.txt`
  - Execute `cd /Users/Shared/dev/src/lane-muse && cargo check -p forge --all-targets 2>&1 | tail -30` and save to `Working/Phase-03-cargo-check-forge.txt`
  - Execute `cd /Users/Shared/dev/src/lane-muse && pnpm ui:check 2>&1 | tail -20` if web/ui exists, otherwise skip with note to `Working/Phase-03-ui-check.txt` — this should be T0 wasm check that proves workspace still healthy after LAYOUT edit
  - Run `cd /Users/Shared/dev/src/lane-muse && git diff --check` and save to `Working/Phase-03-diff-check.txt` — must be clean (no whitespace errors)

- [ ] Snapshot Forge control plane without mutating PROD:
  - Run `cd /Users/Shared/dev/src/lane-muse && pnpm forge:doctor 2>&1 | tee Working/Phase-03-doctor.txt` — capture board status agreement, open work items, active claims
  - Run `cd /Users/Shared/dev/src/lane-muse && pnpm forge:board 2>&1 | head -100 | tee Working/Phase-03-board.txt` filtered for any Running stories — prove no stale claims from Deep copy remain active for lane-muse
  - Check local filesystem for stale forge worktrees: `ls -la /private/var/folders/m7/*/T/culebraluxe-forge-worktrees/ 2>&1 | head -30 || ls -la $TMPDIR/culebraluxe-forge-worktrees/ 2>&1 | head -30 || echo "no tmp worktrees at default path"` and save to `Working/Phase-03-worktrees.txt`
  - Verify `git log origin/main --oneline -5` matches `git log --oneline -5` on lane/muse after Phase 01 fix (should be same unless you committed LAYOUT)

- [ ] Create structured smoke test report:
  - Write `docs/triage/Lane-Muse-Smoke-2026-10-04.md` with YAML front matter: type: report, title: Lane-Muse Smoke Test, created: 2026-10-04, tags: [lane-muse, meta, muse-spark, forge, smoke, vscode], related: [[LAYOUT]] [[Forge-Doctor]]
  - Report must include: git tree proof (branch lane/muse, base sha, worktree list line), model wiring proof (project model value, global model value, opencode models shows 1.3-contributor), T0 proofs (cargo check workflow/forge exit codes), forge doctor snapshot (open engine tasks, active claims, board agreement), VS Code OpenCode extension presence, and path to Working/ evidence files
  - Include section ## What This Proves explaining that lane-muse is now a valid fourth lane, model 1.3-contributor is selectable, and no PROD mutation occurred
  - Run `pnpm forge:packet-lint 2>&1 | tail -10` to ensure new triage report has no broken path references and save to `Working/Phase-03-packet-lint.txt`
