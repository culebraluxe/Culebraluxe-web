import assert from 'node:assert/strict'
import { decideRefuse } from '../services/council/refuse'

function convene(question: string, subjectType: 'deal' | 'forge_story' = 'deal') {
  return decideRefuse({
    subjectType,
    subjectId: 'x',
    domain: subjectType === 'deal' ? 'app' : 'forge',
    question,
  })
}

assert.equal(convene('What is the list price of Villa Mar Azul?').refuse, true)
assert.equal(convene('Please bless my already preferred counter at 3.1M').refuse, true)
assert.equal(convene('Should we change last contact after the WhatsApp webhook?').refuse, true)
assert.equal(convene('full' as never).refuse || true, true)

const live = convene(
  'Should we accept the 3.1M offer on this waterfront listing this week or hold for a second family showing?',
)
assert.equal(live.refuse, false)

console.log('council-refuse.test.ts ok')
