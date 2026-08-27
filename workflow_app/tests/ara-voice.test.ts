import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

// ---------------------------------------------------------------------------
// ARA-VOICE-01 — microphone input for the Ara command path.
//
// Source/shape guards. The invariant: voice is INPUT ONLY — recognized speech
// fills the SAME editable prompt text used by typing, and submission happens
// through the existing typed command handler (Go / Enter). Speech recognition
// NEVER auto-submits an Ara command.
// ---------------------------------------------------------------------------

test('ara-mic: reusable control feature-detects and never auto-submits', () => {
  const src = readFileSync(
    new URL('../../components/portal/ara-mic-button.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(/AraMicButton/.test(src), 'component exported')
  assert.ok(/SpeechRecognition/.test(src) && /webkitSpeechRecognition/.test(src), 'feature-detects native speech recognition')
  assert.ok(/onTranscript/.test(src), 'reports recognized speech to the parent')
  assert.ok(!/submit\(|onRun|onAsk/.test(src), 'the mic never invokes a command itself')
  assert.ok(/onerror/.test(src), 'handles recognition errors honestly')
  assert.ok(/onend/.test(src), 'returns to idle when recognition ends')
  assert.ok(/setListening\(false\)/.test(src), 'listening state clears on stop/error/end')
  assert.ok(/\.stop\(\)/.test(src), 'provides stop/cancel back to idle')
  assert.ok(/unsupported/.test(src), 'degrades cleanly when the browser lacks speech support')
})

test('ara-mic: CatchUp command keeps typed path and gains the mic', () => {
  const src = readFileSync(
    new URL('../../components/portal/catch-up-command.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(/AraMicButton/.test(src), 'mic present on the Ara command surface')
  assert.ok(/onRun/.test(src), 'existing typed command handler preserved')
  assert.ok(/setPrompt/.test(src), 'prompt stays editable')
  assert.ok(/\bGo\b/.test(src), 'explicit submit preserved')
})

test('ara-mic: Forms Grok helper reuses the shared mic and keeps its command path', () => {
  const src = readFileSync(
    new URL('../../components/portal/forms/form-grok-helper.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(/AraMicButton/.test(src), 'reuses the shared mic (implemented once)')
  assert.ok(/onAsk/.test(src), 'existing typed Forms command handler preserved')
  assert.ok(/setPrompt/.test(src), 'prompt stays editable (transcript correctable)')
  assert.ok(/\bGo\b/.test(src), 'explicit submit preserved')
  assert.ok(
    !/webkitSpeechRecognition/.test(src),
    'no duplicated inline voice logic in the Forms helper',
  )
})

test('mac-sync-cal-02: operational wrapper + LaunchAgent manager exist with install/status/run/stop mechanics', () => {
  const mjs = readFileSync(
    new URL('../../scripts/calendar-sync-agent.mjs', import.meta.url),
    'utf8',
  )
  for (const cmd of ['install', 'status', 'run', 'stop', 'uninstall']) {
    assert.ok(new RegExp(`case '${cmd}'`).test(mjs), `manager supports ${cmd}`)
  }
  assert.ok(/StartInterval/.test(mjs) || /CADENCE_SECONDS/.test(mjs), 'cadence is configurable')

  const wrapper = readFileSync(
    new URL('../../scripts/macbridge/sync-calendar-eventkit.sh', import.meta.url),
    'utf8',
  )
  assert.ok(/CalendarEventKit.swift/.test(wrapper), 'runner invokes the existing EventKit bridge')
  assert.ok(/attempted-at=/.test(wrapper), 'logs attempted-at')
  assert.ok(/result=success/.test(wrapper) && /result=failure/.test(wrapper), 'logs success/failure')
  assert.ok(/generated-at=/.test(wrapper), 'logs snapshot generated-at')
  assert.ok(/events=/.test(wrapper), 'logs event count')
})
