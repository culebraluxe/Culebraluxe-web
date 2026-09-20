import { readFileSync, existsSync } from 'node:fs'
import { join } from 'node:path'

export type CouncilPin = {
  repo: string
  tag: string | null
  sha: string
  adapterVersion: string
  protocolFiles: string[]
  ignored: string[]
}

export type VendorProtocol = {
  pin: CouncilPin
  checkoutSha: string | null
  skillPresent: boolean
  agentIds: string[]
  drift: string | null
}

const PIN_REL = join('vendor', 'council-pin.json')
const VENDOR_REL = join('vendor', 'council-of-high-intelligence')

export function loadPin(root = process.cwd()): CouncilPin {
  const path = join(root, PIN_REL)
  if (!existsSync(path)) {
    throw new Error(`COUNCIL_PIN_MISSING: ${PIN_REL} is required`)
  }
  const raw = JSON.parse(readFileSync(path, 'utf8')) as CouncilPin
  if (!raw.sha || !raw.adapterVersion) {
    throw new Error('COUNCIL_PIN_INVALID: sha and adapterVersion are required')
  }
  return raw
}

export function inspectVendor(root = process.cwd()): VendorProtocol {
  const pin = loadPin(root)
  const vendorRoot = join(root, VENDOR_REL)
  const headFile = join(vendorRoot, '.git')
  const skill = join(vendorRoot, 'SKILL.md')
  const agentsDir = join(vendorRoot, 'agents')

  const checkoutSha = existsSync(join(vendorRoot, '.git'))
    ? readCheckoutSha(vendorRoot)
    : null

  const skillPresent = existsSync(skill)
  const agentIds = existsSync(agentsDir)
    ? listAgentIds(agentsDir)
    : []

  let drift: string | null = null
  if (!existsSync(vendorRoot)) {
    drift = 'vendor checkout missing — run git submodule update --init'
  } else if (checkoutSha && checkoutSha !== pin.sha) {
    drift = `checkout ${checkoutSha} != pin ${pin.sha}`
  } else if (!skillPresent) {
    drift = 'SKILL.md missing from vendor checkout'
  }

  return { pin, checkoutSha, skillPresent, agentIds, drift }
}

function readCheckoutSha(vendorRoot: string): string | null {
  // Submodule .git is often a file: "gitdir: ../../.git/modules/..."
  // Prefer reading HEAD via git in the sync script. Here we only sniff.
  const headPath = join(vendorRoot, 'HEAD')
  if (existsSync(headPath)) {
    const text = readFileSync(headPath, 'utf8').trim()
    if (/^[0-9a-f]{40}$/i.test(text)) return text.toLowerCase()
  }
  return null
}

function listAgentIds(agentsDir: string): string[] {
  // Keep this sync and tiny. The service does not parse 18 novels at request time.
  try {
    const { readdirSync } = require('node:fs') as typeof import('node:fs')
    return readdirSync(agentsDir)
      .filter((name) => name.startsWith('council-') && name.endsWith('.md'))
      .map((name) => name.replace(/\.md$/, ''))
      .sort()
  } catch {
    return []
  }
}
