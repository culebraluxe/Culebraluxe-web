import { NextRequest, NextResponse } from "next/server"
import { getImportedContacts } from "@/db/imported-contacts"

// ---------------------------------------------------------------------------
// SUPPORT-2 — Imported Contacts (Apple Contacts load projection) search/pages.
// Server-side search + pagination so the browser never receives the full
// 2,573-row payload. These are LOAD rows, never canonical Clients.
// ---------------------------------------------------------------------------

export async function GET(req: NextRequest) {
  const params = req.nextUrl.searchParams
  const search = params.get("search") ?? ""
  const page = Math.max(1, parseInt(params.get("page") ?? "1", 10) || 1)
  const pageSize = Math.max(1, Math.min(50, parseInt(params.get("pageSize") ?? "25", 10) || 25))

  const result = await getImportedContacts({ search, page, pageSize })
  return NextResponse.json(result)
}
