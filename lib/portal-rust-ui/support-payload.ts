import 'server-only'

import { getClients } from '@/legacy/db/clients'

// ---------------------------------------------------------------------------
// SUPPORT PAYLOADS — the four diagnostic screens' reads, in one place.
//
// WHERE THE PROJECTION LIVES, AND WHY IT STAYS THERE. These screens' numbers come from reads that already exist and already
// define what the numbers MEAN: `legacy/db/clients.ts`, `legacy/db/auth-status.ts`, `lib/auth/break-glass-readiness.ts`,
// `legacy/db/system-health.ts` and the diagnostics module. This module calls them rather than re-deriving their SQL in the
// Rust service, because a second copy of a projection is a second answer — and the way to tell which one is wrong is a
// reconciliation, months later. What the Rust service owns for SUPPORT is the screens: their DTOs, their state, their
// effects, their rendering.
//
// WHAT DOES NOT TRAVEL. A diagnostic screen is the easiest place in an application to leak a secret by accident, because it
// is the screen that talks about configuration. These payloads carry boolean posture and operational counts. No tokens, no
// credential values, no connection strings, no hashes, no environment URLs — the WhatsApp token and the break-glass secret
// stay on this side of the boundary and are represented only by "configured" or "missing".
//
// NO SAMPLE DATA. Every field comes from a read. A screen whose read fails says so; it does not fall back to a plausible
// looking example, because a diagnostic that invents its own numbers is worse than one that is visibly broken.
// ---------------------------------------------------------------------------

/** The four SUPPORT screens this module serves, exactly as the registry keys them. */
export const SUPPORT_SCREENS = [
  'system-health',
  'db-test',
  'whatsapp-meta',
  'security',
] as const

export type SupportScreen = (typeof SUPPORT_SCREENS)[number]

export function isSupportScreen(value: string): value is SupportScreen {
  return (SUPPORT_SCREENS as readonly string[]).includes(value)
}

/** One client as the DB Test screen shows it: who it is, and how to reach them. Nothing about their preferences. */
export type SupportDbTestClient = {
  id: string
  displayName: string
  role: string
  status: string
  email: string | null
  phone: string | null
}

export type SupportDbTest = {
  /** Was the database reachable and the read answered? See `supportPayload`: it is true when the read returns at all. */
  connected: boolean
  clientCount: number
  clients: SupportDbTestClient[]
}

/**
 * The payload one SUPPORT screen needs, read from the projections that already define it.
 *
 * A screen is served what it renders and nothing else, and a read that fails throws: the bridge's error handling turns that
 * into an explicit failure state on the screen rather than an empty success.
 */
export async function supportPayload(
  screen: SupportScreen,
): Promise<Record<string, unknown>> {
  switch (screen) {
    case 'db-test': {
      // THE PRE-CUTOVER SCREEN, PORTED. It read `getClients()` and showed `connected`, `clientCount` and the rows. The read
      // is the same one; what changes is the shape: a diagnostic carries the identity columns it prints and not the CRM
      // fields it never showed.
      const clients = await getClients()
      const dbTest: SupportDbTest = {
        // Reaching this line IS the connectivity answer: a database that could not be reached would have thrown here, and
        // the screen would be showing the failure instead. `true` is not a hopeful constant.
        connected: true,
        clientCount: clients.length,
        clients: clients.map((client) => ({
          id: client.id,
          displayName: client.displayName,
          role: client.role,
          status: client.status,
          email: client.email ?? null,
          phone: client.phone ?? null,
        })),
      }
      return { support: { dbTest } }
    }
    case 'system-health':
    case 'whatsapp-meta':
    case 'security': {
      // NOT YET PORTED. An explicit refusal rather than an empty object: a screen whose payload is silently empty renders as
      // a screen with nothing to show, which reads as "the database is empty" rather than "this is not built yet".
      throw new Error(`The '${screen}' payload is not wired yet.`)
    }
  }
}
