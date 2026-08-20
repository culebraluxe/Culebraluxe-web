import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import {
  parseProcessDefinitionXml,
  validateProcessGraph,
  type ParsedProcessDefinition,
} from '../xml'

// ---------------------------------------------------------------------------
// RE_supermodel-v1 — authoritative XML definition loader.
//
// The XML file is the single source of truth; this module only reads, parses,
// and validates it. It never re-encodes the model in TypeScript. Consumers
// (tests, the generic deployment script) use `parseReSupermodel()` and get a
// fully validated `ParsedProcessDefinition`.
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
  return parsed
}
