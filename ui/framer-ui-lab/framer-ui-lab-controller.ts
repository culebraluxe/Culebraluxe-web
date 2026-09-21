import {
  BasePageController,
  type PageOperationDefinitions,
} from "@/ui/runtime"
import {
  FRAMER_LAB_DEALS,
  FRAMER_LAB_HOTSPOTS,
  FRAMER_LAB_PROGRESS_STEPS,
  FRAMER_LAB_PROPERTIES,
  INITIAL_FRAMER_UI_LAB_MODEL,
  type FramerLabCoreCardId,
  type FramerLabCoreTab,
  type FramerLabFilterStatus,
  type FramerLabQuickAction,
  type FramerLabSortKey,
  type FramerUiLabIntentMap,
  type FramerUiLabPageModel,
} from "./model"

const CORE_TABS: readonly FramerLabCoreTab[] = ["overview", "activity", "documents"]
const DEAL_STATUSES: readonly FramerLabFilterStatus[] = ["all", "active", "attention", "closed"]
const SORT_KEYS: readonly FramerLabSortKey[] = ["client", "property", "stage", "value"]
const CORE_CARD_IDS: readonly FramerLabCoreCardId[] = ["client-brief", "deal-readiness", "next-action"]
const QUICK_ACTIONS: readonly FramerLabQuickAction[] = ["call", "email", "task", "document"]

export class FramerUiLabController extends BasePageController<
  FramerUiLabPageModel,
  FramerUiLabIntentMap
> {
  protected readonly operations: PageOperationDefinitions<
    FramerUiLabPageModel,
    FramerUiLabIntentMap
  >

  constructor(initialModel: FramerUiLabPageModel = INITIAL_FRAMER_UI_LAB_MODEL) {
    super({ ...initialModel })

    this.operations = {
      "framerLab.toggleMotion": {
        description: "Enable or disable all motion candidates without changing their visual composition.",
        execution: "parallel",
        handle: (_request, context) => {
          context.update((model) => ({ ...model, motionEnabled: !model.motionEnabled }))
        },
      },
      "framerLab.selectMotionProfile": {
        description: "Change the candidate motion language while keeping the same lab content.",
        execution: "parallel",
        handle: ({ profile }, context) => {
          context.update((model) => ({ ...model, motionProfile: profile }))
        },
      },
      "framerLab.selectHeroMode": {
        description: "Compare a still hero against the cinematic treatment.",
        execution: "parallel",
        handle: ({ mode }, context) => {
          context.update((model) => ({ ...model, heroMode: mode }))
        },
      },
      "framerLab.selectProperty": {
        description: "Select one property for the layout-transition candidate.",
        execution: "parallel",
        handle: ({ propertyId }, context) => {
          if (!FRAMER_LAB_PROPERTIES.some((property) => property.id === propertyId)) return
          context.update((model) => ({ ...model, selectedPropertyId: propertyId }))
        },
      },
      "framerLab.selectHotspot": {
        description: "Select one visual annotation on the interactive property image.",
        execution: "parallel",
        handle: ({ hotspotId }, context) => {
          if (!FRAMER_LAB_HOTSPOTS.some((hotspot) => hotspot.id === hotspotId)) return
          context.update((model) => ({ ...model, selectedHotspotId: hotspotId }))
        },
      },
      "framerLab.setCommandPaletteOpen": {
        description: "Open or close the command palette candidate.",
        execution: "parallel",
        handle: ({ open }, context) => {
          context.update((model) => ({
            ...model,
            commandPaletteOpen: open,
            commandQuery: open ? model.commandQuery : "",
          }))
        },
      },
      "framerLab.setCommandQuery": {
        description: "Update command palette search text.",
        execution: "parallel",
        handle: ({ query }, context) => {
          context.update((model) => ({ ...model, commandQuery: query.slice(0, 80) }))
        },
      },
      "framerLab.selectCoreTab": {
        description: "Switch the reusable segmented-control candidate.",
        execution: "parallel",
        handle: ({ tab }, context) => {
          if (!CORE_TABS.includes(tab)) return
          context.update((model) => ({ ...model, coreTab: tab }))
        },
      },
      "framerLab.setFilterQuery": {
        description: "Update the reusable search and filter toolbar candidate.",
        execution: "parallel",
        handle: ({ query }, context) => {
          context.update((model) => ({ ...model, filterQuery: query.slice(0, 100) }))
        },
      },
      "framerLab.selectDealStatus": {
        description: "Select a status chip for the deal filter candidate.",
        execution: "parallel",
        handle: ({ status }, context) => {
          if (!DEAL_STATUSES.includes(status)) return
          context.update((model) => ({ ...model, dealStatusFilter: status }))
        },
      },
      "framerLab.sortDeals": {
        description: "Sort the dense data-table candidate.",
        execution: "parallel",
        handle: ({ key }, context) => {
          if (!SORT_KEYS.includes(key)) return
          context.update((model) => ({
            ...model,
            tableSortKey: key,
            tableSortDirection:
              model.tableSortKey === key
                ? model.tableSortDirection === "asc"
                  ? "desc"
                  : "asc"
                : key === "value"
                  ? "desc"
                  : "asc",
          }))
        },
      },
      "framerLab.selectDeal": {
        description: "Select one deal row without coupling the table to navigation.",
        execution: "parallel",
        handle: ({ dealId }, context) => {
          if (!FRAMER_LAB_DEALS.some((deal) => deal.id === dealId)) return
          context.update((model) => ({ ...model, selectedDealId: dealId }))
        },
      },
      "framerLab.selectProgressStep": {
        description: "Select a workflow step in the reusable progress rail candidate.",
        execution: "parallel",
        handle: ({ stepId }, context) => {
          if (!FRAMER_LAB_PROGRESS_STEPS.some((step) => step.id === stepId)) return
          context.update((model) => ({ ...model, selectedProgressStepId: stepId }))
        },
      },
      "framerLab.toggleCoreCard": {
        description: "Expand or collapse an operational disclosure card.",
        execution: "parallel",
        handle: ({ cardId }, context) => {
          if (!CORE_CARD_IDS.includes(cardId)) return
          context.update((model) => ({
            ...model,
            expandedCoreCardId: model.expandedCoreCardId === cardId ? null : cardId,
          }))
        },
      },
      "framerLab.selectQuickAction": {
        description: "Select a contextual quick action candidate.",
        execution: "parallel",
        handle: ({ action }, context) => {
          if (!QUICK_ACTIONS.includes(action)) return
          context.update((model) => ({ ...model, selectedQuickAction: action }))
        },
      },
    }
  }
}
