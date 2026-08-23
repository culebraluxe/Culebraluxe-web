// ---------------------------------------------------------------------------
// DOC-06 — deterministic PDF renderer (POC).
//
// WHY THIS RENDERER:
//   - Server-side, pure Node, zero native/browser dependencies → deploys on
//     Vercel without a headless browser or desktop tooling.
//   - FULLY DETERMINISTIC by construction: no timestamps, no random ids, no
//     external fonts (standard Helvetica Type1 fonts are referenced by name,
//     never embedded). Identical input → identical bytes, which is what makes
//     the issued-checksum invariant meaningful.
//   - No document platform is introduced. This is a deliberately small fixed-
//     layout writer for the POC Offer Letter; the TemplateDefinition seam
//     (fields/sections/rendering) drives what text is placed.
//
// The 14 standard PDF fonts make hand-rolled layout safe: Helvetica's AFM
// widths are a public constant, so word-wrapping is exact and reproducible.
// ---------------------------------------------------------------------------

import type {
  TemplateDefinition,
  TemplateFieldValues,
  TemplateSectionValues,
} from './template-types'

// Helvetica AFM widths (1/1000 em) for printable WinAnsi ASCII 32..126.
const HELVETICA_WIDTHS: Record<string, number> = {
  ' ': 278, '!': 278, '"': 355, '#': 556, $: 556, '%': 889, '&': 667,
  "'": 191, '(': 333, ')': 333, '*': 389, '+': 584, ',': 278, '-': 333,
  '.': 278, '/': 278,
  '0': 556, '1': 556, '2': 556, '3': 556, '4': 556, '5': 556,
  '6': 556, '7': 556, '8': 556, '9': 556,
  ':': 278, ';': 278, '<': 584, '=': 584, '>': 584, '?': 556, '@': 1015,
  A: 667, B: 667, C: 722, D: 722, E: 667, F: 611, G: 778, H: 722,
  I: 278, J: 500, K: 667, L: 556, M: 833, N: 722, O: 778, P: 667,
  Q: 778, R: 722, S: 667, T: 611, U: 722, V: 667, W: 944, X: 667,
  Y: 667, Z: 611,
  '[': 278, '\\': 278, ']': 278, '^': 469, _: 556, '`': 333,
  a: 556, b: 556, c: 500, d: 556, e: 556, f: 278, g: 556, h: 556,
  i: 222, j: 222, k: 500, l: 222, m: 833, n: 556, o: 556, p: 556,
  q: 556, r: 333, s: 500, t: 278, u: 556, v: 500, w: 722, x: 500,
  y: 500, z: 500,
  '{': 334, '|': 260, '}': 334, '~': 584,
}

const FALLBACK_WIDTH = HELVETICA_WIDTHS['?']

function charWidth(ch: string): number {
  return HELVETICA_WIDTHS[ch] ?? FALLBACK_WIDTH
}

function textWidth(text: string, fontSize: number): number {
  let units = 0
  for (const ch of text) units += charWidth(ch)
  return (units / 1000) * fontSize
}

/** Greedy word-wrap; treats every run of whitespace as a break. */
export function wrapText(
  text: string,
  fontSize: number,
  maxWidth: number,
): string[] {
  const normalized = text.replace(/\s+/g, ' ').trim()
  if (!normalized) return []
  const words = normalized.split(' ')
  const lines: string[] = []
  let current = ''
  for (const word of words) {
    const candidate = current ? `${current} ${word}` : word
    if (textWidth(candidate, fontSize) <= maxWidth) {
      current = candidate
    } else {
      if (current) lines.push(current)
      current = word
    }
  }
  if (current) lines.push(current)
  return lines
}

/** Deterministic USD formatting for money fields. */
export function formatMoney(value: string): string {
  const digits = value.replace(/[^0-9.]/g, '')
  if (!digits) return value.trim()
  const [whole, decimal] = digits.split('.')
  const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, ',')
  return decimal ? `$${grouped}.${decimal}` : `$${grouped}`
}

const MONTHS = [
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
]

/** Deterministic ISO (YYYY-MM-DD) → 'Month D, YYYY' date formatting. */
export function formatDate(value: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(value.trim())
  if (!match) return value.trim()
  const year = Number(match[1])
  const month = Number(match[2])
  const day = Number(match[3])
  if (month < 1 || month > 12 || day < 1 || day > 31) return value.trim()
  return `${MONTHS[month - 1]} ${day}, ${year}`
}

/** Escape a string for a PDF literal-string (parentheses/backslash). */
function escapeLiteral(value: string): string {
  // WinAnsi-safe: anything outside printable ASCII/Latin-1 becomes '?'.
  const safe = value.replace(/[^\x20-\x7e\xa0-\xff]/g, '?')
  return safe.replace(/\\/g, '\\\\').replace(/\(/g, '\\(').replace(/\)/g, '\\)')
}

type PdfFont = 'F1' | 'F2'

type Op = {
  font: PdfFont
  size: number
  x: number
  y: number // bottom-up points
  text: string
}

// US Letter geometry.
const PAGE_WIDTH = 612
const PAGE_HEIGHT = 792
const MARGIN = 56
const MAX_LINE_WIDTH = PAGE_WIDTH - MARGIN * 2
// DOC-08 multi-page: content flows between these margins; overflow emits a
// deterministic page break (isolated behind this renderer seam, NOT a general
// document engine).
const TOP_MARGIN = 72
const BOTTOM_MARGIN = 72

class PdfLayout {
  private ops: Op[] = []
  private y = PAGE_HEIGHT - 90

  lineGap(size: number) {
    this.y -= size * 1.45
  }

  space(points: number) {
    this.y -= points
  }

  /** Small extra downward nudge without a full line gap (footer spacing). */
  tight() {
    this.y -= 2
  }

  /**
   * Ensure at least `needed` points remain below the current cursor; otherwise
   * emit a deterministic page break and reset to the top margin. Returns true
   * when a break was emitted (so a caller can re-emit a running header).
   */
  ensureSpace(needed: number): boolean {
    if (this.y - needed < BOTTOM_MARGIN) {
      this.ops.push({ font: 'F1', size: 1, x: MARGIN, y: this.y, text: '\u0000BREAK' })
      this.y = PAGE_HEIGHT - TOP_MARGIN
      return true
    }
    return false
  }

  text(font: PdfFont, size: number, text: string, x = MARGIN) {
    this.ensureSpace(size)
    this.ops.push({ font, size, x, y: this.y, text })
    this.lineGap(size)
  }

  paragraph(
    font: PdfFont,
    size: number,
    text: string,
    x = MARGIN,
    width = MAX_LINE_WIDTH,
    lineHeight = size * 1.45,
  ) {
    for (const line of wrapText(text, size, width)) {
      this.ensureSpace(lineHeight)
      this.ops.push({ font, size, x, y: this.y, text: line })
      this.y -= lineHeight
    }
  }

  rule() {
    // A filled black rectangle across the content width (sentinel op).
    this.ops.push({ font: 'F1', size: 1, x: MARGIN, y: this.y, text: '\u0000RULE' })
    this.y -= 14
  }

  build(ops: Op[]): string[] {
    const lines: string[] = []
    for (const op of ops) {
      if (op.text === '\u0000BREAK') continue
      if (op.text === '\u0000RULE') {
        const h = 0.6
        const yBottom = op.y - h
        lines.push(`${op.x} ${yBottom.toFixed(2)} ${MAX_LINE_WIDTH} ${h} re f`)
        continue
      }
      lines.push('BT')
      lines.push(`/${op.font} ${op.size} Tf`)
      lines.push(`${op.x} ${op.y.toFixed(2)} Td`)
      lines.push(`(${escapeLiteral(op.text)}) Tj`)
      lines.push('ET')
    }
    return lines
  }

  getOps(): Op[] {
    return this.ops
  }
}

/**
 * Render the POC Offer Letter PDF from a TemplateDefinition + values/sections.
 * Deterministic: same input → same bytes. `issuedVersion` is drawn on the
 * document so the printed artifact carries its own lineage label.
 */
/**
 * FORMS-01 — the single renderer for preview and issuance.
 * OFFER-01 keeps its field-centric layout (byte-stable with DOC-08 proofs).
 * Every other template uses the multi-page document layout.
 */
export function renderFormPdf(
  template: TemplateDefinition,
  values: TemplateFieldValues,
  sections: TemplateSectionValues,
  issuedVersion: number,
): Buffer {
  if (template.id === 'OFFER-01') {
    return buildOfferLetterPdf(template, values, sections, issuedVersion)
  }
  return buildPurchaseSalePdf(template, values, sections, issuedVersion)
}

export function buildOfferLetterPdf(
  template: TemplateDefinition,
  values: TemplateFieldValues,
  sections: TemplateSectionValues,
  issuedVersion: number,
): Buffer {
  const layout = new PdfLayout()

  layout.text('F1', 9, template.rendering.issuer.toUpperCase())
  layout.text('F2', 20, template.rendering.title, MARGIN)
  layout.space(6)
  layout.rule()
  layout.space(10)

  for (const field of template.fields) {
    const raw = (values[field.name] ?? '').trim()
    if (!raw && !field.required) continue // blank optional fields are omitted
    let rendered = raw
    if (field.type === 'money') rendered = formatMoney(raw)
    if (field.type === 'date') rendered = formatDate(raw)
    layout.text('F2', 9.5, field.label.toUpperCase())
    layout.text('F1', 11, rendered)
    layout.space(6)
  }

  for (const section of template.sections) {
    // DOC-08: the runtime draft (JSONB) is authoritative for editable prose;
    // otherwise the template's default segments (boilerplate + <value>
    // substitutions) render.
    const raw = (sections[section.name] ?? '').trim()
    const text = raw || interpolateSectionText(section, values, formatFieldValue)
    if (!text.trim()) continue
    layout.space(4)
    layout.text('F2', 11, section.label.toUpperCase())
    layout.paragraph('F1', 10.5, text.trim())
  }

  layout.space(18)
  layout.rule()

  const footer = [
    `${template.rendering.issuer} · Issued document v${issuedVersion}`,
    'CulebraLuxe Real Estate - deterministic issued artifact. This PDF is immutable in the canonical repository.',
  ]
  for (const line of footer) {
    layout.text('F1', 8, line, MARGIN)
    layout.tight()
  }

  return assemblePdf(layout.getOps())
}

/** Format one field value for rendering (money/date aware). */
export function formatFieldValue(field: { type: string; name: string }, raw: string): string {
  if (field.type === 'money' && raw.trim()) return formatMoney(raw)
  if (field.type === 'date' && raw.trim()) return formatDate(raw)
  return raw
}

/**
 * DOC-08 — interpolate a section's default segments: literal text plus the
 * declared `<value field="X"/>` substitutions (a declarative binding, NOT an
 * expression engine). The runtime draft, when present, takes precedence.
 */
export function interpolateSectionText(
  section: { segments: readonly { kind: 'text' | 'value'; text?: string; field?: string }[] },
  values: TemplateFieldValues,
  format: (field: { type: string; name: string }, raw: string) => string,
  fields: readonly { type: string; name: string }[] = [],
): string {
  let out = ''
  for (const segment of section.segments ?? []) {
    if (segment.kind === 'text') {
      out += segment.text ?? ''
    } else {
      const field = fields.find((f) => f.name === segment.field)
      const raw = (values[segment.field ?? ''] ?? '').trim()
      out += field ? format(field, raw) : raw
    }
  }
  return out.replace(/\s+/g, ' ').trim()
}

function emitHeader(
  layout: PdfLayout,
  template: TemplateDefinition,
  issuedVersion: number,
) {
  layout.text('F1', 9, template.rendering.issuer.toUpperCase())
  layout.text('F2', 16, template.rendering.title, MARGIN)
  layout.text('F1', 8, `Issued document v${issuedVersion}`, MARGIN)
  layout.rule()
  layout.space(8)
}

/**
 * DOC-08 — P&S proof renderer. Section-centric, multi-page (deterministic
 * page breaks), boilerplate sections render their default segments with
 * `<value>` substitutions; negotiated sections render the runtime draft prose;
 * signature blocks render at the end. This is a bounded proof renderer behind
 * the same seam — not a general document engine.
 */
export function buildPurchaseSalePdf(
  template: TemplateDefinition,
  values: TemplateFieldValues,
  sections: TemplateSectionValues,
  issuedVersion: number,
): Buffer {
  const layout = new PdfLayout()
  emitHeader(layout, template, issuedVersion)

  for (const section of template.sections) {
    const raw = (sections[section.name] ?? '').trim()
    const text = section.editable
      ? raw
      : raw || interpolateSectionText(section, values, formatFieldValue, template.fields)
    if (!text.trim()) continue
    const broke = layout.ensureSpace(34)
    if (broke) emitHeader(layout, template, issuedVersion)
    layout.text('F2', 11, section.label.toUpperCase())
    layout.paragraph('F1', 10, text.trim())
    layout.space(8)
  }

  if (template.signatureGroups.length > 0) {
    if (layout.ensureSpace(70)) emitHeader(layout, template, issuedVersion)
    layout.text('F2', 13, 'SIGNATURES')
    layout.space(4)
    for (const group of template.signatureGroups) {
      layout.ensureSpace(52)
      const name = group.field ? (values[group.field] ?? '').trim() : ''
      layout.text('F1', 10, `${group.label}${name ? ` — ${name}` : ''}`)
      layout.text('F1', 10, 'By: ____________________________________')
      if (group.initials) layout.text('F1', 10, 'Initials: ________')
      layout.space(12)
    }
  }

  return assemblePdf(layout.getOps())
}

/** Split ops into page segments at deterministic page-break markers. */
function splitPages(ops: Op[]): Op[][] {
  const pages: Op[][] = []
  let current: Op[] = []
  for (const op of ops) {
    if (op.text === '\u0000BREAK') {
      if (current.length > 0) pages.push(current)
      current = []
      continue
    }
    current.push(op)
  }
  if (current.length > 0) pages.push(current)
  if (pages.length === 0) pages.push([])
  return pages
}

/** Assemble a minimal PDF 1.4 document with the given content operations. */
function assemblePdf(ops: Op[]): Buffer {
  const pages = splitPages(ops)
  const pageCount = pages.length
  const builder = new PdfLayout()

  const objects: string[] = []
  objects.push('<< /Type /Catalog /Pages 2 0 R >>') // 1
  const pageRefs = Array.from({ length: pageCount }, (_, i) => 3 + i * 2).join(' ')
  objects.push(`<< /Type /Pages /Kids [${pageRefs}] /Count ${pageCount} >>`) // 2

  const font1 = 3 + pageCount * 2
  const font2 = font1 + 1

  for (let i = 0; i < pageCount; i++) {
    const content = `${builder.build(pages[i]).join('\n')}\n`
    const pageObj = 3 + i * 2
    const contentObj = pageObj + 1
    objects.push(
      `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${PAGE_WIDTH} ${PAGE_HEIGHT}] ` +
        `/Resources << /Font << /F1 ${font1} 0 R /F2 ${font2} 0 R >> >> ` +
        `/Contents ${contentObj} 0 R >>`,
    )
    objects.push(`<< /Length ${content.length} >>\nstream\n${content}endstream`)
  }

  objects.push('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>')
  objects.push('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>')

  const lines: string[] = []
  lines.push('%PDF-1.4')
  lines.push('%\u00e2\u00e3\u00cf\u00d3')
  const offsets: number[] = []
  for (let i = 0; i < objects.length; i++) {
    offsets.push(Buffer.byteLength(lines.join('\n'), 'binary'))
    lines.push(`${i + 1} 0 obj`)
    lines.push(objects[i])
    lines.push('endobj')
  }
  const xrefStart = Buffer.byteLength(lines.join('\n'), 'binary')
  lines.push('xref')
  lines.push(`0 ${objects.length + 1}`)
  lines.push('0000000000 65535 f ')
  for (const offset of offsets) {
    lines.push(String(offset).padStart(10, '0') + ' 00000 n ')
  }
  lines.push('trailer')
  lines.push(`<< /Size ${objects.length + 1} /Root 1 0 R >>`)
  lines.push('startxref')
  lines.push(String(xrefStart))
  lines.push('%%EOF')

  return Buffer.from(lines.join('\n'), 'binary')
}

