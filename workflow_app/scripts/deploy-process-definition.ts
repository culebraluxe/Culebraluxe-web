// ---------------------------------------------------------------------------
// Generic workflow definition deployment command (Story 127 / 133).
//
// Canonical path:
//
//   XML file -> parse -> validate -> ProcessGraph -> upsertProcessDefinition
//
// Usage:
//
//   node_modules/.bin/tsx workflow_app/scripts/deploy-process-definition.ts \
//     workflow_app/definitions/RE_supermodel-v1.xml
//
//   node_modules/.bin/tsx workflow_app/scripts/deploy-process-definition.ts \
//     workflow_app/definitions/RE_supermodel-v1.xml --dry-run
//
// The pipeline is generic: it accepts ANY file matching the workflow XML
// grammar. Versioned and idempotent ONLY while the (key, version) row has no
// instances; once instances exist the version is immutable and a new version
// must be deployed (see version-policy.ts).
//
// --dry-run parses and validates the definition and prints the deployment plan
// WITHOUT importing the database layer or performing any write.
// ---------------------------------------------------------------------------

import { readFile } from 'node:fs/promises'
import { parseProcessDefinitionXml, validateProcessGraph } from '../xml'

async function main(filePath: string, dryRun: boolean): Promise<void> {
  const source = await readFile(filePath, 'utf-8')
  const parsed = parseProcessDefinitionXml(source)
  const validation = validateProcessGraph(parsed.graph)

  if (!validation.valid) {
    for (const err of validation.errors) console.error(`  ✗ ${err}`)
    throw new Error(
      `'${filePath}' failed validation (${validation.errors.length} error(s))`,
    )
  }
  for (const warn of validation.warnings) console.warn(`  ⚠ ${warn}`)

  const nodeCount = Object.keys(parsed.graph.nodes).length
  const startNodeId = parsed.graph.startNodeId

  console.log(`Definition: ${parsed.key} v${parsed.version}`)
  console.log(`  name        : ${parsed.name}`)
  console.log(`  description : ${parsed.description ?? '(none)'}`)
  console.log(`  start node  : ${startNodeId}`)
  console.log(`  nodes       : ${nodeCount}`)
  console.log(`  display order: ${parsed.displayOrder.length} node(s)`)

  if (dryRun) {
    console.log('\nDRY RUN — no database write performed.')
    return
  }

  // Import the database layer only for a real deployment (dry-run stays DB-free).
  const { upsertProcessDefinition } = await import('../definitions/deploy')
  const result = await upsertProcessDefinition({
    key: parsed.key,
    version: parsed.version,
    name: parsed.name,
    description: parsed.description,
    graph: parsed.graph,
    createdBy: 'crm14',
  })

  console.log(
    `\n${parsed.key} v${parsed.version} ${result.created ? 'created' : 'updated'} (definition id ${result.id})`,
  )
}

const args = process.argv.slice(2)
const dryRun = args.includes('--dry-run')
const filePath = args.find((a) => !a.startsWith('--'))

if (!filePath) {
  console.error(
    'Usage: tsx workflow_app/scripts/deploy-process-definition.ts <definition.xml> [--dry-run]',
  )
  process.exit(1)
}

main(filePath, dryRun).catch((err) => {
  console.error('Deployment failed:', err)
  process.exit(1)
})
