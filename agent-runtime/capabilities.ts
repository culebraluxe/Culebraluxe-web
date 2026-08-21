// ---------------------------------------------------------------------------
// Capability model for concrete runtime adapters (ENG-18).
//
// A runtime advertises which capabilities it can satisfy. The invoker uses
// these for deterministic eligibility (profile/capability availability) and
// future adapters (browser, connectors) advertise more. Capability names are
// generic and provider-neutral.
// ---------------------------------------------------------------------------

/** Vendor-neutral capability vocabulary. */
export type AgentCapability =
  // A. WORKSPACE / GIT
  | 'workspace.fs.read'
  | 'workspace.fs.write'
  | 'workspace.fs.delete'
  | 'git.status'
  | 'git.diff'
  | 'git.history'
  | 'git.commit'
  | 'git.push'
  // B. DATA / CONFIG
  | 'data.db.read'
  | 'data.db.write'
  | 'data.schema.migrate'
  | 'storyboard.read'
  | 'storyboard.write'
  | 'config.read'
  // C. HOST EXECUTION
  | 'host.exec'
  | 'host.process'
  | 'host.logs'
  | 'host.tests'
  | 'host.typecheck'
  | 'host.build'
  | 'host.lint'
  | 'host.repo-scripts'
  // D. BROWSER / UI (optional future)
  | 'browser.dom'
  | 'browser.screenshot'
  | 'browser.interact'
  // E. EXTERNAL CONNECTORS / DEPLOYMENT (optional future)
  | 'connector.github'
  | 'connector.vercel'
  | 'connector.neon'

export const CORE_CAPABILITIES: AgentCapability[] = [
  'workspace.fs.read',
  'workspace.fs.write',
  'workspace.fs.delete',
  'git.status',
  'git.diff',
  'git.history',
  'git.commit',
  'data.db.read',
  'data.db.write',
  'data.schema.migrate',
  'storyboard.read',
  'storyboard.write',
  'config.read',
  'host.exec',
  'host.process',
  'host.logs',
  'host.tests',
  'host.typecheck',
  'host.build',
  'host.lint',
  'host.repo-scripts',
]

export const OPTIONAL_FUTURE_CAPABILITIES: AgentCapability[] = [
  'git.push',
  'browser.dom',
  'browser.screenshot',
  'browser.interact',
  'connector.github',
  'connector.vercel',
  'connector.neon',
]

export function hasCapability(
  capabilities: AgentCapability[],
  required: AgentCapability,
): boolean {
  return capabilities.includes(required)
}

export function hasAllCapabilities(
  capabilities: AgentCapability[],
  required: AgentCapability[],
): boolean {
  return required.every((c) => capabilities.includes(c))
}
