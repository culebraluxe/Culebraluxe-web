// ---------------------------------------------------------------------------
// SCOPE MANIFEST — "here are the exact files to read for this scope", generated
// and gate-able, instead of "here is what grep happened to match".
//
// Pirated from OpenContext's `oc context manifest` (0xranx/OpenContext, MIT) and
// rebuilt in our shape. What we did NOT copy is the part that made theirs weak:
// their query is `ORDER BY rel_path`, so the manifest is a directory listing and
// its only signal is the one-line description someone remembered to write. We
// measured that against our corpus on 2026-09-15:
//
//   grep -ril "how does a story move get written to the database" docs/agent
//     -> 124 of 133 files, alphabetically (so the first five are arbitrary)
//   the same query ranked by structural signals
//     -> the packet, the paths it cites, the files the story's own commits touched
//
// Lanes are STRUCTURAL FIRST, because we have signals a document store cannot
// have: a packet that cites paths, git commits that name the story, and story
// state that lives in Neon. Lexical ranking is the second lane and runs over the
// grep-restricted candidate set rather than the whole corpus, because an
// identifier like `forge_batch_item` is already an exact-match query.
//
// Pure functions only. Filesystem, git and the CLI live in
// scripts/forge-manifest.ts so the ranking can be asserted without either.
// ---------------------------------------------------------------------------

export type ManifestLane = 'handbook' | 'packet' | 'cited' | 'commit' | 'lexical' | 'index'

export type ManifestEntry = {
  /** Repo-relative path. */
  path: string
  lane: ManifestLane
  /** Why this row is here, in the words a reader would use. */
  detail: string
  /** Last commit date touching the path (YYYY-MM-DD), or null when unknown. */
  lastTouched: string | null
  /** The path was named (packet, commit) but is not on disk. Always a finding. */
  missing: boolean
  /** Lexical relevance; 0 for every structural lane. */
  score: number
}

export type ManifestCorpusFile = { path: string; content: string }

/**
 * Always-read files. Not ranked, not scored: if you work in this repo you read
 * these, and a manifest that omits them is how someone works from a stale picture
 * of the rules.
 */
export const HANDBOOK_PATHS = [
  'AGENTS.md',
  'docs/agent/ORIENTATION.md',
  'docs/agent/MEMORY.md',
  'docs/agent/CURRENT.md',
] as const

const STOPWORDS = new Set(
  (
    'the a an and or of to in on for is are was were be been am it its this that those these with from as at by if then than into not no do does ' +
    'did how what when where which who why you your we our they their them there here all any some more most other such only own same so too very ' +
    'can will just should now also must may might shall about over under again further once during before after above below out off while each ' +
    'add adds added new use uses used using make makes made get gets got run runs ran set sets back onto per via'
  ).split(/\s+/),
)

/** Words from a query or packet: lowercased, stopworded, identifiers kept intact. */
export function tokenize(text: string): string[] {
  return text
    .toLowerCase()
    .split(/[^a-z0-9_-]+/)
    .map((token) => token.replace(/^[-_]+|[-_]+$/g, ''))
    .filter((token) => token.length >= 3 && !STOPWORDS.has(token))
}

/**
 * Identifiers are what grep already does perfectly, so the lexical lane must not
 * dilute them: a token containing a digit or underscore is treated as an exact
 * term (`forge_batch_item`, `v5-23`, `storyboard_story`).
 */
export function isIdentifierToken(token: string): boolean {
  return /[0-9_]/.test(token)
}

/**
 * Repo paths named in backticks or markdown links.
 *
 * Deliberately the same contract as the packet lint's `citedRepoPaths`: a manifest
 * row and a packet citation should mean the same thing, or the gates disagree.
 */
export function citedPaths(text: string): string[] {
  const out = new Set<string>()
  const push = (raw: string) => {
    let token = raw.trim()
    // `path:120` / `path:120-140` — keep the path, the range is validated separately.
    const lineSuffixed = /^(.+?):(\d+)(?:-(\d+))?$/.exec(token)
    if (lineSuffixed && /\.[a-z]+$/.test(lineSuffixed[1])) token = lineSuffixed[1]
    if (!token.includes('/')) return
    if (token.startsWith('/') || token.startsWith('http') || token.startsWith('~')) return
    if (/^\.(next|vercel|git)\//.test(token)) return
    if (/[<>{}*]/.test(token)) return
    if (/\.(md|ts|tsx|mjs|cjs|js|jsx|json|sql|xml|css|sh|yml|yaml|plist|swift|txt)$/.test(token)) {
      out.add(token)
    }
  }
  for (const match of text.matchAll(/`([^`\n]+)`/g)) push(match[1])
  for (const match of text.matchAll(/\]\(([^)\s]+)\)/g)) push(match[1])
  return [...out]
}

/**
 * `path:120` and `path:120-140` citations.
 *
 * Item 5 of the steal list: evidence names a file AND a range so a reader can jump
 * to it, and a range that runs past the end of the file is a stale claim.
 */
export type LineCitation = { path: string; start: number; end: number; raw: string }

export function lineCitations(text: string): LineCitation[] {
  const out: LineCitation[] = []
  const pattern = /`([A-Za-z0-9_./-]+\.(?:ts|tsx|mjs|js|json|sql|sh|md|css)):(\d+)(?:-(\d+))?`/g
  for (const match of text.matchAll(pattern)) {
    const start = Number(match[2])
    const end = match[3] ? Number(match[3]) : start
    out.push({ path: match[1], start, end, raw: match[0] })
  }
  return out
}

export type RankManifestInput = {
  scope: string
  /** Repo-relative path of the packet, when the scope is a story. */
  packetPath?: string | null
  /** The packet's text, used for its cited paths and its vocabulary. */
  packetContent?: string
  /** Paths named by commits whose message mentions the scope. */
  commitPaths?: readonly string[]
  /** How many such commits there were, for the "why" line. */
  commitCount?: number
  /** path -> YYYY-MM-DD of the last commit touching it. */
  lastTouched?: Readonly<Record<string, string>>
  /** Candidate docs for the lexical lane (already the grep-restricted set). */
  corpus?: readonly ManifestCorpusFile[]
  /** Existence check, injected so this stays pure. */
  exists?: (path: string) => boolean
  /** Cap on lexical rows. Structural rows are never dropped. */
  lexicalLimit?: number
}

function corpusStats(corpus: readonly ManifestCorpusFile[]) {
  const documentFrequency = new Map<string, number>()
  const tokensByFile = new Map<string, string[]>()
  for (const file of corpus) {
    const tokens = tokenize(file.content)
    tokensByFile.set(file.path, tokens)
    for (const token of new Set(tokens)) {
      documentFrequency.set(token, (documentFrequency.get(token) ?? 0) + 1)
    }
  }
  return { documentFrequency, tokensByFile, total: Math.max(corpus.length, 1) }
}

/**
 * Build the ranked rows.
 *
 * Ordering is by lane, never by score across lanes: a file the packet cites outranks
 * a file that merely talks about the same words, however often it says them.
 */
export function rankManifestEntries(input: RankManifestInput): ManifestEntry[] {
  const exists = input.exists ?? (() => true)
  const lastTouched = input.lastTouched ?? {}
  const rows: ManifestEntry[] = []
  const seen = new Set<string>()

  const push = (path: string, lane: ManifestLane, detail: string, score = 0) => {
    if (!path || seen.has(path)) return
    seen.add(path)
    rows.push({ path, lane, detail, lastTouched: lastTouched[path] ?? null, missing: !exists(path), score })
  }

  for (const path of HANDBOOK_PATHS) push(path, 'handbook', 'always-read handbook')

  if (input.packetPath) push(input.packetPath, 'packet', `the packet for ${input.scope}`)

  for (const path of citedPaths(input.packetContent ?? '')) {
    if (path === input.packetPath) continue
    push(path, 'cited', `cited by ${input.scope}`)
  }

  const commits = [...new Set(input.commitPaths ?? [])]
  const commitCount = input.commitCount ?? 0
  for (const path of commits) {
    if (seen.has(path)) continue
    push(path, 'commit', `touched by ${commitCount || 1} commit(s) naming ${input.scope}`)
  }

  // Lexical lane: only paths sharing at least one token with the packet text.
  const corpus = (input.corpus ?? []).filter((file) => !seen.has(file.path))
  const terms = [...new Set(tokenize(input.packetContent ?? ''))]
  if (terms.length > 0 && corpus.length > 0) {
    const { documentFrequency, tokensByFile, total } = corpusStats(corpus)
    const termSet = new Set(terms)
    const scored = corpus
      .map((file) => {
        const tokens = tokensByFile.get(file.path) ?? []
        const counts = new Map<string, number>()
        for (const token of tokens) {
          if (termSet.has(token)) counts.set(token, (counts.get(token) ?? 0) + 1)
        }
        if (counts.size === 0) return { path: file.path, score: 0, matched: [] as string[] }
        let score = 0
        for (const [token, count] of counts) {
          const idf = Math.log(1 + total / (documentFrequency.get(token) ?? total))
          const weight = isIdentifierToken(token) ? 3 : 1
          score += weight * (1 + Math.log(count)) * idf
        }
        // A file that matches more distinct terms is about more of the question.
        score *= 1 + 0.35 * (counts.size - 1)
        // Length normalization: a 60KB log must not win a term-count argument.
        score /= Math.sqrt(1 + tokens.length / 1000)
        return { path: file.path, score, matched: [...counts.keys()] }
      })
      .filter((row) => row.score > 0)
      .sort((a, b) => b.score - a.score)

    for (const row of scored.slice(0, input.lexicalLimit ?? 8)) {
      push(row.path, 'lexical', `term match: ${row.matched.slice(0, 4).join(', ')}`, row.score)
    }
  }

  return rows
}

export type ManifestMeta = {
  scope: string
  generatedAt: string
  commit: string
  branch: string
  dirty: boolean
  /** Command that reproduces this file, printed in the header so nobody hand-edits it. */
  command: string
}

/**
 * The manifest as markdown: `path · lane · why · last touched`, with a MISSING flag
 * on the row itself, because a manifest that quietly drops a deleted file is how a
 * reader concludes the file still exists.
 */
export function renderManifest(entries: readonly ManifestEntry[], meta: ManifestMeta): string {
  const lines: string[] = []
  lines.push(`# Scope manifest — ${meta.scope}`)
  lines.push('')
  lines.push('<!-- GENERATED FILE. Do not hand-edit: the packet lint fails when it drifts from a fresh render. -->')
  lines.push('')
  lines.push(`- generated: ${meta.generatedAt}`)
  lines.push(`- commit: \`${meta.commit}\`${meta.dirty ? ' (working tree dirty)' : ''} on \`${meta.branch}\``)
  lines.push(`- regenerate: \`${meta.command}\``)
  lines.push(`- rows: ${entries.length} — packet, cited paths and story commits first, lexical matches after`)
  lines.push('')
  lines.push('Read top-down. A row is a file to open, and the why column says why it is here.')
  lines.push('')
  for (const entry of entries) {
    const touched = entry.lastTouched ?? 'untracked'
    const missing = entry.missing ? ' **MISSING**' : ''
    lines.push(`- \`${entry.path}\`${missing} — ${entry.lane} · ${entry.detail} · last touched ${touched}`)
  }
  lines.push('')
  return lines.join('\n')
}

/** Scope names become file names; keep a story id or a path from escaping the directory. */
export function manifestFileName(scope: string): string {
  const safe = scope.trim().replace(/[^A-Za-z0-9._-]+/g, '-').replace(/^-+|-+$/g, '')
  return safe || 'all'
}

/** Rows the lint must fail on: a manifest row naming a path that is not on disk. */
export function missingManifestPaths(entries: readonly ManifestEntry[]): string[] {
  return entries.filter((entry) => entry.missing).map((entry) => entry.path)
}

/** Same shape as the packet lint's Finding, so one printer serves both. */
export type ManifestFinding = { level: 'fail' | 'warn'; rule: string; file: string; message: string }

/** A manifest row for a path that no longer exists is a stale map, not a note. */
export function lintManifest(entries: readonly ManifestEntry[], file: string): ManifestFinding[] {
  return missingManifestPaths(entries).map((path) => ({
    level: 'fail' as const,
    rule: 'manifest-cites-missing-path',
    file,
    message: `row \`${path}\` is not on disk — regenerate: pnpm forge:manifest <scope>`,
  }))
}
