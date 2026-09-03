import assert from 'node:assert/strict'
import test from 'node:test'

import {
  FORGE_SLACK_WEBHOOK_URL,
  buildSlackMessage,
  forgePlayerLabel,
  forgeSlackWebhookUrl,
  isSlackConfigured,
  postSlackNotification,
  redactForSlack,
  type ForgeSlackContext,
  type SlackWebhookPost,
} from './slack-notifier'

const WEBHOOK = 'https://hooks.slack.com/services/T000/B000/secret'

type RecordedRequest = {
  url: string
  method?: string
  headers?: Record<string, string>
  body?: string
  signal?: AbortSignal
}

function recordingPost(records: RecordedRequest[], response?: { ok: boolean; status: number }): SlackWebhookPost {
  return async (url, init) => {
    records.push({ url, ...init })
    return response ?? { ok: true, status: 200 }
  }
}

function startedContext(overrides: Partial<ForgeSlackContext> = {}): ForgeSlackContext {
  return {
    event: 'lane-started',
    storyId: 'ENG-FORGE-V4-11',
    workItemId: 'aw-test-1',
    storyTitle: 'Slack Run Notifications',
    role: 'builder',
    modelProfile: 'builder-flash',
    ...overrides,
  }
}

test('disabled notifier: no webhook URL means no request and no throw', async () => {
  const records: RecordedRequest[] = []
  const post = recordingPost(records)

  for (const env of [{}, { [FORGE_SLACK_WEBHOOK_URL]: '' }, { [FORGE_SLACK_WEBHOOK_URL]: '   ' }]) {
    const delivered = await postSlackNotification(startedContext(), { env, post })
    assert.equal(delivered, false)
  }
  assert.equal(records.length, 0)
  assert.equal(isSlackConfigured({}), false)
  assert.equal(forgeSlackWebhookUrl({ [FORGE_SLACK_WEBHOOK_URL]: '' }), null)
  assert.equal(
    forgeSlackWebhookUrl({ [FORGE_SLACK_WEBHOOK_URL]: '  https://hooks.slack.com/services/x  ' }),
    'https://hooks.slack.com/services/x',
  )
})

test('payload requires both durable identifiers (story id + work item id)', () => {
  assert.equal(buildSlackMessage(startedContext({ workItemId: '' })), null)
  assert.equal(buildSlackMessage(startedContext({ storyId: '  ' })), null)
  const text = buildSlackMessage(startedContext())
  assert.ok(text)
  assert.match(text, /story ENG-FORGE-V4-11 \| work item aw-test-1/)
})

test('successful POST sends one concise JSON payload with the durable identifiers', async () => {
  const records: RecordedRequest[] = []
  const post = recordingPost(records)

  const delivered = await postSlackNotification(
    startedContext({
      event: 'lane-completed',
      resultStatus: 'Complete',
      completion: 100,
      commitHash: '5db17bf89d3a715b95eec52f40630e3d945bd98f',
      externalRunId: 'deepseek-session-abc123',
    }),
    { env: { [FORGE_SLACK_WEBHOOK_URL]: WEBHOOK }, post },
  )

  assert.equal(delivered, true)
  assert.equal(records.length, 1)
  assert.equal(records[0]!.url, WEBHOOK)
  assert.equal(records[0]!.method, 'POST')
  assert.equal(records[0]!.headers?.['content-type'], 'application/json')

  const body = JSON.parse(records[0]!.body ?? '{}') as { text?: string }
  assert.ok(typeof body.text === 'string' && body.text.length > 0)
  assert.match(body.text!, /Smith completed/)
  assert.match(body.text!, /story ENG-FORGE-V4-11 \| work item aw-test-1/)
  assert.match(body.text!, /Slack Run Notifications/)
  assert.match(body.text!, /result Complete · 100%/)
  assert.match(body.text!, /commit 5db17bf89d3a715b95eec52f40630e3d945bd98f/)
  assert.match(body.text!, /external run deepseek-session-abc123/)
  assert.ok(!body.text!.includes('secret'))
})

test('started and follow messages stay concise and carry role/profile/to-lane', () => {
  const started = buildSlackMessage(startedContext())!
  assert.match(started, /Smith started/)
  assert.match(started, /Smith \(builder-flash\)/)

  const follow = buildSlackMessage(
    startedContext({ event: 'lane-follow', role: 'builder', toLane: 'assay' }),
  )!
  assert.match(follow, /follows to Assay/)
  assert.match(follow, /Smith \(builder-flash\) → Assay/)

  assert.equal(forgePlayerLabel('builder'), 'Smith')
  assert.equal(forgePlayerLabel('verifier'), 'Assay')
  assert.equal(forgePlayerLabel('reviewer'), 'Assay')
  assert.equal(forgePlayerLabel('assay'), 'Assay')
  assert.equal(forgePlayerLabel('custom'), 'Custom')
})

test('terminal messages carry the outcome and never leak secrets or transcripts', () => {
  const text = buildSlackMessage(
    startedContext({
      event: 'lane-terminal',
      role: 'verifier',
      modelProfile: 'verifier-mini',
      resultStatus: 'Assay Failed',
      detail:
        'failed commands: pnpm test agent-runtime/x.test.ts | token=sk-abcdefghijklmnopqrstuvwxyz | DATABASE_URL=postgres://user:pass@example.com/db',
    }),
  )!
  assert.match(text, /Assay terminal/)
  assert.match(text, /outcome Assay Failed/)
  assert.match(text, /story ENG-FORGE-V4-11 \| work item aw-test-1/)
  assert.ok(!text.includes('sk-abcdefghijklmnopqrstuvwxyz'))
  assert.ok(!text.includes('postgres://user:pass@example.com/db'))
  assert.ok(!text.includes('token='))

  assert.match(redactForSlack('see https://hooks.slack.com/services/a/b with Bearer abc')!, /\[redacted\]/)
})

test('fail-open: network/HTTP failure never throws and never mutates anything', async () => {
  const cases: Array<{ label: string; post: SlackWebhookPost }> = [
    {
      label: 'network error',
      post: async () => {
        throw new Error('ECONNREFUSED')
      },
    },
    {
      label: 'non-2xx response',
      post: async () => ({ ok: false, status: 500 }),
    },
    {
      label: 'invalid response object',
      post: async () => ({ ok: 'yes', status: 200 } as never),
    },
  ]
  for (const scenario of cases) {
    const delivered = await postSlackNotification(startedContext(), {
      env: { [FORGE_SLACK_WEBHOOK_URL]: WEBHOOK },
      post: scenario.post,
      log: () => {},
    })
    assert.equal(delivered, false, scenario.label)
  }
})

test('fail-open: malformed or non-https webhook URL makes no request', async () => {
  const records: RecordedRequest[] = []
  const post = recordingPost(records)

  for (const env of [
    { [FORGE_SLACK_WEBHOOK_URL]: 'http://hooks.slack.com/services/x' },
    { [FORGE_SLACK_WEBHOOK_URL]: 'not-a-url' },
    { [FORGE_SLACK_WEBHOOK_URL]: 'ftp://hooks.slack.com/services/x' },
  ]) {
    const delivered = await postSlackNotification(startedContext(), {
      env,
      post,
      log: () => {},
    })
    assert.equal(delivered, false)
  }
  assert.equal(records.length, 0)
})

test('fail-open: a timed-out delivery aborts the request and resolves false', async () => {
  const post: SlackWebhookPost = (_url, init) =>
    new Promise((_resolve, reject) => {
      init.signal?.addEventListener('abort', () => reject(new Error('aborted: timeout')))
    })

  const startedAt = Date.now()
  const delivered = await postSlackNotification(startedContext(), {
    env: { [FORGE_SLACK_WEBHOOK_URL]: WEBHOOK },
    post,
    timeoutMs: 60,
    log: () => {},
  })
  const elapsed = Date.now() - startedAt

  assert.equal(delivered, false)
  assert.ok(elapsed >= 40, `expected the abort timeout to fire (elapsed ${elapsed}ms)`)
})
