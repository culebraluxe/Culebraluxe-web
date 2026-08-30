// ---------------------------------------------------------------------------
// CulebraLuxe document composition engine (pdf-lib).
//
// TemplateDefinition + draft values produce one deterministic PDF artifact.
// The same renderer powers live preview and immutable issuance. Pagination
// resolves provider-neutral signature rectangles against the exact PDF pages;
// issuance snapshots those anchors beside the immutable bytes.
// ---------------------------------------------------------------------------

import { readFile } from 'node:fs/promises'

import {
  PDFDocument,
  PDFFont,
  PDFImage,
  PDFPage,
  StandardFonts,
  rgb,
} from 'pdf-lib'

import type {
  TemplateDefinition,
  TemplateFieldDefinition,
  TemplateFieldValues,
  TemplateSectionValues,
} from './template-types'
import {
  PDF_SIGNATURE_COORDINATE_SPACE,
  type FormSignatureAnchor,
  type FormSignatureAnchorKind,
  type PdfPointRectangle,
} from './signature-anchors'
import {
  formatBrokerInitials,
  formatBrokerSignatureDate,
  type AppliedSignatureEvidence,
  type FormAppliedSignature,
} from './applied-signature'
import {
  documentBodyText,
  formatDate,
  formatFieldValue,
  formatMoney,
  interpolateSectionText,
} from './format'

export {
  documentBodyText,
  formatDate,
  formatFieldValue,
  formatMoney,
  interpolateSectionText,
}

const PAGE = { width: 612, height: 792 } as const

const TOKENS = {
  marginX: 52,
  contentWidth: 508,
  headerTop: 34,
  bodyTop: 672,
  footerY: 34,
  bodyBottom: 58,
  bodySize: 10.35,
  bodyLeading: 14.7,
  sectionGap: 15,
  navy: rgb(3 / 255, 15 / 255, 35 / 255),
  navySoft: rgb(47 / 255, 70 / 255, 94 / 255),
  gold: rgb(198 / 255, 161 / 255, 91 / 255),
  ink: rgb(28 / 255, 31 / 255, 35 / 255),
  muted: rgb(98 / 255, 104 / 255, 111 / 255),
  rule: rgb(213 / 255, 216 / 255, 220 / 255),
  paperTint: rgb(247 / 255, 248 / 255, 249 / 255),
} as const

const LOGO_TARGET_HEIGHT = 42
const HEADER_TITLE_SIZE = 12.5
const SIGNATURE_BLOCK_HEIGHT = 104
const DETERMINISTIC_PDF_DATE = new Date('2000-01-01T00:00:00.000Z')

export type FormRenderParticipant = {
  role: string
  slotId?: string | null
  name: string
}

export type FormRenderOptions = {
  participants?: readonly FormRenderParticipant[]
  /** Authorized, issuance-bound signatures to compose into the artifact. */
  appliedSignatures?: readonly FormAppliedSignature[]
}

export type RenderedFormPdfArtifact = {
  bytes: Buffer
  pageCount: number
  pageSize: typeof PAGE
  signatureAnchors: FormSignatureAnchor[]
  appliedSignatures: AppliedSignatureEvidence[]
}

type EmbeddedAppliedSignature = FormAppliedSignature & { image: PDFImage }

let logoBytesCache: Uint8Array | null | undefined

async function loadLogoBytes(): Promise<Uint8Array | null> {
  if (logoBytesCache !== undefined) return logoBytesCache
  try {
    // Vercel traces this canonical public asset into the function bundle.
    // Resolve it from the deployed project root rather than import.meta.url,
    // whose location changes after Next/Turbopack compiles this module.
    const raw = await readFile(
      `${process.cwd()}/public/brand/CLLOGO.png`,
    )
    logoBytesCache = new Uint8Array(raw)
  } catch {
    logoBytesCache = null
  }
  return logoBytesCache
}

async function embedLogo(doc: PDFDocument): Promise<PDFImage | null> {
  const bytes = await loadLogoBytes()
  if (!bytes) return null
  try {
    return await doc.embedPng(bytes)
  } catch {
    return null
  }
}

/** WinAnsi-safe text while preserving Puerto Rican/Spanish Latin characters. */
function pdfSafe(value: string): string {
  return value
    .normalize('NFC')
    .replace(/[\u2018\u2019]/g, "'")
    .replace(/[\u201c\u201d]/g, '"')
    .replace(/[\u2013\u2014]/g, '-')
    .replace(/\u2026/g, '...')
    .replace(/\u00a0/g, ' ')
    .replace(/[^\u0020-\u007e\u00a1-\u00ff]/g, '?')
}

function splitLongWord(
  word: string,
  fontSize: number,
  maxWidth: number,
  font?: PDFFont,
): string[] {
  const widthOf = (value: string) =>
    font
      ? font.widthOfTextAtSize(value, fontSize)
      : value.length * fontSize * 0.5
  if (widthOf(word) <= maxWidth) return [word]
  const pieces: string[] = []
  let current = ''
  for (const character of word) {
    const candidate = `${current}${character}`
    if (current && widthOf(candidate) > maxWidth) {
      pieces.push(current)
      current = character
    } else {
      current = candidate
    }
  }
  if (current) pieces.push(current)
  return pieces
}

/** Greedy, measured word wrap with a safe fallback for long unbroken text. */
export function wrapText(
  text: string,
  fontSize: number,
  maxWidth: number,
  font?: PDFFont,
): string[] {
  const normalized = pdfSafe(text).replace(/\s+/g, ' ').trim()
  if (!normalized) return []
  const widthOf = (value: string) =>
    font
      ? font.widthOfTextAtSize(value, fontSize)
      : value.length * fontSize * 0.5
  const words = normalized
    .split(' ')
    .flatMap((word) => splitLongWord(word, fontSize, maxWidth, font))
  const lines: string[] = []
  let current = ''
  for (const word of words) {
    const candidate = current ? `${current} ${word}` : word
    if (widthOf(candidate) <= maxWidth) {
      current = candidate
    } else {
      if (current) lines.push(current)
      current = word
    }
  }
  if (current) lines.push(current)
  return lines
}

function textWidth(font: PDFFont, value: string, size: number): number {
  return font.widthOfTextAtSize(pdfSafe(value), size)
}

function parseBodyBlocks(body: string): Array<{ heading: string; body: string }> {
  return body
    .split(/\n{2,}/)
    .map((block) => block.trim())
    .filter(Boolean)
    .map((block) => {
      const lines = block.split('\n')
      return {
        heading: (lines.shift() ?? '').trim(),
        body: lines.join(' ').replace(/\s+/g, ' ').trim(),
      }
    })
}

function overviewFields(
  template: TemplateDefinition,
  values: TemplateFieldValues,
): Array<{ field: TemplateFieldDefinition; value: string }> {
  const filled = template.fields
    .map((field) => ({
      field,
      value: formatFieldValue(field, (values[field.name] ?? '').trim()),
    }))
    .filter((item) => Boolean(item.value))
  if (template.rendering.presentation !== 'agreement') return filled

  // Agreements lead with a deliberately small transaction overview. The
  // contract prose remains authoritative below; this is not a field dump.
  const preferred = filled.filter(({ field }) =>
    /buyer|seller|property|price|amount|deposit|closing date|start date|end date/i.test(
      `${field.name} ${field.label}`,
    ),
  )
  const selected = preferred.slice(0, 8)
  if (selected.length >= 4) return selected
  for (const item of filled) {
    if (selected.includes(item)) continue
    selected.push(item)
    if (selected.length >= 6) break
  }
  return selected
}

class DocumentComposer {
  private page!: PDFPage
  private pageIndex = -1
  private cursorY: number = TOKENS.bodyTop
  private readonly anchors: FormSignatureAnchor[] = []
  private readonly appliedEvidence: AppliedSignatureEvidence[] = []

  private constructor(
    private readonly doc: PDFDocument,
    private readonly template: TemplateDefinition,
    private readonly issuedVersion: number,
    private readonly bodyFont: PDFFont,
    private readonly bodyBold: PDFFont,
    private readonly sans: PDFFont,
    private readonly sansBold: PDFFont,
    private readonly logo: PDFImage | null,
    private readonly appliedSignatures: readonly EmbeddedAppliedSignature[],
  ) {
    this.addPage()
  }

  static async create(
    template: TemplateDefinition,
    issuedVersion: number,
    appliedSignatures: readonly FormAppliedSignature[],
  ): Promise<DocumentComposer> {
    const doc = await PDFDocument.create()
    doc.setTitle(template.rendering.title)
    doc.setAuthor(template.rendering.issuer)
    doc.setSubject(template.documentTypeLabel)
    doc.setCreator('CulebraLuxe Forms')
    doc.setProducer('CulebraLuxe Forms')
    doc.setCreationDate(DETERMINISTIC_PDF_DATE)
    doc.setModificationDate(DETERMINISTIC_PDF_DATE)
    const bodyFont = await doc.embedFont(StandardFonts.TimesRoman)
    const bodyBold = await doc.embedFont(StandardFonts.TimesRomanBold)
    const sans = await doc.embedFont(StandardFonts.Helvetica)
    const sansBold = await doc.embedFont(StandardFonts.HelveticaBold)
    const logo = await embedLogo(doc)
    const embeddedSignatures = await Promise.all(
      appliedSignatures.map(async (signature) => ({
        ...signature,
        image:
          signature.imageMimeType === 'image/png'
            ? await doc.embedPng(signature.imageBytes)
            : await doc.embedJpg(signature.imageBytes),
      })),
    )
    return new DocumentComposer(
      doc,
      template,
      issuedVersion,
      bodyFont,
      bodyBold,
      sans,
      sansBold,
      logo,
      embeddedSignatures,
    )
  }

  private addPage(): PDFPage {
    const page = this.doc.addPage([PAGE.width, PAGE.height])
    this.page = page
    this.pageIndex += 1
    this.cursorY = TOKENS.bodyTop
    this.drawHeader()
    return page
  }

  private drawHeader(): void {
    const logoHeight = LOGO_TARGET_HEIGHT
    const logoY = PAGE.height - TOKENS.headerTop - logoHeight
    const logoWidth = this.logo
      ? (this.logo.width / this.logo.height) * logoHeight
      : 0
    if (this.logo) {
      this.page.drawImage(this.logo, {
        x: PAGE.width - TOKENS.marginX - logoWidth,
        y: logoY,
        width: logoWidth,
        height: logoHeight,
      })
    } else {
      this.page.drawText('CULEBRALUXE', {
        x:
          PAGE.width -
          TOKENS.marginX -
          textWidth(this.bodyBold, 'CULEBRALUXE', 12),
        y: PAGE.height - TOKENS.headerTop - 24,
        size: 12,
        font: this.bodyBold,
        color: TOKENS.navy,
      })
    }

    const titleWidth = Math.max(
      250,
      TOKENS.contentWidth - Math.max(logoWidth, 112) - 28,
    )
    const title = pdfSafe(this.template.rendering.title)
    if (textWidth(this.sansBold, title, HEADER_TITLE_SIZE) > titleWidth) {
      throw new Error(
        `Form title exceeds the fixed header width: ${this.template.rendering.title}`,
      )
    }
    this.page.drawText(title, {
      x: TOKENS.marginX,
      // One canonical baseline on every form, optically centered to the
      // locked logo rather than drifting with title length or wrapping.
      y: logoY + (logoHeight - HEADER_TITLE_SIZE) / 2 + 1,
      size: HEADER_TITLE_SIZE,
      font: this.sansBold,
      color: TOKENS.navy,
    })
    this.page.drawLine({
      start: { x: TOKENS.marginX, y: PAGE.height - TOKENS.headerTop - logoHeight - 11 },
      end: {
        x: PAGE.width - TOKENS.marginX,
        y: PAGE.height - TOKENS.headerTop - logoHeight - 11,
      },
      thickness: 1.1,
      color: TOKENS.gold,
    })
  }

  private drawFooters(): void {
    const pages = this.doc.getPages()
    pages.forEach((page, index) => {
      page.drawLine({
        start: { x: TOKENS.marginX, y: TOKENS.footerY + 10 },
        end: { x: PAGE.width - TOKENS.marginX, y: TOKENS.footerY + 10 },
        thickness: 0.45,
        color: TOKENS.rule,
      })
      const identity = `${this.template.id} · Template ${this.template.version} · Document v${this.issuedVersion}`
      page.drawText(pdfSafe(identity), {
        x: TOKENS.marginX,
        y: TOKENS.footerY,
        size: 6.8,
        font: this.sans,
        color: TOKENS.muted,
      })
      const pageLabel = `Page ${index + 1} of ${pages.length}`
      page.drawText(pageLabel, {
        x:
          PAGE.width -
          TOKENS.marginX -
          textWidth(this.sans, pageLabel, 6.8),
        y: TOKENS.footerY,
        size: 6.8,
        font: this.sans,
        color: TOKENS.muted,
      })
    })
  }

  private ensureSpace(height: number): void {
    if (this.cursorY - height >= TOKENS.bodyBottom) return
    this.addPage()
  }

  private drawUpperLabel(value: string, x: number, y: number): void {
    this.page.drawText(pdfSafe(value.toUpperCase()), {
      x,
      y,
      size: 6.8,
      font: this.sansBold,
      color: TOKENS.muted,
    })
  }

  drawDocumentMetadata(): void {
    this.ensureSpace(22)
    this.page.drawText(
      pdfSafe(`${this.template.rendering.issuer} · Document version ${this.issuedVersion}`),
      {
        x: TOKENS.marginX,
        y: this.cursorY - 1,
        size: 7.5,
        font: this.sans,
        color: TOKENS.muted,
      },
    )
    this.cursorY -= 22
  }

  drawOverview(
    fields: Array<{ field: TemplateFieldDefinition; value: string }>,
  ): void {
    if (fields.length === 0) return
    const heading =
      this.template.rendering.presentation === 'agreement'
        ? 'Agreement overview'
        : 'Document details'
    this.ensureSpace(58)
    this.drawUpperLabel(heading, TOKENS.marginX + 12, this.cursorY - 2)
    this.cursorY -= 15

    const columnGap = 24
    const columnWidth = (TOKENS.contentWidth - columnGap - 24) / 2
    let index = 0
    while (index < fields.length) {
      const left = fields[index]
      const right = fields[index + 1]
      const leftLines = wrapText(left.value, 10, columnWidth, this.bodyFont)
      const rightLines = right
        ? wrapText(right.value, 10, columnWidth, this.bodyFont)
        : []
      const rowHeight = Math.max(leftLines.length, rightLines.length, 1) * 13 + 25
      this.ensureSpace(rowHeight)
      this.page.drawRectangle({
        x: TOKENS.marginX,
        y: this.cursorY - rowHeight + 6,
        width: TOKENS.contentWidth,
        height: rowHeight,
        color: TOKENS.paperTint,
      })
      this.page.drawRectangle({
        x: TOKENS.marginX,
        y: this.cursorY - rowHeight + 6,
        width: 2,
        height: rowHeight,
        color: TOKENS.gold,
      })
      const drawCell = (
        item: { field: TemplateFieldDefinition; value: string },
        x: number,
        lines: string[],
      ) => {
        this.drawUpperLabel(item.field.label, x, this.cursorY - 9)
        lines.forEach((line, lineIndex) => {
          this.page.drawText(line, {
            x,
            y: this.cursorY - 23 - lineIndex * 13,
            size: 10,
            font: this.bodyFont,
            color: TOKENS.ink,
          })
        })
      }
      drawCell(left, TOKENS.marginX + 12, leftLines)
      if (right) {
        drawCell(
          right,
          TOKENS.marginX + 12 + columnWidth + columnGap,
          rightLines,
        )
      }
      this.cursorY -= rowHeight + 4
      index += 2
    }
    this.cursorY -= 8
  }

  drawBody(body: string): void {
    const blocks = parseBodyBlocks(body)
    let sectionNumber = 0
    for (const block of blocks) {
      if (!block.body) continue
      sectionNumber += 1
      const heading =
        this.template.rendering.presentation === 'agreement'
          ? `${String(sectionNumber).padStart(2, '0')}  ${block.heading}`
          : block.heading
      const bodyLines = wrapText(
        block.body,
        TOKENS.bodySize,
        TOKENS.contentWidth,
        this.bodyFont,
      )
      const keepWith = 14 + Math.min(2, bodyLines.length) * TOKENS.bodyLeading
      this.ensureSpace(keepWith + TOKENS.sectionGap)
      this.page.drawText(pdfSafe(heading.toUpperCase()), {
        x: TOKENS.marginX,
        y: this.cursorY,
        size: 8.2,
        font: this.sansBold,
        color: TOKENS.navy,
      })
      this.cursorY -= 15
      for (const line of bodyLines) {
        this.ensureSpace(TOKENS.bodyLeading)
        this.page.drawText(line, {
          x: TOKENS.marginX,
          y: this.cursorY,
          size: TOKENS.bodySize,
          font: this.bodyFont,
          color: TOKENS.ink,
        })
        this.cursorY -= TOKENS.bodyLeading
      }
      this.cursorY -= TOKENS.sectionGap
    }
  }

  private addAnchor(
    role: string,
    slotId: string | null,
    kind: FormSignatureAnchorKind,
    rect: PdfPointRectangle,
  ): void {
    this.anchors.push({
      role,
      slotId,
      kind,
      pageIndex: this.pageIndex,
      pageWidth: PAGE.width,
      pageHeight: PAGE.height,
      rect,
      coordinateSpace: PDF_SIGNATURE_COORDINATE_SPACE,
    })
  }

  private drawSignatureBlock(input: {
    role: string
    slotId: string | null
    label: string
    name: string
    initials: boolean
    appliedSignature: EmbeddedAppliedSignature | null
  }): void {
    this.ensureSpace(SIGNATURE_BLOCK_HEIGHT)
    this.drawUpperLabel(input.label, TOKENS.marginX, this.cursorY)
    this.cursorY -= 11

    const lineY = this.cursorY - 39
    const signatureWidth = input.initials ? 252 : 302
    const initialsX = TOKENS.marginX + signatureWidth + 20
    const initialsWidth = 64
    const dateX = input.initials
      ? initialsX + initialsWidth + 20
      : TOKENS.marginX + 322
    const dateWidth = PAGE.width - TOKENS.marginX - dateX

    const drawLine = (x: number, width: number) => {
      this.page.drawLine({
        start: { x, y: lineY },
        end: { x: x + width, y: lineY },
        thickness: 0.65,
        color: TOKENS.navySoft,
      })
    }
    drawLine(TOKENS.marginX, signatureWidth)
    if (input.initials) drawLine(initialsX, initialsWidth)
    drawLine(dateX, dateWidth)

    const signatureRect = {
      x: TOKENS.marginX,
      y: lineY + 2,
      width: signatureWidth,
      height: 34,
    }
    const initialsRect = input.initials
      ? {
          x: initialsX,
          y: lineY + 2,
          width: initialsWidth,
          height: 27,
        }
      : null
    const dateRect = {
      x: dateX,
      y: lineY + 2,
      width: dateWidth,
      height: 20,
    }
    if (!input.appliedSignature) {
      this.addAnchor(input.role, input.slotId, 'signature', signatureRect)
    }
    if (initialsRect && !input.appliedSignature) {
      this.addAnchor(input.role, input.slotId, 'initial', initialsRect)
    }
    if (!input.appliedSignature) {
      this.addAnchor(input.role, input.slotId, 'date', dateRect)
    }

    if (input.appliedSignature) {
      const applied = input.appliedSignature
      const imageBounds = applied.image.scaleToFit(
        signatureRect.width - 8,
        signatureRect.height - 2,
      )
      this.page.drawImage(applied.image, {
        x: signatureRect.x + 4,
        y: signatureRect.y + 1,
        width: imageBounds.width,
        height: imageBounds.height,
      })
      const renderedDate = formatBrokerSignatureDate(applied.appliedAt)
      const dateLine = wrapText(
        renderedDate,
        8.6,
        dateRect.width - 4,
        this.sans,
      )[0]
      if (dateLine) {
        this.page.drawText(dateLine, {
          x: dateRect.x + 2,
          y: dateRect.y + 5,
          size: 8.6,
          font: this.sans,
          color: TOKENS.ink,
        })
      }
      const renderedInitials = initialsRect
        ? formatBrokerInitials(applied.signerName)
        : null
      if (initialsRect && renderedInitials) {
        const initialsSize = 10.5
        this.page.drawText(renderedInitials, {
          x:
            initialsRect.x +
            (initialsRect.width -
              textWidth(this.sansBold, renderedInitials, initialsSize)) /
              2,
          y: initialsRect.y + 6,
          size: initialsSize,
          font: this.sansBold,
          color: TOKENS.ink,
        })
      }
      this.appliedEvidence.push({
        role: applied.role,
        slotId: applied.slotId,
        signerName: applied.signerName,
        credentialLine: applied.credentialLine,
        signerAppUserId: applied.signerAppUserId,
        assetMediaId: applied.assetMediaId,
        assetChecksumSha256: applied.assetChecksumSha256,
        appliedAt: applied.appliedAt,
        consentBasis: applied.consentBasis,
        dateSemantic: applied.dateSemantic,
        renderedDate,
        renderedInitials,
        pageIndex: this.pageIndex,
        signatureRect,
        initialsRect,
        dateRect,
      })
    }

    this.page.drawText('SIGNATURE', {
      x: TOKENS.marginX,
      y: lineY - 9,
      size: 6.1,
      font: this.sans,
      color: TOKENS.muted,
    })
    if (input.initials) {
      this.page.drawText('INITIALS', {
        x: initialsX,
        y: lineY - 9,
        size: 6.1,
        font: this.sans,
        color: TOKENS.muted,
      })
    }
    this.page.drawText('DATE', {
      x: dateX,
      y: lineY - 9,
      size: 6.1,
      font: this.sans,
      color: TOKENS.muted,
    })
    if (input.name) {
      const name = wrapText(
        input.name,
        8.2,
        signatureWidth,
        this.sans,
      )[0]
      if (name) {
        this.page.drawText(name, {
          x: TOKENS.marginX,
          y: lineY - 22,
          size: 8.2,
          font: this.sans,
          color: TOKENS.ink,
        })
      }
    }
    if (input.appliedSignature) {
      this.page.drawText(pdfSafe(input.appliedSignature.credentialLine), {
        x: TOKENS.marginX,
        y: lineY - 34,
        size: 7.2,
        font: this.sans,
        color: TOKENS.navySoft,
      })
    }
    this.cursorY = lineY - 49
  }

  drawSignatures(
    template: TemplateDefinition,
    values: TemplateFieldValues,
    participants: readonly FormRenderParticipant[],
  ): void {
    if (template.signatureGroups.length === 0) return
    const signatureBlockCount = template.signatureGroups.reduce(
      (count, group) =>
        count +
        Math.max(
          1,
          participants.filter(
            (participant) => participant.role === group.role,
          ).length,
        ),
      0,
    )
    const fullSectionHeight =
      46 + signatureBlockCount * SIGNATURE_BLOCK_HEIGHT
    if (
      fullSectionHeight <= TOKENS.bodyTop - TOKENS.bodyBottom &&
      this.cursorY - fullSectionHeight < TOKENS.bodyBottom
    ) {
      this.addPage()
    } else {
      this.ensureSpace(65)
    }
    this.page.drawLine({
      start: { x: TOKENS.marginX, y: this.cursorY },
      end: { x: PAGE.width - TOKENS.marginX, y: this.cursorY },
      thickness: 0.75,
      color: TOKENS.rule,
    })
    this.cursorY -= 23
    this.page.drawText('SIGNATURES', {
      x: TOKENS.marginX,
      y: this.cursorY,
      size: 12,
      font: this.bodyBold,
      color: TOKENS.navy,
    })
    this.cursorY -= 23

    for (const group of template.signatureGroups) {
      const matching = participants.filter(
        (participant) => participant.role === group.role,
      )
      const signers =
        matching.length > 0
          ? matching
          : [
              {
                role: group.role,
                slotId: null,
                name: group.field
                  ? (values[group.field] ?? '').trim()
                  : '',
              },
            ]
      for (const signer of signers) {
        const roleApplied = this.appliedSignatures.filter(
          (signature) => signature.role === group.role,
        )
        const exactApplied = signer.slotId
          ? roleApplied.filter(
              (signature) => signature.slotId === signer.slotId,
            )
          : []
        const fallbackApplied =
          signers.length === 1 && roleApplied.length === 1
            ? roleApplied
            : []
        const appliedSignature = (exactApplied.length > 0
          ? exactApplied
          : fallbackApplied)[0] ?? null
        this.drawSignatureBlock({
          role: group.role,
          slotId: signer.slotId ?? null,
          label: group.label,
          name: signer.name,
          initials: group.initials,
          appliedSignature,
        })
      }
    }
  }

  async finish(): Promise<RenderedFormPdfArtifact> {
    this.drawFooters()
    const pageCount = this.doc.getPageCount()
    const bytes = await this.doc.save({
      useObjectStreams: false,
      addDefaultPage: false,
      updateFieldAppearances: false,
    })
    return {
      bytes: Buffer.from(bytes),
      pageCount,
      pageSize: PAGE,
      signatureAnchors: [...this.anchors],
      appliedSignatures: [...this.appliedEvidence],
    }
  }
}

export async function renderFormPdfArtifact(
  template: TemplateDefinition,
  values: TemplateFieldValues,
  sections: TemplateSectionValues,
  issuedVersion: number,
  options: FormRenderOptions = {},
): Promise<RenderedFormPdfArtifact> {
  const composer = await DocumentComposer.create(
    template,
    issuedVersion,
    options.appliedSignatures ?? [],
  )
  composer.drawDocumentMetadata()
  composer.drawOverview(overviewFields(template, values))
  const body =
    (sections.body ?? '').trim() || documentBodyText(template, values)
  composer.drawBody(body)
  composer.drawSignatures(template, values, options.participants ?? [])
  return composer.finish()
}

export async function renderFormPdf(
  template: TemplateDefinition,
  values: TemplateFieldValues,
  sections: TemplateSectionValues,
  issuedVersion: number,
  options: FormRenderOptions = {},
): Promise<Buffer> {
  return (
    await renderFormPdfArtifact(
      template,
      values,
      sections,
      issuedVersion,
      options,
    )
  ).bytes
}

export async function buildOfferLetterPdf(
  template: TemplateDefinition,
  values: TemplateFieldValues,
  sections: TemplateSectionValues,
  issuedVersion: number,
): Promise<Buffer> {
  return renderFormPdf(template, values, sections, issuedVersion)
}

export async function buildPurchaseSalePdf(
  template: TemplateDefinition,
  values: TemplateFieldValues,
  sections: TemplateSectionValues,
  issuedVersion: number,
): Promise<Buffer> {
  return renderFormPdf(template, values, sections, issuedVersion)
}