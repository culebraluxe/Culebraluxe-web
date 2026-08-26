// Security hardening Story Board updates (HARDEN-01/02/04/06, AUTH-05/06).
// Creates missing stories; updates existing ones. No secrets in any note.
import {
  createStoryboardStory,
  updateStoryboardStory,
  type StoryboardStoryInput,
} from '../db/storyboard'

type Core = {
  id: string
  workstream: string
  title: string
  priority: string
  status: string
  notes: string
  completion: number
  rollup: boolean
  operatingSurface: string
}

function base(f: Core): Omit<StoryboardStoryInput, 'id'> {
  return {
    workstream: f.workstream,
    title: f.title,
    priority: f.priority,
    status: f.status,
    notes: f.notes,
    batch: null,
    goal: null,
    scope: null,
    dependencies: null,
    preconditions: null,
    architectBrief: null,
    contextRefs: null,
    acceptanceCriteria: null,
    postconditions: null,
    completion: f.completion,
    rollup: f.rollup,
    plannedStartAt: null,
    actualStartAt: null,
    completedAt: null,
    operatingSurface: f.operatingSurface,
  }
}

async function upsertStory(f: Core) {
  try {
    await createStoryboardStory({ id: f.id, ...base(f) })
    console.log('created', f.id)
  } catch {
    await updateStoryboardStory(f.id, base(f))
    console.log('updated', f.id)
  }
}

const WORKSTREAM = 'Security / Auth'

async function main() {
  await upsertStory({
    id: 'HARDEN-01',
    workstream: WORKSTREAM,
    title: 'Fail-Closed Portal Auth Bypass',
    priority: 'High',
    status: 'Complete',
    notes: 'Proven: the DEV bypass (PORTAL_AUTH_BYPASS) is never honored when NODE_ENV or APP_ENV is production — a production deployment with the flag set fails closed (denies), it does not open the portal. DEV (NODE_ENV=development) still bypasses. Also confirmed unauthenticated /portal requests are redirected by middleware + the authoritative server-side layout guard. Covered by workflow_app/tests/security-hardening.test.ts.',
    completion: 100,
    rollup: false,
    operatingSurface: 'OPS',
  })

  await upsertStory({
    id: 'HARDEN-04',
    workstream: WORKSTREAM,
    title: 'Fail Closed on Production Environment Mismatch',
    priority: 'High',
    status: 'Complete',
    notes: 'Proven: a production auth/environment mismatch (DEV bypass flag set in a production deployment) now fails closed instead of silently falling back to DEV auth. isPortalAuthBypass() denies in production. Covered by workflow_app/tests/security-hardening.test.ts.',
    completion: 100,
    rollup: false,
    operatingSurface: 'OPS',
  })

  await upsertStory({
    id: 'HARDEN-06',
    workstream: WORKSTREAM,
    title: 'Bound Uploads and Make Property Media Atomic',
    priority: 'High',
    status: 'Complete',
    notes: 'Confirmed + fixed: /api/media/upload had no size/MIME/name limits. Added shared lib/media/upload-policy.ts (50MB cap, allowed MIME families, filename/path sanitization) wired into /api/media/upload and /api/property-media/upload. Both routes remain auth-gated (listing.write) and fail closed before any write. Covered by workflow_app/tests/security-hardening.test.ts.',
    completion: 100,
    rollup: false,
    operatingSurface: 'OPS',
  })

  await upsertStory({
    id: 'HARDEN-02',
    workstream: WORKSTREAM,
    title: 'Close Public Media Authorization Leak',
    priority: 'High',
    status: 'In Progress',
    notes: 'Confirmed defect: /api/media/[id] and /api/media/documents/[id] serve any row from the canonical media table to anonymous callers by URL (no auth). Public listing images AND public property documents legitimately use these routes (components/property/property-documents.tsx), so blanket protection would break the public site. Correct fix requires classifying media (property-linked listing media = public; client/deal/admin = private). Smallest safe correction deferred to avoid breaking public property images/documents; a media classification seam is the pending fix.',
    completion: 20,
    rollup: false,
    operatingSurface: 'OPS',
  })

  await upsertStory({
    id: 'AUTH-06',
    workstream: WORKSTREAM,
    title: 'Capability Authorization at Business Command Seam',
    priority: 'High',
    status: 'Complete',
    notes: 'Already enforced: every Portal write action goes through portalWrite(authority, handler) -> runAuthorized(getPortalSessionAdapter(), authority, ...) which resolves the acting user and requires the exact authority BEFORE the canonical write. Verified across crm.write and deal.write actions. Prior "Planned" status was stale.',
    completion: 100,
    rollup: false,
    operatingSurface: 'OPS',
  })

  await upsertStory({
    id: 'AUTH-05',
    workstream: WORKSTREAM,
    title: 'Sensitive Administrative Write Audit',
    priority: 'High',
    status: 'Complete',
    notes: 'Implemented: actor is threaded into CommandReceipt.actorAppUserId for sensitive deal/offer/needs-review writes (db/deal-stage.ts, db/offer-acceptance.ts, db/needs-review-resolution.ts), and recordSecurityAuditEvent() records security-significant events (break-glass login, relationship-evidence review). Audit preserves actor/action/target/timestamp/receipt without logging private payload contents.',
    completion: 100,
    rollup: false,
    operatingSurface: 'OPS',
  })
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err)
    process.exit(1)
  })
