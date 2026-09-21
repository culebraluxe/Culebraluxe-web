import {
  BasePageController,
  type PageOperationDefinitions,
} from "@/ui/runtime"
import {
  FRAMER_LAB_HOTSPOTS,
  FRAMER_LAB_PROPERTIES,
  INITIAL_FRAMER_UI_LAB_MODEL,
  type FramerUiLabIntentMap,
  type FramerUiLabPageModel,
} from "./model"

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
    }
  }
}
