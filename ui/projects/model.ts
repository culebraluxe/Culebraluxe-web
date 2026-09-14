import type { ProjectCalendarItem } from "./secondary-projection"

export type ProjectDomainKey =
  | "properties"
  | "people"
  | "deals"
  | "firm"
  | "marketing"
  | "accounting"

export type ProjectDomain = {
  key: ProjectDomainKey
  label: string
  shortLabel: string
}

export type ProjectWorkStatus = "complete" | "waiting" | "in-progress" | "not-started" | "blocked" | "dismissed"

export type ProjectWorkNodeType =
  | "group"
  | "task"
  | "milestone"
  | "contract"
  | "document"
  | "media"
  | "appointment"
  | "accounting"
  | "approval"
  | "workflow"

export type ProjectWorkNode = {
  id: string
  title: string
  type: ProjectWorkNodeType
  status: ProjectWorkStatus
  dueLabel?: string
  dueAt?: string
  owner?: string
  note?: string
  relatedLabel?: string
  /**
   * Canonical entity link the node is anchored to (`person`/`property`/`contract`/
   * `deal` + stable id). Labels for the inspector are resolved from identityNames,
   * never invented, and never matched by display name.
   */
  entity?: { type: string; id: string }
  children?: ProjectWorkNode[]
  /**
   * Selected-work content kept on the WorkNode so the MVI model, not JSX,
   * remains the source for the compact bottom inspector.
   */
  inspector?: {
    summary?: string
    relatedItems?: Array<{ label: string; caption?: string }>
  }
  /** Selected-work actions supplied by the model. */
  actions?: string[]
}

export type ProjectAssetKind = "document" | "photo"
export type ProjectAssetSource = "vault" | "property-media"

/**
 * Unified READ MODEL for the Documents tab. It never changes source ownership:
 * Vault documents remain Vault documents and photos remain Property media.
 */
export type ProjectAsset = {
  id: string
  /** Native id in the authoritative source, kept separate from the projected id. */
  sourceId: string
  kind: ProjectAssetKind
  name: string
  source: ProjectAssetSource
  propertyId: string | null
  createdAt: string | null
  href?: string
  state?: string
  caption?: string | null
  altText?: string | null
  mimeType?: string | null
  fileSize?: number | null
}

/**
 * Provenance of a project's secondary panes. `linked` = an anchor exists and
 * records matched it; `empty` = an anchor exists but no records matched;
 * `unlinked` = no anchor exists, so the pane must explain the absence instead
 * of inventing content. Never computed from display-name equality.
 */
export type ProjectSecondaryViewStatus = "linked" | "unlinked" | "empty"

export type ProjectSecondaryViewProvenance = {
  /** Documents tab provenance now reflects all project assets (Vault + photos). */
  documents: ProjectSecondaryViewStatus
  activity: ProjectSecondaryViewStatus
  calendar: ProjectSecondaryViewStatus
}

export type ProjectPlan = {
  id: string
  title: string
  kind: string
  status: "active" | "planning" | "hold" | "complete" | "archived"
  progress: number
  phaseLabel: string
  playbookId?: string
  playbookVersion?: number
  contextLabels?: string[]
  anchorSource?: 'row' | 'wbs' | 'none'
  provenance?: ProjectSecondaryViewProvenance
  nextAction?: string
  nextActionDetail?: string
  blocker?: string
  calendarItems?: ProjectCalendarItem[]
  documents?: Array<{ id: string; title: string; state: string; propertyId: string | null; createdAt: string }>
  /** Project-scoped view over documents + photos; storage remains in source services. */
  assets?: ProjectAsset[]
  activity?: Array<{ id: string; channel: string; direction: string | null; occurredAt: string; occurredAtLabel: string; title: string | null; summary: string | null; personName: string | null; propertyName: string | null }>
  workNodes: ProjectWorkNode[]
}

export type ProjectPole = {
  id: string
  domain: ProjectDomainKey
  label: string
  subtitle: string
  progress: number
  statusLabel: string
  projects: ProjectPlan[]
}

/**
 * Discriminated server-composed load state for the workspace. `ready` and
 * `empty` are successful reads (empty = zero projects); `unauthorized` is an
 * authorization denial; `failure` is a service/DB error. Kept distinct so the
 * view never renders one state as another.
 */
export type ProjectsWorkspaceStatus = "ready" | "empty" | "unauthorized" | "failure"

export type ProjectsWorkspaceLoadState = {
  status: ProjectsWorkspaceStatus
  message?: string
}

export type ProjectsWorkspaceData = {
  domains: ProjectDomain[]
  poles: ProjectPole[]
  loadState?: ProjectsWorkspaceLoadState
}

export type ProjectWorkspaceView =
  | "work-plan"
  | "timeline"
  | "calendar"
  | "documents"
  | "financials"
  | "activity"

export type ProjectsWorkspacePageModel = {
  data: ProjectsWorkspaceData | null
  query: string
  activeDomain: ProjectDomainKey
  expandedPoleIds: string[]
  selectedPoleId: string | null
  selectedProjectId: string | null
  selectedNodeId: string | null
  activeView: ProjectWorkspaceView
  loading: boolean
  error: string | null
}

export const INITIAL_PROJECTS_WORKSPACE_MODEL: ProjectsWorkspacePageModel = {
  data: null,
  query: "",
  activeDomain: "properties",
  expandedPoleIds: [],
  selectedPoleId: null,
  selectedProjectId: null,
  selectedNodeId: null,
  activeView: "work-plan",
  loading: true,
  error: null,
}

type EmptyPayload = Record<string, never>

export type ProjectsWorkspaceIntentMap = {
  "projects.load": { request: EmptyPayload; response: void }
  "projects.queryChanged": { request: { query: string }; response: void }
  "projects.selectDomain": { request: { domain: ProjectDomainKey }; response: void }
  "projects.togglePole": { request: { poleId: string }; response: void }
  "projects.selectPole": { request: { poleId: string }; response: void }
  "projects.selectProject": { request: { poleId: string; projectId: string }; response: void }
  "projects.selectNode": { request: { nodeId: string | null }; response: void }
  "projects.selectView": { request: { view: ProjectWorkspaceView }; response: void }
}
