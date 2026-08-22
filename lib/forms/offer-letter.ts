// ---------------------------------------------------------------------------
// DOC-07 — Offer Letter TemplateDefinition (adapter/fixture).
//
// The POC proof form: NEXUS → Forms → Offer Letter. Fields use existing
// canonical data where available (bindings) and explicit user-entered fields
// where the domain has no canonical fact yet. One bounded editable section
// (Special Terms) per the order.
//
// A future story can replace this fixture with a small human-editable
// declarative format behind the same TemplateDefinition seam.
// ---------------------------------------------------------------------------

import type { TemplateDefinition } from './template-types'

export const OFFER_LETTER_TEMPLATE_ID = 'OFFER-01'

export const OFFER_LETTER_TEMPLATE: TemplateDefinition = {
  id: OFFER_LETTER_TEMPLATE_ID,
  version: 1,
  displayName: 'Offer Letter',
  documentTypeLabel: 'Offer Letter',
  fields: [
    {
      name: 'buyerName',
      label: 'Buyer / Client',
      type: 'text',
      required: true,
      binding: 'deal.client.name',
    },
    {
      name: 'property',
      label: 'Property',
      type: 'text',
      required: true,
      binding: 'deal.property.label',
    },
    {
      name: 'offerAmount',
      label: 'Offer amount',
      type: 'money',
      required: true,
      binding: 'deal.offer.amount',
    },
    {
      name: 'deposit',
      label: 'Deposit',
      type: 'money',
      required: false,
      binding: null,
    },
    {
      name: 'financing',
      label: 'Cash / Financing',
      type: 'select',
      required: true,
      binding: 'deal.financing.type',
      options: ['Cash', 'Financed'],
    },
    {
      name: 'closingDate',
      label: 'Proposed closing date',
      type: 'date',
      required: true,
      binding: 'deal.closing.date',
    },
    {
      name: 'expiration',
      label: 'Offer expiration',
      type: 'date',
      required: true,
      binding: null,
    },
    {
      name: 'contingencies',
      label: 'Contingencies',
      type: 'textarea',
      required: false,
      binding: null,
    },
  ],
  sections: [
    {
      name: 'specialTerms',
      label: 'Special Terms',
      editable: true,
    },
  ],
  rendering: {
    title: 'OFFER LETTER',
    issuer: 'CulebraLuxe Real Estate',
  },
}
