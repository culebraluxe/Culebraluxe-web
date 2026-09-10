import assert from 'node:assert/strict'
import test from 'node:test'

import { sql } from '../../db/client'
import { getStoryboardStory } from '../../db/storyboard'

// REGRESSION (2026-09-10): `batch_deploy` was added to the StoryboardStory type and
// the row mapper but NOT to the SELECT lists in db/storyboard.ts. The mapper read an
// absent column, `batchDeploy` was silently false for every story, and the batch
// rollout hold in the role runner could never fire — a DEV_OPS lane published a
// batch story to main as a result. A mapped field is useless if the query omits it,
// so this test reads through the REAL path.
//
// DB-backed (DEV). Skipped when no database is configured.

const dbConfigured = Boolean(process.env.DATABASE_URL_DEV || process.env.DATABASE_URL)
const DEV = 'development'

test(
  'getStoryboardStory returns batch_deploy through the real read path',
  { skip: dbConfigured ? false : 'DB not configured (run with .env.local on demand)' },
  async () => {
    process.env.APP_ENV = DEV
    const story = `BATCHDEPLOY-PROBE-${Date.now()}`
    try {
      await sql`
        insert into storyboard_story (id, workstream, title, priority, status, notes, batch, batch_deploy, completion, rollup)
        values (${story}, 'ENGINEERING', 'batch deploy probe', 'Medium', 'Planned', 'temp', 2, true, 0, false)
      `
      const on = await getStoryboardStory(story)
      assert.equal(on?.batchDeploy, true, 'a batch-deploy story must read back as batchDeploy=true')
      assert.equal(on?.batch, 2)

      await sql`update storyboard_story set batch_deploy = false where id = ${story}`
      const off = await getStoryboardStory(story)
      assert.equal(off?.batchDeploy, false)
    } finally {
      await sql`delete from storyboard_story where id = ${story}`
    }
  },
)
