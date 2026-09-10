# Projects MVI deployment handoff

This bundle contains the local Projects MVI implementation. It includes MVI-01 truthful Work Plan reads and MVI-02 ProjectService-owned versioned playbook instantiation, plus the New Project flow, editable WBS inspector, Calendar/ Documents/Activity projections, and Pane 2 redesign.

## Deploy to DEV

1. Install dependencies with the repository's pinned pnpm version.
2. Apply `db/migrations/141_project_playbook_identity.sql` to the DEV Neon branch (`br-solitary-star-axgusezm`, database `neondb`). Review before applying.
3. Run `APP_ENV=development node --env-file=.env.local --import tsx scripts/seed-projects-mvi2-dev.ts`.
4. Start the app with `pnpm dev` and open `/portal/projects`.
5. Verify create project, select WBS, edit/save, complete/dismiss, documents, activity, and refresh persistence.

## Remaining work

- Apply and verify migration/seed in DEV; then promote through the normal reviewed process.
- Browser QA against real authenticated data.
- Replace free-text assignee with canonical app-user picker.
- Add typed Contract/Activity/Take Action routing.
- Complete Timeline and Financials projections using existing readers.
- Final responsive/visual polish.

No Git metadata, dependencies, build output, environment files, or secrets are included.
