import { NextRequest, NextResponse } from 'next/server'

import { getPortalActingUser } from '@/lib/auth/portal-session'
import { AuthError } from '@/lib/auth/errors'
import { rustApiRead, rustApiTechCockpit } from '@/lib/rust-api/client'
import { withApiHandler } from '@/lib/error-capture-seam'

export const dynamic = 'force-dynamic'

type RustUiRow = { id: string; cells: string[]; badge?: string | null }

function primitiveCells(item: unknown): string[] {
  if (item === null || item === undefined) return []
  if (typeof item !== 'object') return [String(item)]
  return Object.values(item as Record<string, unknown>).flatMap((entry) => {
    if (entry === null || entry === undefined) return []
    if (typeof entry === 'object') {
      return [Array.isArray(entry) ? `${entry.length}` : '']
    }
    return [String(entry)]
  })
}

function factRowsFrom(value: unknown): RustUiRow[] {
  if (value === null || value === undefined) return []
  if (Array.isArray(value)) {
    return value.flatMap((item, index) => {
      const record =
        item && typeof item === 'object'
          ? (item as Record<string, unknown>)
          : null
      const id =
        record && (record.id ?? record.key ?? record.slug)
          ? String(record.id ?? record.key ?? record.slug)
          : `item-${index}`
      const cells = primitiveCells(item)
      return cells.length ? [{ id, cells }] : []
    })
  }
  return Object.entries(value as Record<string, unknown>)
    .filter(([, entry]) =>
      entry === null ||
      Array.isArray(entry) ||
      ['string', 'number', 'boolean'].includes(typeof entry),
    )
    .map(([key, entry]) => ({
      id: key,
      cells: [
        key
          .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
          .replace(/[_-]/g, ' ')
          .replace(/^./, (character) => character.toUpperCase()),
        Array.isArray(entry) ? `${entry.length}` : String(entry ?? '—'),
      ],
      badge: Array.isArray(entry) ? 'count' : undefined,
    }))
}

async function rowsFor(screen: string): Promise<RustUiRow[]> {
  switch (screen) {
    case 'issues': {
      const page = await rustApiRead<{ rows: unknown[] }>(
        '/v1/issues?scope=OPERATIONS_EXCEPTION&state=OPEN&page=1&pageSize=50' as `/v1/${string}`,
      )
      return factRowsFrom(page.value.rows)
    }
    case 'portal-root': {
      const result = await rustApiRead<unknown>('/v1/cockpit')
      return factRowsFrom(result.value)
    }
    case 'client-admin': {
      const result = await rustApiRead<{ rows: unknown[] }>(
        '/v1/clients?page=1&pageSize=50&sort=name' as `/v1/${string}`,
      )
      return factRowsFrom(result.value.rows)
    }
    case 'media-admin': {
      const result = await rustApiRead<{ rows: unknown[] }>(
        '/v1/properties/admin?page=1&pageSize=50' as `/v1/${string}`,
      )
      return factRowsFrom(result.value.rows)
    }
    case 'settings-roles':
    case 'settings-authorities': {
      const result = await rustApiRead<unknown>('/v1/security/role-entitlements')
      return factRowsFrom(result.value)
    }
    case 'tech-app-errors':
    case 'identity-quality':
    case 'reporting':
    case 'system-health': {
      const result = await rustApiRead<unknown>('/v1/support/system-health')
      return factRowsFrom(result.value)
    }
    case 'command-console':
    case 'tech-runs':
    case 'tech-kanban':
    case 'tech-line':
    case 'command-center': {
      const snapshot = await rustApiTechCockpit()
      const pick =
        screen === 'tech-runs'
          ? snapshot?.recentFlights
          : screen === 'tech-line'
            ? snapshot?.activeWork
            : screen === 'tech-kanban'
              ? snapshot?.queuedCards
              : snapshot
      return factRowsFrom(pick ?? [])
    }
    case 'marketing':
    case 'marketing-syndication': {
      const result = await rustApiRead<unknown>('/v1/public/listing-copy')
      return factRowsFrom(result.value)
    }
    case 'attention':
    case 'needs-review': {
      const result = await rustApiRead<unknown>('/v1/cockpit')
      return factRowsFrom(result.value)
    }
    case 'whatsapp-coexistence': {
      const result = await rustApiRead<unknown>('/v1/support/system-health')
      return factRowsFrom(result.value)
    }
    // These older generic surfaces do not yet have a dedicated Rust read.
    // Returning no rows is the route's established contract for an unwired
    // screen; importantly, it no longer falls back into the legacy database.
    case 'showings':
      return []
    default:
      return []
  }
}

async function GETHandler(req: NextRequest): Promise<Response> {
  try {
    await getPortalActingUser()
  } catch (error) {
    if (error instanceof AuthError) {
      return NextResponse.json({ error: 'Unauthorized.' }, { status: 401 })
    }
    throw error
  }

  const screen = req.nextUrl.searchParams.get('screen') ?? ''
  return NextResponse.json(await rowsFor(screen))
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/rows', route: '/api/portal/rust-ui/rows' },
  GETHandler,
)
