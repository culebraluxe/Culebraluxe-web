import assert from 'node:assert/strict'
import test from 'node:test'

import { driveForgeStory } from '../forge/forge-executor'

test('ENG-FORGE-V10: production driver refuses to invent a role runner', async () => {
  await assert.rejects(
    () => driveForgeStory('ENG-FORGE-V10'),
    /requires an explicit real role runner/i,
  )
})
