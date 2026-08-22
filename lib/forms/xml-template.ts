// ---------------------------------------------------------------------------
// DOC-08 — tiny human-editable XML template contract: parser + validator.
//
// XML is the CANONICAL template authoring format. This module is the smallest
// parser/validator necessary: a controlled-vocabulary XML subset mapped onto
// the existing TemplateDefinition seam. The runtime NEVER sees raw XML — it
// consumes TemplateDefinition.
//
// Deliberately NO: XSLT, XPath business logic, embedded JavaScript, SQL,
// expression engine, schema compiler, form-builder, Word.
//
// The vocabulary:
//   <form id version title [documentType] [issuer]>
//     <field id label type [required] [source] [options]/>
//     <section id title [editable]>  text + <value field="X"/>  </section>
//     <participants> <participant role label [multiple]/> </participants>
//     <signatures> <signature-group role label [field] [initials]/> </signatures>
//   </form>
//
// A section's text content is DEFAULT/boilerplate prose with inline
// `<value field="X"/>` substitutions (declarative binding, NOT an expression
// engine). The runtime draft (JSONB) is plain text and takes precedence for
// editable sections.
// ---------------------------------------------------------------------------

import type {
  TemplateDefinition,
  TemplateFieldDefinition,
  TemplateFieldType,
  TemplateFieldBinding,
  TemplateParticipantRole,
  TemplateSectionDefinition,
  TemplateSectionSegment,
  TemplateSignatureGroup,
} from './template-types'

export class TemplateXmlError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'TemplateXmlError'
  }
}

const FIELD_TYPES: readonly string[] = ['text', 'money', 'date', 'textarea', 'select']

const BINDINGS: readonly string[] = [
  'deal.client.name',
  'deal.property.label',
  'deal.offer.amount',
  'deal.financing.type',
  'deal.closing.date',
]

const KNOWN_FORM_CHILDREN = new Set(['field', 'section', 'participants', 'signatures'])
const KNOWN_SECTION_CHILDREN = new Set(['value'])
const KNOWN_PARTICIPANT_CHILDREN = new Set(['participant'])
const KNOWN_SIGNATURES_CHILDREN = new Set(['signature-group'])

// ---------------------------------------------------------------------------
// Tiny XML tokenizer for the controlled vocabulary (elements, attributes,
// comments, CDATA, the five named entities). Text runs are preserved so a
// section can carry default prose.
// ---------------------------------------------------------------------------

type Token =
  | { kind: 'open'; name: string; attrs: Record<string, string> }
  | { kind: 'selfClose'; name: string; attrs: Record<string, string> }
  | { kind: 'close'; name: string }
  | { kind: 'text'; value: string }

function decodeEntities(value: string): string {
  return value
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&amp;/g, '&')
}

function tokenize(source: string): Token[] {
  const tokens: Token[] = []
  let i = 0
  while (i < source.length) {
    const lt = source.indexOf('<', i)
    if (lt === -1) {
      const text = source.slice(i)
      if (text.trim()) tokens.push({ kind: 'text', value: decodeEntities(text.trim()) })
      break
    }
    if (lt > i) {
      const text = source.slice(i, lt)
      if (text.trim()) tokens.push({ kind: 'text', value: decodeEntities(text.trim()) })
    }

    if (source.startsWith('<!--', lt)) {
      const end = source.indexOf('-->', lt)
      if (end === -1) throw new TemplateXmlError('Malformed XML: unterminated comment.')
      i = end + 3
      continue
    }
    if (source.startsWith('<![CDATA[', lt)) {
      const end = source.indexOf(']]>', lt)
      if (end === -1) throw new TemplateXmlError('Malformed XML: unterminated CDATA.')
      tokens.push({ kind: 'text', value: source.slice(lt + 9, end) })
      i = end + 3
      continue
    }
    if (source.startsWith('<?', lt)) {
      const end = source.indexOf('?>', lt)
      if (end === -1) throw new TemplateXmlError('Malformed XML: unterminated processing instruction.')
      i = end + 2
      continue
    }

    const gt = source.indexOf('>', lt)
    if (gt === -1) throw new TemplateXmlError(`Malformed XML: unterminated tag at offset ${lt}.`)
    const raw = source.slice(lt + 1, gt).trim()
    i = gt + 1

    if (raw.startsWith('/')) {
      const name = raw.slice(1).trim()
      if (!name) throw new TemplateXmlError(`Malformed XML: empty closing tag at offset ${lt}.`)
      tokens.push({ kind: 'close', name })
      continue
    }

    const nameMatch = /^([A-Za-z_][A-Za-z0-9_-]*)/.exec(raw)
    if (!nameMatch) throw new TemplateXmlError(`Malformed XML: invalid tag at offset ${lt}.`)
    const name = nameMatch[1]
    const rest = raw.slice(name.length).trim()

    const attrs: Record<string, string> = {}
    const attrRe = /([A-Za-z_][A-Za-z0-9_-]*)\s*=\s*("([^"]*)"|'([^']*)')/g
    let m: RegExpExecArray | null
    let lastIndex = 0
    while ((m = attrRe.exec(rest)) !== null) {
      const between = rest.slice(lastIndex, m.index)
      if (!/^\s*$/.test(between)) {
        throw new TemplateXmlError(
          `Malformed XML: unexpected content in <${name}> at offset ${lt}: "${between.trim()}".`,
        )
      }
      attrs[m[1]] = decodeEntities(m[3] ?? m[4] ?? '')
      lastIndex = m.index + m[0].length
    }
    const trailing = rest.slice(lastIndex).trim()
    if (trailing === '/') {
      tokens.push({ kind: 'selfClose', name, attrs })
    } else if (trailing === '') {
      tokens.push({ kind: 'open', name, attrs })
    } else {
      throw new TemplateXmlError(
        `Malformed XML: unexpected content in <${name}> at offset ${lt}: "${trailing}".`,
      )
    }
  }
  return tokens
}

// __PART2__
// ---------------------------------------------------------------------------
// Tree builder (elements + text children) with well-formedness checks.
// ---------------------------------------------------------------------------

type XmlElement = {
  name: string
  attrs: Record<string, string>
  children: (XmlElement | string)[]
}

function buildTree(tokens: Token[]): XmlElement {
  const stack: XmlElement[] = []
  let root: XmlElement | null = null

  const append = (el: XmlElement) => {
    if (stack.length === 0) {
      if (root) throw new TemplateXmlError('Malformed XML: multiple root elements.')
      root = el
    } else {
      stack[stack.length - 1].children.push(el)
    }
  }

  for (const token of tokens) {
    if (token.kind === 'open') {
      const el: XmlElement = { name: token.name, attrs: token.attrs, children: [] }
      append(el)
      stack.push(el)
    } else if (token.kind === 'selfClose') {
      append({ name: token.name, attrs: token.attrs, children: [] })
    } else if (token.kind === 'text') {
      if (stack.length > 0) stack[stack.length - 1].children.push(token.value)
    } else {
      const top = stack.pop()
      if (!top || top.name !== token.name) {
        throw new TemplateXmlError(`Malformed XML: unexpected closing </${token.name}>.`)
      }
    }
  }
  if (stack.length > 0) {
    throw new TemplateXmlError(`Malformed XML: unclosed element <${stack[stack.length - 1].name}>.`)
  }
  if (!root) throw new TemplateXmlError('Malformed XML: empty document.')
  return root
}

// ---------------------------------------------------------------------------
// Mapping + validation: XML -> TemplateDefinition.
// ---------------------------------------------------------------------------

function requireAttr(el: XmlElement, name: string): string {
  const value = el.attrs[name]
  if (value === undefined || value === '') {
    throw new TemplateXmlError(`<${el.name}> requires attribute "${name}".`)
  }
  return value
}

function boolAttr(el: XmlElement, name: string, fallback: boolean): boolean {
  const value = el.attrs[name]
  if (value === undefined || value === '') return fallback
  const normalized = value.toLowerCase()
  if (normalized === 'true' || normalized === '1') return true
  if (normalized === 'false' || normalized === '0') return false
  throw new TemplateXmlError(
    `<${el.name}> attribute "${name}" must be true or false (got "${value}").`,
  )
}

function parseVersion(value: string): number {
  const version = Number(value)
  if (!Number.isInteger(version) || version < 1) {
    throw new TemplateXmlError(`<form> version must be a positive integer (got "${value}").`)
  }
  return version
}

function parseField(el: XmlElement, fieldIds: Set<string>): TemplateFieldDefinition {
  const name = requireAttr(el, 'id')
  if (fieldIds.has(name)) throw new TemplateXmlError(`Duplicate field id "${name}".`)
  fieldIds.add(name)

  const type = requireAttr(el, 'type')
  if (!FIELD_TYPES.includes(type)) {
    throw new TemplateXmlError(`Field "${name}" has unknown type "${type}".`)
  }

  const source = el.attrs['source']
  if (source !== undefined && !BINDINGS.includes(source)) {
    throw new TemplateXmlError(`Field "${name}" has unknown source binding "${source}".`)
  }

  let options: readonly string[] | undefined
  if (type === 'select') {
    options = (el.attrs['options'] ?? '')
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
    if (options.length === 0) {
      throw new TemplateXmlError(`Select field "${name}" requires a non-empty options list.`)
    }
  }

  return {
    name,
    label: requireAttr(el, 'label'),
    type: type as TemplateFieldType,
    required: boolAttr(el, 'required', false),
    binding: source ? (source as TemplateFieldBinding) : null,
    ...(options ? { options } : {}),
  }
}

// __PART3__
function parseSection(
  el: XmlElement,
  fieldIds: Set<string>,
  sectionIds: Set<string>,
): TemplateSectionDefinition {
  const name = requireAttr(el, 'id')
  if (sectionIds.has(name)) throw new TemplateXmlError(`Duplicate section id "${name}".`)
  sectionIds.add(name)

  const segments: TemplateSectionSegment[] = []
  const values: string[] = []

  for (const child of el.children) {
    if (typeof child === 'string') {
      if (child.trim()) segments.push({ kind: 'text', text: child.trim() })
      continue
    }
    if (!KNOWN_SECTION_CHILDREN.has(child.name)) {
      throw new TemplateXmlError(
        `Unknown element <${child.name}> inside <section id="${name}">.`,
      )
    }
    const field = requireAttr(child, 'field')
    if (!fieldIds.has(field)) {
      throw new TemplateXmlError(
        `Section "${name}" references unknown field "${field}".`,
      )
    }
    values.push(field)
    segments.push({ kind: 'value', field })
  }

  return {
    name,
    label: requireAttr(el, 'title'),
    editable: boolAttr(el, 'editable', false),
    segments,
    values,
  }
}

function parseParticipants(el: XmlElement): TemplateParticipantRole[] {
  const roles: TemplateParticipantRole[] = []
  for (const child of el.children) {
    if (typeof child === 'string') continue
    if (!KNOWN_PARTICIPANT_CHILDREN.has(child.name)) {
      throw new TemplateXmlError(
        `Unknown element <${child.name}> inside <participants>.`,
      )
    }
    const role = requireAttr(child, 'role')
    roles.push({
      role,
      label: child.attrs['label'] ?? role,
      multiple: boolAttr(child, 'multiple', false),
    })
  }
  return roles
}

function parseSignatures(
  el: XmlElement,
  fieldIds: Set<string>,
): TemplateSignatureGroup[] {
  const groups: TemplateSignatureGroup[] = []
  for (const child of el.children) {
    if (typeof child === 'string') continue
    if (!KNOWN_SIGNATURES_CHILDREN.has(child.name)) {
      throw new TemplateXmlError(
        `Unknown element <${child.name}> inside <signatures>.`,
      )
    }
    const role = requireAttr(child, 'role')
    const field = child.attrs['field'] ?? null
    if (field && !fieldIds.has(field)) {
      throw new TemplateXmlError(
        `Signature group "${role}" references unknown field "${field}".`,
      )
    }
    groups.push({
      role,
      label: child.attrs['label'] ?? role,
      field,
      initials: boolAttr(child, 'initials', false),
    })
  }
  return groups
}

/**
 * Parse + validate one XML template into the canonical TemplateDefinition.
 * Throws TemplateXmlError with a clear message on any structural problem.
 */
export function parseTemplateXml(xml: string): TemplateDefinition {
  const root = buildTree(tokenize(xml))
  if (root.name !== 'form') {
    throw new TemplateXmlError(`Template root must be <form> (got <${root.name}>).`)
  }

  const id = requireAttr(root, 'id')
  const version = parseVersion(requireAttr(root, 'version'))
  const title = requireAttr(root, 'title')
  const documentType = root.attrs['documentType'] ?? title
  const issuer = root.attrs['issuer'] ?? 'CulebraLuxe Real Estate'

  const fieldIds = new Set<string>()
  const sectionIds = new Set<string>()
  const fields: TemplateFieldDefinition[] = []
  const sections: TemplateSectionDefinition[] = []
  let participants: TemplateParticipantRole[] = []
  let signatureGroups: TemplateSignatureGroup[] = []

  for (const child of root.children) {
    if (typeof child === 'string') {
      if (child.trim()) {
        throw new TemplateXmlError(`Unexpected text content directly under <form>.`)
      }
      continue
    }
    if (!KNOWN_FORM_CHILDREN.has(child.name)) {
      throw new TemplateXmlError(`Unknown element <${child.name}> inside <form>.`)
    }
    switch (child.name) {
      case 'field':
        fields.push(parseField(child, fieldIds))
        break
      case 'section':
        sections.push(parseSection(child, fieldIds, sectionIds))
        break
      case 'participants':
        participants = parseParticipants(child)
        break
      case 'signatures':
        signatureGroups = parseSignatures(child, fieldIds)
        break
    }
  }

  if (fields.length === 0) {
    throw new TemplateXmlError('Template must declare at least one <field>.')
  }

  return {
    id,
    version,
    displayName: documentType,
    documentTypeLabel: documentType,
    fields,
    sections,
    participants,
    signatureGroups,
    rendering: { title, issuer },
  }
}


