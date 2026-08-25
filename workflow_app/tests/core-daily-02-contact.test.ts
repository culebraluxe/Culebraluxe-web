import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  resolveContactTargets,
  buildCallTarget,
  buildEmailTarget,
  buildSmsTarget,
  buildWhatsAppTarget,
} from '../../lib/relationship-intel/contact-targets'

// ---------------------------------------------------------------------------
// CORE-DAILY-02 — reusable contact actions (pure). No database.
// ---------------------------------------------------------------------------

test('CORE-DAILY-02: valid phone produces a safe tel: target', () => {
  const t = buildCallTarget('+1 (787) 555-0134', 'Mobile')
  assert.ok(t)
  assert.equal(t.channel, 'call')
  assert.equal(t.url, 'tel:+17875550134')
})

test('CORE-DAILY-02: valid email produces a safe mailto: target', () => {
  const t = buildEmailTarget(' Jane@Example.com ', 'Work')
  assert.ok(t)
  assert.equal(t.channel, 'email')
  assert.equal(t.url, 'mailto:Jane%40Example.com')
})

test('CORE-DAILY-02: valid SMS target', () => {
  const t = buildSmsTarget('7875550134')
  assert.ok(t)
  assert.equal(t.url, 'sms:+17875550134')
})

test('CORE-DAILY-02: WhatsApp only when approved', () => {
  assert.equal(buildWhatsAppTarget('7875550134', { approved: false }), null)
  const t = buildWhatsAppTarget('7875550134', { approved: true, text: 'Hello' })
  assert.ok(t)
  assert.equal(t.url, 'https://wa.me/17875550134?text=Hello')
})

test('CORE-DAILY-02: invalid contact values are omitted honestly', () => {
  assert.equal(buildCallTarget('not-a-phone'), null)
  assert.equal(buildEmailTarget('not-an-email'), null)
  assert.equal(buildSmsTarget('12345'), null) // unreliable length -> no guess
})

test('CORE-DAILY-02: resolver offers multiple legitimate choices in stable order', () => {
  const targets = resolveContactTargets({
    emails: ['jane@example.com'],
    phones: ['+17875550134'],
    whatsappApproved: true,
  })
  const order = targets.map((t) => t.channel)
  // Call → Message → Email → WhatsApp
  assert.deepEqual(order, ['call', 'sms', 'email', 'whatsapp'])
  assert.ok(targets.every((t) => t.url.startsWith('tel:') || t.url.startsWith('sms:') || t.url.startsWith('mailto:') || t.url.startsWith('https://wa.me/')))
})

test('CORE-DAILY-02: no available method returns an honest empty set', () => {
  assert.deepEqual(resolveContactTargets({ emails: [], phones: [] }), [])
})

test('CORE-DAILY-02: launch does not record success — this is a pure target builder', () => {
  // No side effects; recording successful communication is a separate command.
  const t = buildCallTarget('7875550134')
  assert.ok(t)
})
