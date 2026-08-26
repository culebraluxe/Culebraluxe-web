'use server'

import { revalidatePath } from 'next/cache'

import { getPortalSessionAdapter } from '@/lib/auth/portal-session'
import { runAuthorized } from '@/lib/auth/require-authority'
import { resolveIssue } from '@/db/issues'

// OPS-11A — Mark Resolved. Minimal operator write action, gated on the
// operational write authority (crm.write) before the business service runs.
// The issue table stays the only durable surface — no escalation machinery.
export async function resolveIssueAction(
  issueId: string,
): Promise<{ ok: boolean; message: string }> {
  return runAuthorized(getPortalSessionAdapter(), 'crm.write', async () => {
    try {
      const ok = await resolveIssue(issueId)
      revalidatePath('/portal/issues')
      return {
        ok,
        message: ok
          ? 'Issue resolved and removed from the open queue.'
          : 'Issue not found or already resolved.',
      }
    } catch {
      return { ok: false, message: 'Could not resolve the issue.' }
    }
  })
}
