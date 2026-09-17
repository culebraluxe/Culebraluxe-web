/** Repository-relative path helpers. File grain is the lock; `#symbol` is documentation. */

/**
 * A Next.js dynamic-route segment: `[id]`, `[...slug]`, `[[...slug]]`. A bracket in this repo is a
 * directory NAME, not a glob — 26 tracked paths look like this, and one of them is
 * `app/api/media/documents/[id]/route.ts`.
 *
 * The parameter name excludes `-` on purpose: `[a-z]` is how a glob character class looks, and every
 * dynamic segment in this repo is identifier-shaped (`[id]`, `[personId]`, `[...nextauth]`). So `[a-z]`
 * stays a PATTERN and is refused, while a real route is a path.
 */
const DYNAMIC_SEGMENT = /^\[{1,2}(\.\.\.)?[A-Za-z0-9_]+\]{1,2}$/

export function fileOf(scope: string): string | null {
  const p = scope.trim().split('#')[0].replace(/^\.\//, '').replace(/\/+$/, '')
  if (!p || p.startsWith('/')) return null
  // Glob syntax is not a file. Brackets used to be refused here as if they were a character class,
  // which made every dynamic route an ILLEGAL SEAM: on 2026-09-17 the architect filed a finding against
  // app/api/media/documents/[id]/route.ts — the very path the story's scope declared — and the lane HOLDed
  // twice on `illegal scope path`. Brackets are now legal when the segment is a dynamic route, so a glob
  // class (`[a-z]`) is still refused while `[id]` is a path. Matching below stays literal: `[id]` means the
  // `[id]` directory, never "any id".
  if (/[\s*?{}:\\]/.test(p)) return null
  if (p.split('/').some((s) => !s || s === '.' || s === '..')) return null
  for (const segment of p.split('/')) {
    if (!segment.includes('[') && !segment.includes(']')) continue
    if (!DYNAMIC_SEGMENT.test(segment)) return null
  }
  return p
}

export function within(path: string, area: string): boolean {
  return path === area || path.startsWith(`${area}/`)
}

export function overlap(a: string, b: string): boolean {
  return within(a, b) || within(b, a)
}

export function unique(values: string[]): boolean {
  return new Set(values).size === values.length
}

/**
 * Is `filePath` inside the allowed set and outside the prohibited set?
 *
 * Used by the Smith scope lock: a diff that touches anything outside the accepted
 * assignment is a MISS, and a sibling's surface is prohibited, not merely unlisted.
 */
export function pathAllowed(filePath: string, allowed: string[], prohibited: string[] = []): boolean {
  const file = fileOf(filePath)
  if (!file) return false
  if (
    prohibited.some((raw) => {
      const p = fileOf(raw)
      return p ? file === p || file.startsWith(`${p}/`) : false
    })
  ) {
    return false
  }
  if (allowed.length === 0) return false
  return allowed.some((raw) => {
    const p = fileOf(raw)
    return p ? file === p || file.startsWith(`${p}/`) : false
  })
}
