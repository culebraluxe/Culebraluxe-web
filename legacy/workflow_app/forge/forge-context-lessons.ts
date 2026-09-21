// ---------------------------------------------------------------------------
// forge-context-lessons — the #8 feedback loop (claude-orchestrate headline rule):
// when a failure traces to MISSING CONTEXT, encode that context back so the same
// context is never missing twice. Logging a gap is not enough; a missing context
// must be RECORDED and INJECTED into future dispatches so the whole failure class
// is eliminated, not just repaired.
//
// Forge-native: a small durable lesson store (file-backed, no schema) + area
// matching + an instruction builder the Scout/Architect grounding can call so
// known context gaps ride along on future runs that touch the same area.
// ---------------------------------------------------------------------------

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname } from 'node:path'

export type ForgeContextLesson = {
  /** Stable id. */
  id: string
  /** Coarse area/seam tags this lesson applies to (e.g. "forge", "smith_split"). */
  area: string[]
  /** The encoded context, phrased as an instruction so it is never missing again. */
  missingFact: string
  /** Node/run that surfaced the gap (source of truth for the fix). */
  source: string
  /** ISO timestamp. */
  createdAt: string
}

export type NewContextLesson = Omit<ForgeContextLesson, 'id' | 'createdAt'>

export type ContextLessonStore = {
  list(): ForgeContextLesson[]
  add(lesson: NewContextLesson): ForgeContextLesson
  remove(id: string): void
}

export function newLessonId(prefix = 'ctx'): string {
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
}

/** In-memory store (for tests and pure composition). */
export class MemoryContextLessonStore implements ContextLessonStore {
  constructor(private lessons: ForgeContextLesson[] = []) {}
  list(): ForgeContextLesson[] {
    return [...this.lessons]
  }
  add(lesson: NewContextLesson): ForgeContextLesson {
    const full: ForgeContextLesson = { ...lesson, id: newLessonId(), createdAt: new Date().toISOString() }
    this.lessons.push(full)
    return full
  }
  remove(id: string): void {
    this.lessons = this.lessons.filter((l) => l.id !== id)
  }
}

/**
 * Durable, process-spanning store backed by a JSON file. The whole point of the
 * loop is persistence: a gap recorded in one run must survive to be injected into
 * the next. Default location is gitignored runtime state; callers may point it at
 * a curated repo path if they want lessons committed as durable engineering
 * knowledge.
 */
export class FileContextLessonStore implements ContextLessonStore {
  constructor(private readonly path: string) {}

  list(): ForgeContextLesson[] {
    if (!existsSync(this.path)) return []
    try {
      const parsed = JSON.parse(readFileSync(this.path, 'utf8'))
      return Array.isArray(parsed) ? (parsed as ForgeContextLesson[]) : []
    } catch {
      return []
    }
  }

  add(lesson: NewContextLesson): ForgeContextLesson {
    const full: ForgeContextLesson = { ...lesson, id: newLessonId(), createdAt: new Date().toISOString() }
    this.write([...this.list(), full])
    return full
  }

  remove(id: string): void {
    this.write(this.list().filter((l) => l.id !== id))
  }

  private write(lessons: ForgeContextLesson[]): void {
    mkdirSync(dirname(this.path), { recursive: true })
    writeFileSync(this.path, JSON.stringify(lessons, null, 2))
  }
}


/** Return the lessons whose area overlaps the run's area tokens (case-insensitive
 * substring match on either side). Empty tokens match nothing. */
export function lessonsForArea(
  lessons: ForgeContextLesson[],
  areaTokens: string[],
): ForgeContextLesson[] {
  const tokens = areaTokens.map((t) => t.toLowerCase()).filter(Boolean)
  if (tokens.length === 0) return []
  return lessons.filter((lesson) =>
    lesson.area.some((a) => {
      const al = a.toLowerCase()
      return tokens.some((t) => al.includes(t) || t.includes(al))
    }),
  )
}

/** Build the instruction block to inject known context gaps into a role's prompt.
 * Returns null when there is nothing to inject. */
export function buildContextLessonDirective(lessons: ForgeContextLesson[]): string | null {
  if (lessons.length === 0) return null
  const lines = lessons.map(
    (l) => `- [${l.area.join('/')}] ${l.missingFact}`,
  )
  return [
    'KNOWN CONTEXT GAPS (recorded by an earlier run so this context is never missing twice):',
    ...lines,
    'Treat each as authoritative context for this area.',
  ].join('\n')
}

