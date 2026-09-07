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
export const FORGE_SDLC_VERSION = 3

const XML_FILE_URL = new URL('./FORGE_SDLC-v3.xml', import.meta.url)

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

// ---------------------------------------------------------------------------
// ENG-FORGE-V12-C / Scope C + review park — FORGE_SDLC-v4 (INACTIVE).
//
// v4 is the next immutable workflow definition. It mirrors v3 for all non-FAST
// behavior and adds: (1) a true FAST lane (Smith -> deterministic QA -> publish
// -> complete, no Lead/human/DEV_OPS) and (2) an Architect review park that
// STOPS for human judgment before implementation (reusing the existing `hold`
// wait + resume — NOT a new end state).
//
// FORGE_SDLC_VERSION stays 3 (ACTIVE). v4 is not reachable until it is seeded
// into process_definitions (version 4) in DEV+PROD and this constant flips to
// 4 in a coordinated activation. This loader exists so the definition is
// parse-validated BEFORE activation — never activated broken.
// ---------------------------------------------------------------------------
export const FORGE_SDLC_V4_VERSION = 4

const XML_V4_FILE_URL = new URL('./FORGE_SDLC-v4.xml', import.meta.url)

export function forgeSdlcV4XmlSource(): string {
  return readFileSync(fileURLToPath(XML_V4_FILE_URL), 'utf-8')
}

/** Parse + validate FORGE_SDLC-v4 (INACTIVE). Throws on failure. */
export function parseForgeSdlcV4(): ParsedProcessDefinition {
  const parsed = parseProcessDefinitionXml(forgeSdlcV4XmlSource())
  const validation = validateParsedDefinition(parsed, forgeCommandIsRouted)
  if (!validation.valid) {
    throw new Error(
      `FORGE_SDLC-v4.xml failed validation:\n${validation.errors
        .map((e) => `  - ${e}`)
        .join('\n')}`,
    )
  }
  return parsed
}
