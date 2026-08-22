import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { parseProcessDefinitionXml, type ParsedProcessDefinition } from '../xml'
import { validateParsedDefinition } from './validate-definition'

// ---------------------------------------------------------------------------
// RE_supermodel-v1 — authoritative XML definition loader.
//
// The XML file is the single source of truth; this module only reads, parses,
// and validates it. It never re-encodes the model in TypeScript. Consumers
// (tests, the generic deployment script) use `parseReSupermodel()` and get a
// fully validated `ParsedProcessDefinition`.
//
// ENG-14: parsing runs the four-layer validation pipeline (XML well-formedness
// + engine grammar via parseProcessDefinitionXml, generic graph semantics via
// graph-validator, and the application contract via validateParsedDefinition —
// every <command-node> must have a router case in the canonical command
// inventory). A command-node added to the XML without a router case, an
// unreachable node, an unsupported node type, or an impossible fork/join
// structure fails here at deploy time and in every test. The generic
// deployment script (deploy-process-definition.ts) accepts ANY workflow XML by
// design, so it stays domain-free; this loader is the RE_supermodel-specific
// deploy-time guard with the same four-layer pipeline.
// ---------------------------------------------------------------------------

export const RE_SUPERMODEL_KEY = 'RE_supermodel'
export const RE_SUPERMODEL_VERSION = 1

const XML_FILE_URL = new URL('./RE_supermodel-v1.xml', import.meta.url)

export function reSupermodelXmlSource(): string {
  return readFileSync(fileURLToPath(XML_FILE_URL), 'utf-8')
}

/** Parse + validate the RE_supermodel XML (all four layers). Throws on failure. */
export function parseReSupermodel(): ParsedProcessDefinition {
  const parsed = parseProcessDefinitionXml(reSupermodelXmlSource())
  const validation = validateParsedDefinition(parsed)
  if (!validation.valid) {
    throw new Error(
      `RE_supermodel-v1.xml failed validation:\n${validation.errors
        .map((e) => `  - ${e}`)
        .join('\n')}`,
    )
  }
  return parsed
}
