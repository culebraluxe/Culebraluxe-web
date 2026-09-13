/** Repository-relative path helpers. File grain is the lock; `#symbol` is documentation. */

export function fileOf(scope: string): string | null {
  const p = scope.trim().split('#')[0].replace(/^\.\//, '').replace(/\/+$/, '')
  if (!p || p.startsWith('/')) return null
  if (/[\s*?[\]{}:]/.test(p)) return null
  if (p.split('/').some((s) => !s || s === '.' || s === '..')) return null
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
