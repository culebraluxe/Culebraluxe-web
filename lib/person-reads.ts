import 'server-only'

import { randomUUID } from 'node:crypto'

import { coreServices } from '@/lib/service-runtime'
import { PERSON_OPERATIONS } from '@/services/person'
import type { PersonSearchResult } from '@/services/person'

/**
 * Person reads for server surfaces — through the Person SERVICE, never the
 * database. (The retired db/people.ts was a Person read living outside the
 * Person domain.)
 */
export type { PersonSearchResult }

const service = coreServices.person

/**
 * Narrow operator search over canonical people: exact substring on display name
 * or a recorded identity value. Not identity resolution, and no fuzzy matching.
 * A failed read returns [] — the caller never sees a rejected query.
 */
export async function searchPeople(query: string, limit = 8): Promise<PersonSearchResult[]> {
  const trimmed = query.trim()
  if (!trimmed) return []

  const result = await service.execute({
    operation: PERSON_OPERATIONS.SEARCH,
    payload: { query: trimmed, limit },
    context: {
      actor: { id: null, kind: 'system' },
      correlationId: randomUUID(),
    },
  })

  // The kernel captured any failure; an empty result is the safe operator default.
  return result.ok ? result.value : []
}
