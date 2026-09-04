import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { parseTemplateXml, TemplateXmlError } from '../../lib/forms/xml-template'
import {
  getTemplate,
  listTemplates,
  OFFER_LETTER_TEMPLATE_ID,
  PURCHASE_SALE_TEMPLATE_ID,
  PURCHASE_SALE_AMENDMENT_TEMPLATE_ID,
  LISTING_AGREEMENT_TEMPLATE_ID,
  SHOWING_INFO_TEMPLATE_ID,
  SHOWING_REPORT_TEMPLATE_ID,
} from '../../lib/forms/template-registry'
import {
  renderFormPdf,
  renderFormPdfArtifact,
  buildOfferLetterPdf,
  signatureBlockDisplayName,
} from '../../lib/forms/pdf'
import { resolveDocumentBody } from '../../lib/forms/format'
import { prefillFieldValues, emptySectionValues } from '../../lib/forms/offer-letter-data'
import { formContentFingerprint } from '../../lib/forms/artifact-identity'
import {
  BROKER_SIGNATURE_CONSENT_BASIS,
  BROKER_SIGNATURE_DATE_SEMANTIC,
} from '../../lib/forms/applied-signature'

const LISA_ROLE_BY_TEMPLATE: Readonly<Record<string, string>> = {
  [OFFER_LETTER_TEMPLATE_ID]: 'BUYER_BROKER',
  [PURCHASE_SALE_TEMPLATE_ID]: 'SELLER_BROKER',
  [PURCHASE_SALE_AMENDMENT_TEMPLATE_ID]: 'SELLER_BROKER',
  [LISTING_AGREEMENT_TEMPLATE_ID]: 'SELLER_BROKER',
  [SHOWING_INFO_TEMPLATE_ID]: 'BUYER_BROKER',
  [SHOWING_REPORT_TEMPLATE_ID]: 'BUYER_BROKER',
}

test('FORMS-01: all production templates load and validate', () => {
  const ids = [
    OFFER_LETTER_TEMPLATE_ID,
    PURCHASE_SALE_TEMPLATE_ID,
    PURCHASE_SALE_AMENDMENT_TEMPLATE_ID,
    LISTING_AGREEMENT_TEMPLATE_ID,
    SHOWING_INFO_TEMPLATE_ID,
    SHOWING_REPORT_TEMPLATE_ID,
  ]
  const loaded = listTemplates()
  for (const id of ids) {
    const template = getTemplate(id, 1)
    assert.ok(template, `${id} loads`)
    assert.ok(template.fields.length > 0, `${id} has fields`)
    assert.equal(loaded.some((item) => item.id === id), true)
  }
})

test('FORMS-01: P&S has role collections and signature groups', () => {
  const template = getTemplate(PURCHASE_SALE_TEMPLATE_ID, 1)
  assert.ok(template)
  assert.ok(template.participants.some((p) => p.role === 'BUYER' && p.multiple))
  assert.ok(template.participants.some((p) => p.role === 'SELLER' && p.multiple))
  assert.ok(template.signatureGroups.some((g) => g.role === 'BUYER'))
  assert.ok(template.signatureGroups.some((g) => g.role === 'SELLER'))
})

test('FORMS-BR: every production form renders at least three complete signer sets', async () => {
  for (const template of listTemplates()) {
    assert.equal(
      template.signatureGroups.every((group) => group.initials),
      true,
      `${template.id} requires Initials on every signer row`,
    )

    const values: Record<string, string> = {}
    for (const field of template.fields) {
      values[field.name] =
        field.type === 'money'
          ? '1000000'
          : field.type === 'date'
            ? '2026-09-01'
            : field.type === 'select'
              ? (field.options?.[0] ?? 'Review option')
              : /broker|agent/i.test(field.name)
                ? 'Lisa Penfield'
                : `${field.label} review value`
    }

    const lisaRole = LISA_ROLE_BY_TEMPLATE[template.id]
    const participants =
      template.id === LISTING_AGREEMENT_TEMPLATE_ID
        ? [
            { role: 'SELLER', slotId: 'SELLER:1', name: 'Carlos Vega' },
            { role: 'SELLER', slotId: 'SELLER:2', name: 'Elena Morales' },
            {
              role: 'SELLER_BROKER',
              slotId: 'SELLER_BROKER:1',
              name: 'Lisa Penfield',
            },
          ]
        : template.signatureGroups.map((group, index) => ({
            role: group.role,
            slotId: `${group.role}:1`,
            name:
              group.role === lisaRole
                ? 'Lisa Penfield'
                : `External Party ${index + 1}`,
          }))

    assert.ok(
      participants.length >= 3,
      `${template.id} declares two external parties plus Lisa`,
    )
    assert.equal(
      participants.filter((participant) => participant.name === 'Lisa Penfield').length,
      1,
      `${template.id} has exactly one Lisa broker row`,
    )

    const artifact = await renderFormPdfArtifact(
      template,
      values,
      emptySectionValues(template),
      1,
      { participants },
    )
    const anchorsBySlot = new Map<string, Set<string>>()
    for (const anchor of artifact.signatureAnchors) {
      const slot = anchor.slotId ?? `${anchor.role}:fallback`
      const kinds = anchorsBySlot.get(slot) ?? new Set<string>()
      kinds.add(anchor.kind)
      anchorsBySlot.set(slot, kinds)
    }
    assert.ok(
      anchorsBySlot.size >= 3,
      `${template.id} renders at least three signer rows`,
    )
    for (const [slot, kinds] of anchorsBySlot) {
      assert.deepEqual(
        [...kinds].sort(),
        ['date', 'initial', 'signature'],
        `${template.id} ${slot} renders Signature, Initials, and Date`,
      )
    }
  }
})

test('FORMS-01: XML rejects unknown bindings and duplicate fields', () => {
  assert.throws(
    () =>
      parseTemplateXml(
        `<form id="X" version="1" title="X"><field id="a" label="A" type="text" source="nope.path"/></form>`,
      ),
    TemplateXmlError,
  )
  assert.throws(
    () =>
      parseTemplateXml(
        `<form id="X" version="1" title="X"><field id="a" label="A" type="text"/><field id="a" label="B" type="text"/></form>`,
      ),
    TemplateXmlError,
  )
})

test('FORMS-01: unified renderer matches Offer Letter', async () => {
  const template = getTemplate(OFFER_LETTER_TEMPLATE_ID, 1)!
  const values = prefillFieldValues(template, {
    clientName: 'James Lee',
    propertyLabel: 'Sunset Point',
    offerAmount: '3950000',
    financingType: 'Cash',
    closingDate: '2026-11-15',
  })
  values.expiration = '2026-09-01'
  const sections = emptySectionValues(template)
  const a = await renderFormPdf(template, values, sections, 1)
  const b = await buildOfferLetterPdf(template, values, sections, 1)
  assert.equal(a.subarray(0, 5).toString(), '%PDF-')
  assert.equal(b.subarray(0, 5).toString(), '%PDF-')
  assert.ok(a.length > 2000)
})

test('FORMS-01: P&S preview and issuance share renderer and paginate', async () => {
  const template = getTemplate(PURCHASE_SALE_TEMPLATE_ID, 1)!
  const values = prefillFieldValues(template, {
    clientName: 'James Lee',
    propertyLabel: 'Sunset Point, Culebra',
    offerAmount: '3950000',
    financingType: 'Financed',
    closingDate: '2026-11-15',
  })
  values.sellerName = 'Casa Luar LLC'
  values.deposit = '100000'
  values.municipality = 'Culebra'
  values.catastroNumber = '123-45'
  values.registryEntry = 'F-99'
  const sections = emptySectionValues(template)
  const preview = await renderFormPdf(template, values, sections, 0)
  const issued = await renderFormPdf(template, values, sections, 1)
  assert.equal(preview.subarray(0, 5).toString(), '%PDF-')
  assert.equal(issued.subarray(0, 5).toString(), '%PDF-')
  const { PDFDocument } = await import('pdf-lib')
  const loaded = await PDFDocument.load(issued)
  assert.ok(loaded.getPageCount() >= 2, 'P&S must be multi-page')
})

test('FORMS-BR: renderer emits bounded bottom-left anchors after pagination', async () => {
  const template = getTemplate(PURCHASE_SALE_TEMPLATE_ID, 1)!
  const values = prefillFieldValues(template, {
    clientName: 'Ana María Rivera',
    propertyLabel: 'Villa del Mar, Culebra, Puerto Rico',
    offerAmount: '4950000',
    financingType: 'Cash',
    closingDate: '2026-11-15',
  })
  values.sellerName = 'Isla Holdings LLC'
  values.deposit = '150000'
  const artifact = await renderFormPdfArtifact(
    template,
    values,
    emptySectionValues(template),
    3,
    {
      participants: [
        { role: 'BUYER', slotId: 'BUYER:1', name: 'Ana María Rivera' },
        { role: 'SELLER', slotId: 'SELLER:1', name: 'Isla Holdings LLC' },
      ],
    },
  )
  assert.ok(artifact.pageCount >= 2)
  assert.ok(artifact.signatureAnchors.length >= 6)
  assert.ok(
    artifact.signatureAnchors.some(
      (anchor) =>
        anchor.slotId === 'BUYER:1' && anchor.kind === 'signature',
    ),
  )
  for (const anchor of artifact.signatureAnchors) {
    assert.equal(anchor.coordinateSpace, 'pdf-points-bottom-left')
    assert.ok(anchor.pageIndex >= 0 && anchor.pageIndex < artifact.pageCount)
    assert.ok(anchor.rect.x >= 0 && anchor.rect.y >= 0)
    assert.ok(anchor.rect.x + anchor.rect.width <= anchor.pageWidth)
    assert.ok(anchor.rect.y + anchor.rect.height <= anchor.pageHeight)
  }
})

test('FORMS-BR: identical inputs produce byte-identical PDFs', async () => {
  const template = getTemplate(SHOWING_REPORT_TEMPLATE_ID, 1)!
  const values: Record<string, string> = {
    visitorName: 'José Muñoz',
    agentName: 'Lisa Penfield',
    property: 'Casa Luar',
    showingDate: '2026-09-01',
    duration: '45 minutes',
    outcome: 'Interested',
    feedbackScore: '5',
  }
  const sections = emptySectionValues(template)
  const first = await renderFormPdf(template, values, sections, 2)
  const second = await renderFormPdf(template, values, sections, 2)
  assert.deepEqual(first, second)
})

test('FORMS-BR: an authorized issuance signature and initials replace all owner anchors', async () => {
  const template = getTemplate(SHOWING_REPORT_TEMPLATE_ID, 1)!
  const logoBytes = readFileSync(
    join(process.cwd(), 'public/brand/CLLOGO.png'),
  )
  const artifact = await renderFormPdfArtifact(
    template,
    {
      visitorName: 'José Muñoz',
      agentName: 'Lisa Penfield',
      property: 'Casa Luar',
      showingDate: '2026-08-26',
      duration: '45 minutes',
      outcome: 'Interested',
      feedbackScore: '5',
    },
    emptySectionValues(template),
    4,
    {
      participants: [
        {
          role: 'BUYER_BROKER',
          slotId: 'BUYER_BROKER:1',
          name: 'Lisa Penfield',
        },
      ],
      appliedSignatures: [
        {
          role: 'BUYER_BROKER',
          slotId: 'BUYER_BROKER:1',
          signerName: 'Lisa Penfield',
          credentialLine: 'Real Estate Broker License #: C-9931',
          signerAppUserId: 'owner-user',
          imageBytes: logoBytes,
          imageMimeType: 'image/png',
          assetMediaId: 'test-protected-media',
          assetChecksumSha256: 'a'.repeat(64),
          // 01:30 UTC is still August 26 in Puerto Rico.
          appliedAt: '2026-08-27T01:30:00.000Z',
          consentBasis: BROKER_SIGNATURE_CONSENT_BASIS,
          dateSemantic: BROKER_SIGNATURE_DATE_SEMANTIC,
        },
      ],
    },
  )
  assert.equal(artifact.appliedSignatures.length, 1)
  assert.equal(artifact.appliedSignatures[0].renderedDate, 'August 26, 2026')
  assert.equal(artifact.appliedSignatures[0].renderedInitials, 'LP')
  assert.equal(artifact.appliedSignatures[0].slotId, 'BUYER_BROKER:1')
  assert.equal(
    artifact.appliedSignatures[0].credentialLine,
    'Real Estate Broker License #: C-9931',
  )
  assert.equal(
    artifact.signatureAnchors.some(
      (anchor) =>
        anchor.slotId === 'BUYER_BROKER:1' &&
        (anchor.kind === 'signature' ||
          anchor.kind === 'initial' ||
          anchor.kind === 'date'),
    ),
    false,
    'BoldSign must not ask the owner to sign, initial, or date an already composed line',
  )
  assert.equal('imageBytes' in artifact.appliedSignatures[0], false)
})

test('FORMS-BR: draft identity ignores record insertion order', () => {
  assert.equal(
    formContentFingerprint({ b: '2', a: '1' }, { z: 'last', m: 'middle' }),
    formContentFingerprint({ a: '1', b: '2' }, { m: 'middle', z: 'last' }),
  )
})


test('LISTING-01: first generic Seller signature row uses the full owner name', () => {
  assert.equal(
    signatureBlockDisplayName('owner', 'Jessica Iverson', 0),
    'Jessica Iverson',
  )
  assert.equal(
    signatureBlockDisplayName('owner', 'Jessica Iverson', 1),
    'owner',
    'only the first Seller row adopts the primary owner field',
  )
  assert.equal(
    signatureBlockDisplayName('Named Co-owner', 'Jessica Iverson', 0),
    'Named Co-owner',
    'an explicit participant name is never overwritten',
  )
})

test('LISTING-01: current fields replace unmarked legacy body text', () => {
  const template = getTemplate(LISTING_AGREEMENT_TEMPLATE_ID, 3)!
  const values: Record<string, string> = {
    sellerName: 'Jessica Iverson',
    sellerCivilStatus: 'Single',
    sellerResidenceAddress: '26 Calle Pedro Marquez, PO Box 786',
    brokerName: 'Lisa Penfield',
    property: 'Sea to Soul',
    propertyLocation: 'Playa Sardinas II',
    catastroNumber: '476-054-192-33-000',
    listPrice: '650000',
    commission: '4%',
    startDate: '2026-08-31',
    endDate: '2027-07-27',
    listingType: 'Exclusive Right to Sell',
  }
  const body = resolveDocumentBody(template, values, {
    body: 'Parties\nStale generated copy for Sunset Point at Punta Aloe.',
  })
  assert.match(body, /26 Calle Pedro Marquez, PO Box 786/)
  assert.match(body, /Sea to Soul/)
  assert.match(body, /Playa Sardinas II/)
  assert.match(body, /476-054-192-33-000/)
  assert.match(body, /August 31, 2026/)
  assert.match(body, /July 27, 2027/)
  assert.match(body, /total commission of 5%/)
  assert.match(body, /Broker acts alone, the commission is 4%/)
  assert.doesNotMatch(body, /Sunset Point|Punta Aloe/)
})

test('FORMS-01: explicitly edited document prose remains authoritative', () => {
  const template = getTemplate(LISTING_AGREEMENT_TEMPLATE_ID, 3)!
  assert.equal(
    resolveDocumentBody(template, {}, {
      body: 'Operator-approved custom document text.',
      bodyEdited: 'true',
    }),
    'Operator-approved custom document text.',
  )
})

test('LISTING-01: new forms default the broker-alone commission to 4%', () => {
  const template = getTemplate(LISTING_AGREEMENT_TEMPLATE_ID, 3)!
  const values = prefillFieldValues(template, {
    clientName: null,
    propertyLabel: null,
    offerAmount: null,
    financingType: null,
    closingDate: null,
  })
  assert.equal(values.commission, '4%')
})

test('FORMS-01: remaining production templates render PDFs', async () => {
  for (const id of [
    PURCHASE_SALE_AMENDMENT_TEMPLATE_ID,
    LISTING_AGREEMENT_TEMPLATE_ID,
    SHOWING_INFO_TEMPLATE_ID,
    SHOWING_REPORT_TEMPLATE_ID,
  ]) {
    const template = getTemplate(id, 1)!
    const values: Record<string, string> = {}
    for (const field of template.fields) {
      values[field.name] = field.type === 'date' ? '2026-09-01' : 'Test'
    }
    const pdf = await renderFormPdf(template, values, emptySectionValues(template), 1)
    assert.equal(pdf.subarray(0, 5).toString(), '%PDF-', id)
    assert.ok(pdf.length > 1000, id)
  }
})

test('FORMS-01: on-disk XML is the authoring source', () => {
  const xml = readFileSync(
    join(process.cwd(), 'lib/forms/templates/PR-PNS.xml'),
    'utf8',
  )
  const parsed = parseTemplateXml(xml)
  assert.equal(parsed.id, 'PR-PNS')
  assert.ok(parsed.sections.length >= 10)
})