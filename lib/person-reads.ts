import 'server-only'

import { rustApiSearchPeople } from '@/lib/rust-api/client'

export type PersonSearchResult = {
  id: string
  displayName: string
  role: string
  status: string
  location: string | null
  email: string | null
  phone: string | null
}

/**
 * Narrow operator search over canonical people. Rust owns authorization and
 * persistence; this module only preserves the caller-facing DTO.
 */
export async function searchPeople(query: string, limit = 8): Promise<PersonSearchResult[]> {
  const trimmed = query.trim()
  if (!trimmed) return []

  try {
    const rows = await rustApiSearchPeople(trimmed, limit)
    return rows.map((row) => ({
      id: row.id,
      displayName: row.display_name,
      role: row.role,
      status: row.status,
      location: row.location,
      email: row.email,
      phone: row.phone,
    }))
  } catch {
    // Preserve the historical failure-safe operator default.
    return []
  }
}
