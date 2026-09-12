export { ProjectsWorkspaceController } from "./projects-controller"
export {
  PROJECTS_PANE_ORDER,
  PROJECTS_GEOMETRY,
  PROJECTS_GRID_TEMPLATE,
  PROJECTS_SURFACE,
  PROJECTS_PRIMITIVES,
  PROJECTS_LONG_CONTENT,
  PROJECTS_SCROLL_CLASS,
  type ProjectsPaneRole,
  type ProjectsSurfaceFamily,
  type ProjectsPrimitiveSurface,
  type ProjectsLongContentPolicy,
} from "./visual-system"
export {
  InMemoryProjectsWorkspaceSource,
  PROJECTS_WORKSPACE_FIXTURE,
  type ProjectsWorkspaceSource,
} from "./source"
export {
  mapProjectCalendarItems,
  mapProjectCalendarToEvents,
  type ProjectCalendarItem,
} from "./secondary-projection"
export { mapProjectToTimeline, type ProjectTimeline as ProjectTimelineModel } from "./timeline-projection"
export { mapProjectToFileTree, type ProjectDocuments } from "./documents-projection"
export type {
  ProjectDomain,
  ProjectDomainKey,
  ProjectPlan,
  ProjectPole,
  ProjectSecondaryViewProvenance,
  ProjectSecondaryViewStatus,
  ProjectWorkNode,
  ProjectWorkNodeType,
  ProjectWorkStatus,
  ProjectsWorkspaceData,
  ProjectsWorkspaceLoadState,
  ProjectsWorkspacePageModel,
  ProjectsWorkspaceStatus,
  ProjectWorkspaceView,
} from "./model"
