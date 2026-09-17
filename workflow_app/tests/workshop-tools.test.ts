import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  FORGE_TOOL_CATALOG,
  FORGE_TOOL_IDS,
  type ForgeToolId,
  type ForgeToolRole,
} from '../forge/forge-tool-catalog'

// ---------------------------------------------------------------------------
// WORKSHOP-TOOLS-01 — the fence between the workshop docs and the tool catalog.
//
// The check reads the SAME source `pnpm forge:tools` reads: FORGE_TOOL_CATALOG.
// The doc follows the table, never the other way round. A tool whose `wired`
// flag is false must not be presented as available for any position; a wired
// tool's declared positions must equal the catalog roles. The fence fails if the
// doc and the table drift apart again.
// ---------------------------------------------------------------------------

const HERE = dirname(fileURLToPath(import.meta.url))
const REPO_ROOT = resolve(HERE, '..', '..')
const WORKSHOP_DOC = resolve(REPO_ROOT, 'docs/agent/FORGE-WORKSHOP.md')
const SKILLS_README = resolve(REPO_ROOT, 'docs/agent/skills/README.md')

// The `###` heading each tool owns in FORGE-WORKSHOP.md. Cruiser's heading
// carries its package name, so both spellings are accepted.
const TOOL_HEADING_ALIASES: Record<ForgeToolId, string[]> = {
  ripwire: ['ripwire'],
  serena: ['serena'],
  rtk: ['rtk'],
  cruiser: ['cruiser', 'dependency-cruiser'],
  semgrep: ['semgrep'],
  knip: ['knip'],
}

// Stable position tokens as the workshop docs spell them. `lead` means every
// lead position; `none` means the tool is granted to no position.
const POSITION_TO_ROLES: Record<string, ForgeToolRole[]> = {
  scout: ['scout'],
  architect: ['architect'],
  'lead pre': ['lead_pre'],
  pre: ['lead_pre'],
  'lead solo': ['lead_solo'],
  solo: ['lead_solo'],
  'lead post': ['lead_post'],
  post: ['lead_post'],
  lead: ['lead_pre', 'lead_solo', 'lead_post'],
  smith: ['smith'],
  inspector: ['inspector'],
  assay: ['assay'],
  qa: ['inspector'],
  'dev ops': ['dev_ops'],
  dev_ops: ['dev_ops'],
  none: [],
}

function findSection(doc: string, aliases: string[]): string | null {
  const lines = doc.split('\n')
  let start = -1
  for (let i = 0; i < lines.length; i++) {
    const heading = /^###\s+(.+?)\s*$/.exec(lines[i])
    if (!heading) continue
    const text = heading[1].toLowerCase()
    if (aliases.some((alias) => text.startsWith(alias))) {
      start = i
      break
    }
  }
  if (start < 0) return null
  const rest = lines.slice(start + 1)
  const end = rest.findIndex((line) => /^###\s/.test(line))
  return (end < 0 ? rest : rest.slice(0, end)).join('\n')
}

function extractPositions(section: string): string | null {
  const match = /^[-*]\s*Positions:\s*(.+?)\s*$/im.exec(section)
  return match ? match[1] : null
}

function parsePositions(text: string): ForgeToolRole[] {
  const roles = new Set<ForgeToolRole>()
  const normalized = text.replace(/[·•]/g, ',').replace(/\//g, ',').toLowerCase()
  for (const raw of normalized.split(',')) {
    const token = raw.trim().replace(/\s+/g, ' ')
    if (!token) continue
    const mapped = POSITION_TO_ROLES[token]
    if (!mapped) {
      throw new Error(`unrecognized position token "${token}" in "${text}"`)
    }
    for (const role of mapped) roles.add(role)
  }
  return [...roles]
}

function wiringTableCanRun(readme: string): Map<string, string> {
  const lines = readme.split('\n')
  const header = lines.findIndex((line) => /^\|\s*tool\s*\|\s*class\s*\|/.test(line))
  assert.ok(header >= 0, 'docs/agent/skills/README.md must keep the Wiring status table')
  const out = new Map<string, string>()
  for (let i = header + 2; i < lines.length; i++) {
    const line = lines[i].trim()
    if (!line.startsWith('|')) break
    const id = /^\|\s*`([^`]+)`\s*\|/.exec(line)
    if (!id) continue
    const cells = line.split('|').map((cell) => cell.trim()).filter(Boolean)
    out.set(id[1], cells[cells.length - 1] ?? '')
  }
  return out
}

test('FORGE-WORKSHOP.md keeps a section for every catalog tool', () => {
  const doc = readFileSync(WORKSHOP_DOC, 'utf8')
  for (const id of FORGE_TOOL_IDS) {
    const section = findSection(doc, TOOL_HEADING_ALIASES[id])
    assert.ok(section, `FORGE-WORKSHOP.md is missing a ### section for ${id}`)
  }
})

test('a wired tool lists exactly the positions the catalog grants', () => {
  const doc = readFileSync(WORKSHOP_DOC, 'utf8')
  for (const id of FORGE_TOOL_IDS) {
    const declaration = FORGE_TOOL_CATALOG[id]
    if (!declaration.wired) continue
    const section = findSection(doc, TOOL_HEADING_ALIASES[id])
    assert.ok(section)
    const positions = extractPositions(section)
    assert.ok(positions, `${id} has no Positions: line`)
    assert.deepEqual(
      parsePositions(positions).sort(),
      [...declaration.roles].sort(),
      `${id} positions in the doc must equal the catalog roles`,
    )
  }
})

test('an unwired tool is never presented as available for a position', () => {
  const doc = readFileSync(WORKSHOP_DOC, 'utf8')
  for (const id of FORGE_TOOL_IDS) {
    const declaration = FORGE_TOOL_CATALOG[id]
    if (declaration.wired) continue
    const section = findSection(doc, TOOL_HEADING_ALIASES[id])
    assert.ok(section)
    assert.match(
      section,
      /not wired|unwired|wired:\s*(no|false)/i,
      `${id} is unwired and its section must say so`,
    )
    const positions = extractPositions(section)
    assert.deepEqual(
      positions ? parsePositions(positions) : [],
      [],
      `${id} is unwired and must not be listed for any position`,
    )
  }
})

test('the Classes line never names an unwired tool as available', () => {
  const doc = readFileSync(WORKSHOP_DOC, 'utf8')
  const classes = findSection(doc, ['classes'])
  assert.ok(classes, 'FORGE-WORKSHOP.md must keep a ### Classes section')
  const bullets = classes.split('\n').filter((line) => /^[-*]\s+\*\*/.test(line))
  assert.ok(bullets.length > 0, 'the Classes section must list class bullets')
  for (const bullet of bullets) {
    const paren = /\(([^)]*)\)\s*\.?\s*$/.exec(bullet.trim())
    if (!paren) continue
    for (const rawName of paren[1].split(',')) {
      const name = rawName.trim().toLowerCase().replace(/[^a-z0-9]/g, '')
      if (!name) continue
      const id = FORGE_TOOL_IDS.find((tool) => tool.replace(/[^a-z0-9]/g, '') === name)
      if (!id) continue
      assert.equal(
        FORGE_TOOL_CATALOG[id].wired,
        true,
        `the Classes line names ${id} as available but it is not wired`,
      )
    }
  }
})

test('the skills README wiring table agrees with the catalog', () => {
  const readme = readFileSync(SKILLS_README, 'utf8')
  const canRun = wiringTableCanRun(readme)
  for (const id of FORGE_TOOL_IDS) {
    assert.ok(canRun.has(id), `docs/agent/skills/README.md is missing a wiring row for ${id}`)
    const cell = canRun.get(id) ?? ''
    if (FORGE_TOOL_CATALOG[id].wired) {
      assert.match(cell, /^yes\b/i, `${id} is wired but the README says "${cell}"`)
    } else {
      assert.match(cell, /^no\b/i, `${id} is unwired but the README says "${cell}"`)
    }
  }
})
