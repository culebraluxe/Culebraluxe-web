# unattended-path-fails-closed-on-git

<!-- GENERATED from forge_decision. Do not hand-edit: pnpm forge:decision --mirror -->

- status: active
- domain: forge
- source: captain
- owner: captain
- evidence: launchd wrapper: git pull --ff-only
- promoted: 2026-09-15T07:24:10.429Z
- supersedes: 

The unattended worker refuses to start unless the repository fast-forwards cleanly to origin main.
