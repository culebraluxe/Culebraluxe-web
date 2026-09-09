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

export type ProjectWorkStatus = "complete" | "waiting" | "in-progress" | "not-started" | "blocked"

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
  owner?: string
  note?: string
  relatedLabel?: string
  children?: ProjectWorkNode[]
}

export type ProjectPlan = {
  id: string
  title: string
  kind: string
  status: "active" | "planning" | "hold" | "complete"
  progress: number
  phaseLabel: string
  nextAction?: string
  nextActionDetail?: string
  blocker?: string
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

export type ProjectsWorkspaceData = {
  domains: ProjectDomain[]
  poles: ProjectPole[]
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
