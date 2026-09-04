import { parseTestMode, type TestMode } from './test-mode'

function section(text: string, name: string): string | null {
  const start = `<<<${name}>>>`
  const end = `<<<END_${name}>>>`
  const from = text.indexOf(start)
  if (from < 0) return null
  const to = text.indexOf(end, from + start.length)
  if (to < 0) return null
  const value = text.slice(from + start.length, to).trim()
  return value || null
}

export type ScoutHandoff = {
  contextRefs: string
}

export function parseScoutHandoff(text: string | null | undefined): ScoutHandoff | null {
  if (!text) return null
  const contextRefs = section(text, 'SCOUT_CONTEXT_REFS')
  return contextRefs ? { contextRefs } : null
}

export type ArchitectHandoff = {
  architectBrief: string
  acceptanceCriteria: string
  assayCommands: string
  testMode: TestMode
}

export function parseArchitectHandoff(
  text: string | null | undefined,
): ArchitectHandoff | null {
  if (!text) return null
  const architectBrief = section(text, 'ARCHITECT_BRIEF')
  const acceptanceCriteria = section(text, 'ARCHITECT_ACCEPTANCE')
  const assayCommands = section(text, 'ARCHITECT_ASSAY_COMMANDS')
  const modeRaw = text.match(/^\s*ARCHITECT_TEST_MODE:\s*(SCOPED|FULL|NONE)\s*$/im)?.[1]
  const testMode = parseTestMode(modeRaw)
  if (!architectBrief || !acceptanceCriteria || !assayCommands || !testMode) return null
  return { architectBrief, acceptanceCriteria, assayCommands, testMode }
}

export const SCOUT_HANDOFF_INSTRUCTIONS = [
  'Your output is the durable Scout -> Architect handoff. End with exactly:',
  '<<<SCOUT_CONTEXT_REFS>>>',
  '<ranked files, signatures, repo facts, and 3-7 must-read files>',
  '<<<END_SCOUT_CONTEXT_REFS>>>',
].join('\n')

export const ARCHITECT_HANDOFF_INSTRUCTIONS = [
  'Your output is the durable Architect -> Lead contract. Do not implement.',
  'End with these exact machine sections; each section must be non-empty:',
  '<<<ARCHITECT_BRIEF>>>',
  '<architecture, constraints, ownership boundaries, dependencies, implementation contract>',
  '<<<END_ARCHITECT_BRIEF>>>',
  '<<<ARCHITECT_ACCEPTANCE>>>',
  '<objective acceptance criteria>',
  '<<<END_ARCHITECT_ACCEPTANCE>>>',
  'ARCHITECT_TEST_MODE: SCOPED | FULL | NONE',
  '<<<ARCHITECT_ASSAY_COMMANDS>>>',
  '<one approved deterministic verification command per line>',
  '<<<END_ARCHITECT_ASSAY_COMMANDS>>>',
].join('\n')
