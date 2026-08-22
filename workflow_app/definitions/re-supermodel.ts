import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { assertCommandNodesRouted } from '../command-types'
import {
  parseProcessDefinitionXml,
  validateProcessGraph,
  type ParsedProcessDefinition,
} from '../xml'
import type { NodeDefinition } from '../../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// RE_supermodel-v1 — authoritative XML definition loader.
//
// The XML file is the single source of truth; this module only reads, parses,
// and validates it. It never re-encodes the model in TypeScript. Consumers
// (tests, the generic deployment script) use `parseReSupermodel()` and get a
// fully validated `ParsedProcessDefinition`.
//
// CRM-14G: parsing additionally enforces that every <command-node> in the XML
// has a router case (workflow_app/command-types.ts inventory). A command-node
// added to the XML without a router case fails here at deploy time and in every
// test. The generic deployment script (deploy-process-definition.ts) accepts
// ANY workflow XML by design, so it stays domain-free; this loader is the
// RE_supermodel-specific deploy-time guard.
// ---------------------------------------------------------------------------

export const RE_SUPERMODEL_KEY = 'RE_supermodel'
export const RE_SUPERMODEL_VERSION = 1

const XML_FILE_URL = new URL('./RE_supermodel-v1.xml', import.meta.url)

export function reSupermodelXmlSource(): string {
  return readFileSync(fileURLToPath(XML_FILE_URL), 'utf-8')
}

/** Parse + validate the RE_supermodel XML. Throws on any parse/validation failure. */
export function parseReSupermodel(): ParsedProcessDefinition {
  const parsed = parseProcessDefinitionXml(reSupermodelXmlSource())
  const validation = validateProcessGraph(parsed.graph)
  if (!validation.valid) {
    throw new Error(
      `RE_supermodel-v1.xml failed validation:\n${validation.errors
        .map((e) => `  - ${e}`)
        .join('\n')}`,
    )
  }
  // CRM-14G deploy-time guard: every XML command-node must have a router case.
  const commandNodeTypes = Object.values(parsed.graph.nodes)
    .filter(
      (n): n is NodeDefinition & { commandType: string } =>
        n.type === 'command' && typeof n.commandType === 'string' && n.commandType.length > 0,
    )
    .map((n) => n.commandType)
  const unrouted = assertCommandNodesRouted(commandNodeTypes)
  if (unrouted.length > 0) {
    throw new Error(
      `RE_supermodel-v1.xml command-nodes have no router case: ${unrouted.join(
        ', ',
      )} — add the router case in workflow_app/command-router.ts (inventory: workflow_app/command-types.ts)`,
    )
  }
  return parsed
}
