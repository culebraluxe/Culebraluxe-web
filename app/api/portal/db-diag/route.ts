import { NextResponse } from "next/server"

import { dbTargetInfo } from "@/db/database-gateway"
import { sql } from "@/db/client"
import { withApiHandler } from '@/lib/error-capture-seam'

// Safe, credential-free DB-target diagnostic. Reports the resolved database
// TARGET (prod/dev), the Vercel/APP env signals, the Neon host token (never the
// password/user/full URL), and — as proof of what this deployment would render —
// the live mv_client_directory and active-person counts on that target.
export const dynamic = "force-dynamic"

async function GETHandler() {
  const info = dbTargetInfo()
  let directoryCount: number | null = null
  let personCount: number | null = null
  let error: string | null = null
  try {
    const dir = await sql`select count(*)::int as n from mv_client_directory`
    directoryCount = Number((dir[0] as { n?: unknown } | undefined)?.n ?? 0)
    const persons = await sql`select count(*)::int as n from person where archived_at is null`
    personCount = Number((persons[0] as { n?: unknown } | undefined)?.n ?? 0)
  } catch (e) {
    error = e instanceof Error ? e.message.slice(0, 120) : "unknown"
  }
  return NextResponse.json({
    db: info,
    read: { directoryCount, personCount, error },
  })
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/portal/db-diag', route: '/api/portal/db-diag' },
  GETHandler,
)
