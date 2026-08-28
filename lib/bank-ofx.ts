// ---------------------------------------------------------------------------
// BANK-OFX — narrow QBO / OFX bank-file parser.
//
// QBO is Intuit's Web Connect flavor of OFX. Both QBO and OFX may be delivered
// as legacy SGML-like text OR as XML. This module detects the family and syntax,
// parses the narrow OFX statement structure into a source-neutral model, and
// validates transactions. No parser-specific XML/SGML shape leaks past this
// boundary. It never touches the database.
//
// Intentionally bounded: this is bank-file intake ONLY. No expense creation, no
// vendor/category inference, no reconciliation.
// ---------------------------------------------------------------------------

export type BankSourceFormat = 'QBO' | 'OFX'
export type BankSyntaxKind = 'sgml' | 'xml'

export type BankStatement = {
  sourceFormat: BankSourceFormat
  syntaxKind: BankSyntaxKind
  bankId: string | null
  accountId: string | null
  accountType: string | null
  currencyCode: string | null
  statementStart: string | null
  statementEnd: string | null
  transactions: BankTransaction[]
}

export type BankTransaction = {
  fitid: string
  transactionType: string | null
  /** ISO-8601 UTC. */
  postedAt: string
  /** ISO-8601 UTC, or null when absent. */
  userInitiatedAt: string | null
  /** Signed bank amount exactly as supplied (credit positive, debit negative). */
  amount: number
  payeeName: string | null
  memo: string | null
  checkNumber: string | null
  referenceNumber: string | null
  /** Bounded raw `<STMTTRN>...</STMTTRN>` fragment for lineage. */
  rawSourceFragment: string
}

export type RejectedTransaction = {
  ordinal: number
  fitid: string | null
  reason: string
}

export type ParsedBankFile = {
  statement: BankStatement
  /** Total STMTTRN blocks encountered (valid + rejected). */
  parsedCount: number
  rejected: RejectedTransaction[]
}

// ---------------------------------------------------------------------------
// OFX date parsing
// ---------------------------------------------------------------------------

/**
 * Parse an OFX date into an ISO-8601 UTC string, or null when unusable.
 * Supports the common OFX forms:
 *   YYYYMMDD
 *   YYYYMMDDHHMMSS
 *   YYYYMMDDHHMMSS.fff  (fractional seconds)
 * each optionally followed by an OFX timezone suffix `[offset:tzid]`
 * (e.g. `[-4:AST]`, `[0:GMT]`) or a trailing `Z`.
 */
export function parseOfxDate(raw: string): string | null {
  const s = raw.trim()
  if (!s) return null

  // OFX timezone bracket suffix: [offset:tzid]
  const tzMatch = s.match(/\[([+-]?\d{1,2})(?::[A-Za-z0-9._+-]+)?\]\s*$/)
  let body = s
  let offsetHours = 0
  if (tzMatch) {
    body = s.slice(0, tzMatch.index).trim()
    offsetHours = Number(tzMatch[1])
  } else if (body.endsWith('Z')) {
    body = body.slice(0, -1).trim()
    offsetHours = 0
  }

  // Compact form: YYYYMMDD[HHMMSS[.fff]]
  const compact = body.match(
    /^(\d{4})(\d{2})(\d{2})(?:(\d{2})(\d{2})(\d{2})(?:\.(\d{1,9}))?)?$/,
  )
  if (compact) {
    const [, y, mo, d, hh = '00', mi = '00', ss = '00', frac = ''] = compact
    const ms = frac ? Math.round(parseFloat(`0.${frac}`) * 1000) : 0
    const baseUtc = Date.UTC(Number(y), Number(mo) - 1, Number(d), Number(hh), Number(mi), Number(ss), ms)
    return new Date(baseUtc - offsetHours * 3_600_000).toISOString()
  }

  // Fallback: ISO-ish form (XML OFX sometimes uses ISO with a Z). Strip any
  // lingering bracket suffix first.
  const isoBody = body.replace(/\[[^\]]*\]\s*$/, '')
  const iso = new Date(isoBody)
  if (!Number.isNaN(iso.getTime())) {
    // Apply the bracket offset (JS parsed it as local/UTC depending on form).
    return new Date(iso.getTime() - offsetHours * 3_600_000).toISOString()
  }
  return null
}

// ---------------------------------------------------------------------------
// Format detection
// ---------------------------------------------------------------------------

export type DetectedFormat = { format: BankSourceFormat; syntaxKind: BankSyntaxKind }

/**
 * Detect rather than trust the filename. `.qbo` normally maps to QBO, `.ofx` to
 * OFX, but the content is still inspected. Rejects anything that cannot be
 * reasonably identified as an OFX-family bank document.
 */
export function detectFormat(content: string, filename: string): DetectedFormat {
  const hasOfxHeader = /OFXHEADER\s*:/.test(content) || /DATA\s*:\s*OFXSGML/.test(content)
  const isXml = /<\?xml/i.test(content) || /^\s*<OFX[>\s]/.test(content)
  const hasOfxRoot = /<OFX[\s>]/i.test(content)

  if (!hasOfxHeader && !hasOfxRoot && !isXml) {
    throw new Error(
      'Not an OFX-family bank document: missing OFXHEADER / <OFX> (unsupported file format)',
    )
  }

  const ext = filename.split('.').pop()?.toLowerCase()
  const format: BankSourceFormat = ext === 'qbo' ? 'QBO' : 'OFX'
  return { format, syntaxKind: isXml ? 'xml' : 'sgml' }
}

// ---------------------------------------------------------------------------
// Narrow OFX/SGML tokenizer
// ---------------------------------------------------------------------------

type RawTransaction = { leaves: Record<string, string>; raw: string }

/** Extract leaf `<TAG>value` / `<TAG>value</TAG>` pairs from a block. */
function extractLeaves(inner: string): Record<string, string> {
  const out: Record<string, string> = {}
  // `([^<]+)` (one-or-more) forces the value to be non-empty so the optional
  // closing-tag group can never satisfy the match with a zero-width value.
  const re = /<([A-Za-z][A-Za-z0-9_]*)\s*>([^<]+)(?:<\/\1\s*>)?/g
  let m: RegExpExecArray | null
  while ((m = re.exec(inner)) !== null) {
    const key = m[1]
    const val = m[2].trim()
    if (key && val && val !== '/') out[key] = val
  }
  return out
}

/** Split a stripped OFX body into individual `<STMTTRN>` blocks. */
function parseTransactions(inner: string): RawTransaction[] {
  const parts = inner.split(/<STMTTRN>/)
  const out: RawTransaction[] = []
  for (let i = 1; i < parts.length; i++) {
    let block = parts[i]
    const close = block.indexOf('</STMTTRN>')
    if (close !== -1) block = block.slice(0, close)
    out.push({ leaves: extractLeaves(block), raw: `<STMTTRN>${block}</STMTTRN>`.trim() })
  }
  return out
}

// ---------------------------------------------------------------------------
// Transaction validation / normalization
// ---------------------------------------------------------------------------

function toTransaction(raw: RawTransaction, ordinal: number): { tx?: BankTransaction; reject?: RejectedTransaction } {
  const l = raw.leaves
  const fitid = (l.FITID ?? '').trim()
  if (!fitid) return { reject: { ordinal, fitid: null, reason: 'missing FITID' } }

  const amountRaw = (l.TRNAMT ?? '').trim()
  const amount = Number(amountRaw)
  if (!amountRaw || !Number.isFinite(amount)) {
    return { reject: { ordinal, fitid, reason: `invalid amount "${amountRaw}"` } }
  }

  const postedRaw = (l.DTPOSTED ?? '').trim()
  const postedAt = postedRaw ? parseOfxDate(postedRaw) : null
  if (!postedAt) return { reject: { ordinal, fitid, reason: `unusable posted date "${postedRaw}"` } }

  const userInitiatedAt = l.DTUSER ? parseOfxDate(l.DTUSER) : null

  return {
    tx: {
      fitid,
      transactionType: l.TRNTYPE ?? null,
      postedAt,
      userInitiatedAt,
      amount,
      payeeName: l.NAME ?? null,
      memo: l.MEMO ?? null,
      checkNumber: l.CHECKNUM ?? null,
      referenceNumber: l.REFNUM ?? null,
      rawSourceFragment: raw.raw,
    },
  }
}

// ---------------------------------------------------------------------------
// Public parse entry point
// ---------------------------------------------------------------------------

/** Parse a QBO/OFX file into a source-neutral statement + rejection summary. */
export function parseBankStatement(content: string, filename: string): ParsedBankFile {
  const { format, syntaxKind } = detectFormat(content, filename)

  const ofxIdx = content.indexOf('<OFX')
  if (ofxIdx === -1) {
    throw new Error('Not an OFX-family bank document: missing <OFX> root')
  }
  const body = content.slice(ofxIdx)

  const allLeaves = extractLeaves(body)
  const rawTxns = parseTransactions(body)

  const transactions: BankTransaction[] = []
  const rejected: RejectedTransaction[] = []
  rawTxns.forEach((raw, idx) => {
    const result = toTransaction(raw, idx + 1)
    if (result.tx) transactions.push(result.tx)
    else if (result.reject) rejected.push(result.reject)
  })

  const statement: BankStatement = {
    sourceFormat: format,
    syntaxKind,
    bankId: allLeaves.BANKID ?? null,
    accountId: allLeaves.ACCTID ?? null,
    accountType: allLeaves.ACCTTYPE ?? null,
    currencyCode: allLeaves.CURDEF ?? null,
    statementStart: ofxDateOnly(allLeaves.DTSTART),
    statementEnd: ofxDateOnly(allLeaves.DTEND),
    transactions,
  }

  return { statement, parsedCount: rawTxns.length, rejected }
}

// ---------------------------------------------------------------------------
// Presentation helpers (no full account numbers / raw files)
// ---------------------------------------------------------------------------

/** Mask an account number to its last four digits (e.g. `****6789`). */
export function maskAccount(account: string | null | undefined): string | null {
  const a = (account ?? '').trim()
  if (!a) return null
  return `****${a.slice(-4)}`
}

/** Currency-format a signed amount (e.g. `-12,431.28`, `8,900.00`). */
export function formatMoney(value: number): string {
  return new Intl.NumberFormat('en-US', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(value)
}

/** Deterministic source account identity for the replay uniqueness contract. */
export function sourceAccountIdentity(accountId: string | null): string {
  return (accountId ?? '').trim() || 'UNKNOWN_ACCOUNT'
}

/** Extract the bank calendar date (`YYYY-MM-DD`) from a raw OFX `YYYYMMDD...` value. */
export function ofxDateOnly(raw: string | undefined): string | null {
  if (!raw) return null
  const m = raw.trim().match(/^(\d{4})(\d{2})(\d{2})/)
  return m ? `${m[1]}-${m[2]}-${m[3]}` : null
}



