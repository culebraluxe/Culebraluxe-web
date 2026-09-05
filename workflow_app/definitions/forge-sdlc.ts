import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { parseProcessDefinitionXml, type ParsedProcessDefinition } from '../xml'
import { validateParsedDefinition } from './validate-definition'
import { forgeCommandIsRouted } from '../forge-command-types'

// ---------------------------------------------------------------------------
// FORGE_SDLC-v2 — authoritative XML definition loader.
//
// The XML file is the single source of truth; this module only reads, parses,
// and validates it. It never re-encodes the model in TypeScript. Consumers
// (tests, the generic deployment script) use `parseForgeSdlc()` and get a
// fully validated `ParsedProcessDefinition`.
//
// v1 is deployed + immutable once it has instances; this v2 adds the ENG-FORGE-
// V11-S1 closed-loop QA failure routing (qa_failure_route) and the FAST_LANE
// (workType FAST -> Smith -> operator confirmation, no auto-QA/DEV_OPS).
//
// Same four-layer pipeline as RE_supermodel (ENG-14): XML well-formedness +
// engine grammar via parseProcessDefinitionXml, generic graph semantics via
// graph-validator, and the application contract via validateParsedDefinition.
// Layer 4 here is the FORGE inventory (forge-command-types.ts), NOT the RE
// registry — the two models are cleanly forked. Every release-critical
// <command-node> must have a Forge-owned router case and canonical handler.
// ---------------------------------------------------------------------------

export const FORGE_SDLC_KEY = 'FORGE_SDLC'
export const FORGE_SDLC_VERSION = 2

const XML_FILE_URL = new URL('./FORGE_SDLC-v2.xml', import.meta.url)

export function forgeSdlcXmlSource(): string {
  return readFileSync(fileURLToPath(XML_FILE_URL), 'utf-8')
}

/** Parse + validate the FORGE_SDLC XML (all four layers). Throws on failure. */
export function parseForgeSdlc(): ParsedProcessDefinition {
  const parsed = parseProcessDefinitionXml(forgeSdlcXmlSource())
  const validation = validateParsedDefinition(parsed, forgeCommandIsRouted)
  if (!validation.valid) {
    throw new Error(
      `FORGE_SDLC-v1.xml failed validation:\n${validation.errors
        .map((e) => `  - ${e}`)
        .join('\n')}`,
    )
  }
  return parsed
}
