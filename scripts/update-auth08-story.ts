// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The TypeScript engine and its libraries were deleted in the 2026-09 Rust port, so this
// file cannot load: the modules it imports from are gone. It is kept as REFERENCE ONLY,
// so the behaviour it describes can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
// AUTH-08 — record the Google OAuth config closure on the Story Board.
import { createStoryboardStory, updateStoryboardStory } from '@/legacy/db/storyboard'

const id = 'AUTH-08'
const notes = `Root cause: AUTH_GOOGLE_ID / AUTH_GOOGLE_SECRET are absent from .env.local (only AUTH_SECRET is set), so the default Google provider has no client credentials and Auth.js fails closed with a generic Configuration error. Code contract is correct (auth.ts -> provider-config.ts reads AUTH_GOOGLE_ID / AUTH_GOOGLE_SECRET). Added: (1) explicit DEV diagnostic log naming the missing vars when Google creds absent; (2) docs/auth-google-setup.md with exact Google Cloud Web-client settings (origin http://localhost:3000, redirect http://localhost:3000/api/auth/callback/google, env vars); (3) .env.example auth contract; (4) tests for provider-config + identity resolution. Canonical mapping seam exists (db/manual v5_link_auth_identity.sql: provider='google' + sub -> active app_user, no email guessing). LIVE Google login NOT yet proven (requires real Google OAuth client credentials, which Cline cannot invent). Status In Progress until Chris clicks Google, selects identity, callback succeeds, and app_user resolves.`

async function main() {
  const fields = {
    id,
    workstream: 'Security / Auth',
    title: 'Google OAuth Local + Production Configuration Closure',
    priority: 'High',
    status: 'In Progress',
    notes,
    batch: null,
    goal: null,
    scope: null,
    dependencies: null,
    preconditions: null,
    architectBrief: null,
    contextRefs: null,
    acceptanceCriteria: null,
    postconditions: null,
    completion: 60,
    rollup: false,
    plannedStartAt: null,
    actualStartAt: null,
    completedAt: null,
    operatingSurface: 'OPS',
  }
  try {
    await createStoryboardStory(fields)
    console.log('created', id)
  } catch {
    await updateStoryboardStory(id, fields)
    console.log('updated', id)
  }
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err)
    process.exit(1)
  })
