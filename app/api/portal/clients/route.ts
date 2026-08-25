import { NextRequest, NextResponse } from "next/server"

import { getClientsPage } from "@/db/clients"
import { getClientAdminPage } from "@/db/client-admin"

// ---------------------------------------------------------------------------
// CLIENTS — server-side pagination over the canonical `person` parent.
//
// The primary Clients screen pages the canonical/parent dataset (person), NOT
// the L/ODS staging tables. This endpoint returns only the current page
// (50 default), with search / filters / sort applied in SQL and a separate
// COUNT(*) for the total. `view=directory` is the primary clients list;
// `view=admin` is the read-only Client Administration projection.
// ---------------------------------------------------------------------------

const VALID_STATUS = ["new", "warm", "active", "referral"]
const VALID_ROLE = ["buyer", "seller", "both"]
const VALID_SORTS = ["name", "created", "recent"]

function intParam(value: string | null, fallback: number, min: number, max: number) {
  const parsed = parseInt(value ?? "", 10)
  if (Number.isNaN(parsed)) return fallback
  return Math.max(min, Math.min(max, parsed))
}

export async function GET(req: NextRequest) {
  const params = req.nextUrl.searchParams
  const view = params.get("view") ?? "directory"
  const search = params.get("search") ?? ""
  const page = intParam(params.get("page"), 1, 1, Number.MAX_SAFE_INTEGER)
  const pageSize = intParam(params.get("pageSize"), 50, 1, 50)

  try {
    if (view === "admin") {
      const result = await getClientAdminPage({ search, page, pageSize })
      return NextResponse.json(result)
    }

    const status = VALID_STATUS.includes(params.get("status") ?? "")
      ? (params.get("status") as string)
      : undefined
    const role = VALID_ROLE.includes(params.get("role") ?? "")
      ? (params.get("role") as string)
      : undefined
    const sort = VALID_SORTS.includes(params.get("sort") ?? "")
      ? (params.get("sort") as string)
      : "name"

    const result = await getClientsPage({ search, status, role, sort, page, pageSize })
    return NextResponse.json(result)
  } catch {
    // Canonical clients seam unavailable -> empty page (safe).
    return NextResponse.json({ rows: [], total: 0, page, pageSize })
  }
}
