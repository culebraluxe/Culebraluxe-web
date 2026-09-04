import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { parseProcessDefinitionXml, type ParsedProcessDefinition } from '../xml'
import { validateParsedDefinition } from './validate-definition'

// ---------------------------------------------------------------------------
// FORGE_SDLC-v1 — authoritative XML definition loader.
//
// The XML file is the single source of truth; this module only reads, parses,
// and validates it. It never re-encodes the model in TypeScript. Consumers
// (tests, the generic deployment script) use `parseForgeSdlc()` and get a
// fully validated `ParsedProcessDefinition`.
//
// Same four-layer pipeline as RE_supermodel (ENG-14): XML well-formedness +
// engine grammar via parseProcessDefinitionXml, generic graph semantics via
// graph-validator, and the application contract via validateParsedDefinition.
// v1 carries NO <command-node> elements (serial human/agent backbone only),
// so Layer 4 passes vacuously; any future forge.* command-node must gain a
// router case plus a canonical handler first, or the deploy fails here exactly
// as it does for RE_supermodel.
// ---------------------------------------------------------------------------

export const FORGE_SDLC_KEY = 'FORGE_SDLC'
export const FORGE_SDLC_VERSION = 1

const XML_FILE_URL = new URL('./FORGE_SDLC-v1.xml', import.meta.url)

export function forgeSdlcXmlSource(): string {
  return readFileSync(fileURLToPath(XML_FILE_URL), 'utf-8')
}

/** Parse + validate the FORGE_SDLC XML (all four layers). Throws on failure. */
export function parseForgeSdlc(): ParsedProcessDefinition {
  const parsed = parseProcessDefinitionXml(forgeSdlcXmlSource())
  const validation = validateParsedDefinition(parsed)
  if (!validation.valid) {
    throw new Error(
      `FORGE_SDLC-v1.xml failed validation:\n${validation.errors
        .map((e) => `  - ${e}`)
        .join('\n')}`,
    )
  }
  return parsed
}
