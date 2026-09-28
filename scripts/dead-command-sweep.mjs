#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Dead-command sweep — the `pnpm` menu, measured against the banner.
//
// `pnpm broken:ts:sweep` answers "which FILES cannot load". This answers "which
// COMMANDS cannot run", because the command menu is what an operator types, and a
// command that names a banner file fails with ERR_MODULE_NOT_FOUND before it does
// anything. It touches no database and no network.
//
//   node scripts/dead-command-sweep.mjs            # the three blocks, and the count
//   node scripts/dead-command-sweep.mjs --check    # fail when the count RISES
//   node scripts/dead-command-sweep.mjs --format json
//
// The ledger those blocks feed is docs/agent/DEAD-COMMANDS.md. The count may only
// fall: porting a command removes its name from here, and a new dead command is a
// finding, not a budget.
// ---------------------------------------------------------------------------
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const BANNER = 'BROKEN ON PURPOSE'

/// The total at the last sweep (docs/agent/DEAD-COMMANDS.md). Lower it when it falls.
const BASELINE = 53

/// The three blocks: who owns the command, in the order they are worked.
const BLOCKS = [
  { key: 'forge', label: 'Forge / story operator surface', test: /^(forge:|story:|story-|sprint$|health$|test:story|test:sprint)/ },
  { key: 'agent-runtime', label: 'agent runtime', test: /^agent:/ },
  { key: 'retired-stack', label: 'retired server stack', test: /.*/ },
]

const packageJson = JSON.parse(fs.readFileSync(path.join(ROOT, 'package.json'), 'utf8'))

/// Every `scripts/…` or `agent-runtime/…` file a command names, with its banner state.
function referencedFiles(command) {
  const matches = [
    ...command.matchAll(/((?:scripts|agent-runtime)\/[A-Za-z0-9_.\/-]+\.(?:ts|mts|mjs|js))/g),
  ].map((match) => match[1])
  return matches.map((file) => {
    const absolute = path.join(ROOT, file)
    if (!fs.existsSync(absolute)) return { file, state: 'missing' }
    const head = fs.readFileSync(absolute, 'utf8').slice(0, 600)
    return { file, state: head.includes(BANNER) ? 'dead' : 'live' }
  })
}

const dead = []
const missingOnly = []
for (const [name, command] of Object.entries(packageJson.scripts)) {
  const files = referencedFiles(command)
  if (files.length === 0) continue
  if (files.some((entry) => entry.state === 'dead')) dead.push({ name, files })
  else if (files.some((entry) => entry.state === 'missing')) missingOnly.push({ name, files })
}

function blockOf(name) {
  return BLOCKS.find((block) => block.test.test(name)) ?? BLOCKS[BLOCKS.length - 1]
}

if (process.argv.includes('--format') && process.argv.includes('json')) {
  console.log(
    JSON.stringify(
      {
        totalScripts: Object.keys(packageJson.scripts).length,
        deadCommands: dead.length,
        baseline: BASELINE,
        missingFileCommands: missingOnly.map((entry) => entry.name),
        blocks: BLOCKS.map((block) => ({
          key: block.key,
          label: block.label,
          commands: dead
            .filter((entry) => blockOf(entry.name).key === block.key)
            .map((entry) => ({
              command: entry.name,
              files: entry.files.filter((file) => file.state === 'dead').map((file) => file.file),
            })),
        })),
      },
      null,
      2,
    ),
  )
} else {
  console.log(`package.json scripts: ${Object.keys(packageJson.scripts).length}`)
  console.log(`commands naming a banner file: ${dead.length} (baseline ${BASELINE})`)
  for (const block of BLOCKS) {
    const commands = dead.filter((entry) => blockOf(entry.name).key === block.key)
    console.log(`\n== ${block.label} — ${commands.length}`)
    for (const entry of commands) {
      const files = entry.files.filter((file) => file.state === 'dead').map((file) => file.file)
      console.log(`  ${entry.name}\t${files.join(' ')}`)
    }
  }
  if (missingOnly.length > 0) {
    console.log(`\n== commands naming a MISSING file (not a banner) — ${missingOnly.length}`)
    for (const entry of missingOnly) console.log(`  ${entry.name}\t${entry.files.map((f) => f.file).join(' ')}`)
  }
}

if (process.argv.includes('--check')) {
  if (dead.length > BASELINE) {
    console.error(
      `\nREFUSING: ${dead.length} commands name a banner file, up from ${BASELINE}. ` +
        'A new one is a finding: repoint it at Rust, or record why not in the same commit ' +
        '(docs/agent/DEAD-COMMANDS.md).',
    )
    process.exit(1)
  }
  if (dead.length < BASELINE) {
    console.error(
      `\nThe count FELL to ${dead.length}, below the baseline of ${BASELINE}. ` +
        'Lower BASELINE in this file and the count in docs/agent/DEAD-COMMANDS.md.',
    )
    process.exit(1)
  }
  console.log(`\nok: ${dead.length} dead commands, at the baseline.`)
}
