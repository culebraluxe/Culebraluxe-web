import { test } from 'node:test'
import assert from 'node:assert/strict'
import { parsePacketLoop } from './loop'
import { sessionFieldsFromGitPacket } from './git-packet'

test('parses repair loop block', () => {
  const loop = parsePacketLoop(
    'intent: repair\nparent_run: abc\nfailed_commands:\n- node --test agent-runtime/loop.test.ts\nloop: 2/3',
  )
  assert.equal(loop.intent, 'repair')
  assert.equal(loop.parentRun, 'abc')
  assert.match(loop.failedCommands[0] ?? '', /loop\.test/)
  assert.equal(loop.loopLabel, '2/3')
})

test('packet heading Loop is parsed', () => {
  const fields = sessionFieldsFromGitPacket(
    '# S\n\n## Architect brief\nfix it\n\n## Loop\nintent: grow\nloop: 1/3\n',
  )
  assert.match(fields.loop ?? '', /intent: grow/)
})
