import {
  BasePageController,
  type PageOperationContext,
  type PageOperationDefinitions,
} from "@/ui/runtime"
import {
  INITIAL_PROJECTS_WORKSPACE_MODEL,
  type ProjectDomainKey,
  type ProjectsWorkspaceIntentMap,
  type ProjectsWorkspacePageModel,
} from "./model"
import type { ProjectsWorkspaceSource } from "./source"

export class ProjectsWorkspaceController extends BasePageController<
  ProjectsWorkspacePageModel,
  ProjectsWorkspaceIntentMap
> {
  protected readonly operations: PageOperationDefinitions<
    ProjectsWorkspacePageModel,
    ProjectsWorkspaceIntentMap
  >

  constructor(
    private readonly source: ProjectsWorkspaceSource,
    initialModel: ProjectsWorkspacePageModel = INITIAL_PROJECTS_WORKSPACE_MODEL,
  ) {
    super({ ...initialModel, expandedPoleIds: [...initialModel.expandedPoleIds] })

    this.operations = {
      "projects.load": {
        description: "Load the Projects workspace and choose a deterministic initial Pole/Project selection.",
        execution: "latest",
        handle: async (_request, context) => this.load(context),
      },
      "projects.queryChanged": {
        description: "Update the local navigator query without touching the source.",
        execution: "parallel",
        handle: async ({ query }, context) => {
          context.update((model) => ({ ...model, query }))
        },
      },
      "projects.selectDomain": {
        description: "Switch the fixed domain rail and select the first Pole/Project in that domain.",
        execution: "parallel",
        handle: async ({ domain }, context) => {
          const data = context.snapshot().data
          const firstPole = data?.poles.find((pole) => pole.domain === domain) ?? null
          const firstProject = firstPole?.projects[0] ?? null
          context.update((model) => ({
            ...model,
            activeDomain: domain,
            selectedPoleId: firstPole?.id ?? null,
            expandedPoleIds: firstPole ? [firstPole.id] : [],
            selectedProjectId: firstProject?.id ?? null,
            selectedNodeId: firstProject?.workNodes[0]?.id ?? null,
            activeView: "work-plan",
          }))
        },
      },
      "projects.togglePole": {
        description: "Expand or collapse one Pole while keeping the selected domain fixed.",
        execution: "parallel",
        handle: async ({ poleId }, context) => {
          context.update((model) => {
            const expanded = model.expandedPoleIds.includes(poleId)
            return {
              ...model,
              expandedPoleIds: expanded
                ? model.expandedPoleIds.filter((id) => id !== poleId)
                : [...model.expandedPoleIds, poleId],
            }
          })
        },
      },
      "projects.selectPole": {
        description: "Select a Pole, expand it, and choose its first Project.",
        execution: "parallel",
        handle: async ({ poleId }, context) => {
          const data = context.snapshot().data
          const pole = data?.poles.find((candidate) => candidate.id === poleId) ?? null
          const project = pole?.projects[0] ?? null
          context.update((model) => ({
            ...model,
            activeDomain: pole?.domain ?? model.activeDomain,
            selectedPoleId: poleId,
            expandedPoleIds: model.expandedPoleIds.includes(poleId)
              ? model.expandedPoleIds
              : [...model.expandedPoleIds, poleId],
            selectedProjectId: project?.id ?? null,
            selectedNodeId: project?.workNodes[0]?.id ?? null,
            activeView: "work-plan",
          }))
        },
      },
      "projects.selectProject": {
        description: "Select a Project under its canonical Pole without creating a second navigation home.",
        execution: "parallel",
        handle: async ({ poleId, projectId }, context) => {
          const data = context.snapshot().data
          const pole = data?.poles.find((candidate) => candidate.id === poleId) ?? null
          const project = pole?.projects.find((candidate) => candidate.id === projectId) ?? null
          context.update((model) => ({
            ...model,
            activeDomain: pole?.domain ?? model.activeDomain,
            selectedPoleId: poleId,
            expandedPoleIds: model.expandedPoleIds.includes(poleId)
              ? model.expandedPoleIds
              : [...model.expandedPoleIds, poleId],
            selectedProjectId: projectId,
            selectedNodeId: project?.workNodes[0]?.id ?? null,
            activeView: "work-plan",
          }))
        },
      },
      "projects.selectNode": {
        description: "Select a WBS node for the inspector while leaving Project context intact.",
        execution: "parallel",
        handle: async ({ nodeId }, context) => {
          context.update((model) => ({ ...model, selectedNodeId: nodeId }))
        },
      },
      "projects.selectView": {
        description: "Change the middle-pane projection of the selected Project.",
        execution: "parallel",
        handle: async ({ view }, context) => {
          context.update((model) => ({ ...model, activeView: view }))
        },
      },
    }
  }

  private async load(context: PageOperationContext<ProjectsWorkspacePageModel>): Promise<void> {
    context.update((model) => ({ ...model, loading: true, error: null }))
    try {
      const data = await this.source.load({ signal: context.signal })
      if (!context.isCurrent()) return

      const activeDomain = this.resolveDomain(data.poles.map((pole) => pole.domain), context.snapshot().activeDomain)
      const firstPole = data.poles.find((pole) => pole.domain === activeDomain) ?? null
      const firstProject = firstPole?.projects[0] ?? null

      context.update((model) => ({
        ...model,
        data,
        activeDomain,
        selectedPoleId: firstPole?.id ?? null,
        expandedPoleIds: firstPole ? [firstPole.id] : [],
        selectedProjectId: firstProject?.id ?? null,
        selectedNodeId: firstProject?.workNodes[0]?.id ?? null,
        loading: false,
        error: null,
      }))
    } catch (error) {
      if (context.signal.aborted) return
      const message = error instanceof Error ? error.message : String(error)
      context.update((model) => ({ ...model, loading: false, error: message }))
    }
  }

  private resolveDomain(domains: ProjectDomainKey[], preferred: ProjectDomainKey): ProjectDomainKey {
    return domains.includes(preferred) ? preferred : domains[0] ?? "properties"
  }
}
