import { test } from 'node:test'
import assert from 'node:assert/strict'

import { formatAddressLine, isRedundantCountry, oneLine } from '../../lib/address-format'

// Real values read from PROD `property.address_line1` for the two Apple Contacts
// addresses that surfaced the bug on Juan A. Santa Cruz's Listing agreement.
// Apple stores these streets on two lines, and that newline is correct source
// data — the defect was that a single-line field rendered it as nothing, so
// "Bo. Delicias" and "Vagabundo Capital LLC" glued into one word.
const VAGABUNDO_STREET = 'Bo. Delicias\nVagabundo Capital LLC'
const HACIENDA_STREET = 'Hacienda Margarita\n174 Calle Carrta'

test('oneLine separates a multi-line Apple street instead of gluing it', () => {
  assert.equal(oneLine(VAGABUNDO_STREET), 'Bo. Delicias, Vagabundo Capital LLC')
  assert.equal(oneLine(HACIENDA_STREET), 'Hacienda Margarita, 174 Calle Carrta')
})

test('oneLine normalizes CRLF and collapses runs of whitespace', () => {
  assert.equal(oneLine('  Bo. Delicias \r\n  Vagabundo   Capital LLC '), 'Bo. Delicias, Vagabundo Capital LLC')
})

test('oneLine treats empty and whitespace-only values as nothing', () => {
  assert.equal(oneLine(null), null)
  assert.equal(oneLine(undefined), null)
  assert.equal(oneLine('   '), null)
  assert.equal(oneLine('\n'), null)
})

test('a Puerto Rico address drops the redundant United States country', () => {
  assert.equal(isRedundantCountry('United States', 'PR'), true)
  assert.equal(isRedundantCountry('United States of America', 'pr'), true)
  assert.equal(isRedundantCountry('U.S.A.', 'PR'), true)
})

test('country is kept when it carries information', () => {
  assert.equal(isRedundantCountry('United States', 'NY'), false)
  assert.equal(isRedundantCountry('Spain', 'PR'), false)
  assert.equal(isRedundantCountry(null, 'PR'), false)
})

test('the Listing agreement address line reads correctly end to end', () => {
  const address = {
    addressLine1: VAGABUNDO_STREET,
    neighborhood: null,
    city: 'Culebra',
    stateOrProvince: 'PR',
    postalCode: '00775',
    country: 'United States',
  }
  assert.equal(formatAddressLine(address), 'Bo. Delicias, Vagabundo Capital LLC, Culebra, PR 00775')
})

test('the Hacienda Margarita address line reads correctly end to end', () => {
  const address = {
    addressLine1: HACIENDA_STREET,
    neighborhood: null,
    city: 'Liquillo',
    stateOrProvince: 'PR',
    postalCode: '00773',
    country: 'United States',
  }
  assert.equal(formatAddressLine(address), 'Hacienda Margarita, 174 Calle Carrta, Liquillo, PR 00773')
})

test('a multi-line street is never glued even without city detail', () => {
  assert.equal(formatAddressLine({ addressLine1: VAGABUNDO_STREET }), 'Bo. Delicias, Vagabundo Capital LLC')
})

test('formatAddressLine returns an empty string for no address', () => {
  assert.equal(formatAddressLine(null), '')
  assert.equal(formatAddressLine(undefined), '')
  assert.equal(formatAddressLine({}), '')
})
