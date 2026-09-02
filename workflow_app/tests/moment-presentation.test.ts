import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  channelLine,
  cleanPreview,
  headerDirectionLabel,
  humanDirection,
  sourceContextMoment,
} from '../../lib/relationship-intel/moment-presentation'

// ---------------------------------------------------------------------------
// REL-INTEL — Contact History presentation helpers.
// ---------------------------------------------------------------------------

test('humanDirection: inbound is client → Lisa, outbound is Lisa → client', () => {
  assert.equal(humanDirection('inbound', 'Ana Rivera'), 'Ana Rivera → Lisa')
  assert.equal(humanDirection('outbound', 'Ana Rivera'), 'Lisa → Ana Rivera')
  assert.equal(humanDirection('two-way', 'Ami'), 'Ami ↔ Lisa')
  assert.equal(humanDirection(null, 'Ana Rivera'), null)
})

test('humanDirection: falls back to a neutral client label when the name is blank', () => {
  assert.equal(humanDirection('inbound', ''), 'Client → Lisa')
  assert.equal(humanDirection('outbound', '  '), 'Lisa → Client')
})

test('headerDirectionLabel: maps single-event direction to generic header words', () => {
  assert.equal(headerDirectionLabel('inbound'), 'Inbound')
  assert.equal(headerDirectionLabel('outbound'), 'Outbound')
  assert.equal(headerDirectionLabel(null), null)
})

test('channelLine: single event is the bare label; burst includes count', () => {
  assert.equal(channelLine('iMessage', false, 1), 'iMessage')
  assert.equal(channelLine('Email', false, 1), 'Email')
  assert.equal(channelLine('Call', false, 1), 'Call')
  assert.equal(channelLine('iMessage', true, 47), 'iMessage conversation · 47 messages')
  assert.equal(channelLine('iMessage', true, 1), 'iMessage conversation · 1 message')
})

test('cleanPreview: real text is preserved; pure replacement chars collapse to null', () => {
  assert.equal(cleanPreview('Did you watch video'), 'Did you watch video')
  assert.equal(cleanPreview('  Let\'s meet   before you leave...  '), "Let's meet before you leave...")
  assert.equal(cleanPreview('\ufffc\ufffc\ufffc\ufffc\ufffc'), null)
  assert.equal(cleanPreview('\ufffd\ufeff'), null)
  assert.equal(cleanPreview('Ready\ufffcto go'), 'Ready to go')
  assert.equal(cleanPreview(null), null)
  assert.equal(cleanPreview(''), null)
})

test('sourceContextMoment: preview keeps its own timestamp and direction', () => {
  assert.deepEqual(sourceContextMoment({
    lastContext: 'New all 4 offerings',
    lastContextAt: '2026-09-01T22:28:01.953Z',
    lastContextDirection: 'inbound',
    lastContactAt: '2026-09-02T15:42:04.659Z',
    lastDirection: 'outbound',
  }), {
    preview: 'New all 4 offerings',
    timestamp: '2026-09-01T22:28:01.953Z',
    direction: 'inbound',
  })
})
