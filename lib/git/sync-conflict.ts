/**
 * CONFLICT COPIES — the debris a cloud-sync client leaves when it reconciles a file that changed while
 * it was uploading: `routes.d 2.ts`, `package 3.json`, `cache-life.d 2.ts`.
 *
 * MEASURED 2026-09-18 on this machine: 554 of them inside `.next` after one build, and one in the
 * repository itself (`contact-export 2.json`). The user believed iCloud was syncing this folder; it is
 * not iCloud. `~/Library/CloudStorage/OneDrive-Personal/` contains `Documents` AND `Desktop`, which is
 * the signature of **OneDrive Known Folder Move** — OneDrive took over `~/Documents`, so every file
 * `pnpm build`, `next dev` and git itself write gets uploaded, and anything that changes mid-upload comes
 * back as a numbered sibling.
 *
 * THE NAME ALONE IS NOT ENOUGH TO DELETE SOMETHING, which is why the two functions are separate. A name
 * pattern of "space + number + extension" also matches `CHANGELOG 2026.md` and `Budget 2026.xlsx`, and
 * the first version of this rule would have let `pnpm health --fix` delete a person's file — a fence
 * caught exactly that case (2026-09-18). Two things make a numbered sibling debris rather than a
 * document: the counter is SMALL (a sync client counts from 2, a year is four digits), and **the
 * original sits beside it in the same directory**. Both are required.
 *
 * Residual risk, stated rather than hidden: `Report.md` next to `Report 2.md`, both written by a person,
 * would still be read as debris. It is rare, `--fix` is deliberate, and the tool prints every path it
 * removes — but it is why `--fix` is not the default.
 */

/** Candidate by name only: a SMALL counter, so a four-digit year is never one. */
export function isConflictCopyName(name: string): boolean {
  return / [0-9]{1,2}\.[A-Za-z0-9]+$/.test(name)
}

/** The name this would be a copy OF, or null when it is not a numbered sibling. */
export function conflictCopyBaseName(name: string): string | null {
  const match = /^(.+) [0-9]{1,2}(\.[A-Za-z0-9]+)$/.exec(name)
  return match ? `${match[1]}${match[2]}` : null
}

/** Names in ONE directory that are debris: small counter AND the original present beside them. */
export function conflictCopiesInDirectory(names: readonly string[]): string[] {
  const present = new Set(names)
  return names
    .filter((name) => {
      const base = conflictCopyBaseName(name)
      return base !== null && present.has(base)
    })
    .sort()
}

/** Paths from a scan that are debris, judged per directory (never across directories). */
export function conflictCopies(paths: readonly string[]): string[] {
  const byDir = new Map<string, string[]>()
  for (const path of paths) {
    const cut = path.lastIndexOf('/')
    const dir = cut >= 0 ? path.slice(0, cut) : ''
    const name = cut >= 0 ? path.slice(cut + 1) : path
    byDir.set(dir, [...(byDir.get(dir) ?? []), name])
  }
  const out: string[] = []
  for (const [dir, names] of byDir) {
    for (const name of conflictCopiesInDirectory(names)) out.push(dir ? `${dir}/${name}` : name)
  }
  return out.sort()
}
