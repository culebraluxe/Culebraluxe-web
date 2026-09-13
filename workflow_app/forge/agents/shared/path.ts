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
