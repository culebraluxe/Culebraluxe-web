import assert from "node:assert/strict"
import { test } from "node:test"

import {
  FRAMER_LAB_DEALS,
  FRAMER_LAB_HOTSPOTS,
  FRAMER_LAB_PROGRESS_STEPS,
  FRAMER_LAB_PROPERTIES,
  FramerUiLabController,
} from "../../ui/framer-ui-lab"

test("Framer UI Lab MVI controller owns all discrete interaction state", async () => {
  const controller = new FramerUiLabController()

  assert.equal(controller.snapshot().motionEnabled, true)
  assert.equal(controller.snapshot().motionProfile, "cinematic")

  await controller.dispatch({
    operation: "framerLab.selectMotionProfile",
    payload: { profile: "restrained" },
  })
  await controller.dispatch({
    operation: "framerLab.selectProperty",
    payload: { propertyId: FRAMER_LAB_PROPERTIES[1].id },
  })
  await controller.dispatch({
    operation: "framerLab.selectHotspot",
    payload: { hotspotId: FRAMER_LAB_HOTSPOTS[1].id },
  })
  await controller.dispatch({
    operation: "framerLab.selectHeroMode",
    payload: { mode: "still" },
  })
  await controller.dispatch({
    operation: "framerLab.toggleMotion",
    payload: {},
  })

  const model = controller.snapshot()
  assert.equal(model.motionProfile, "restrained")
  assert.equal(model.selectedPropertyId, FRAMER_LAB_PROPERTIES[1].id)
  assert.equal(model.selectedHotspotId, FRAMER_LAB_HOTSPOTS[1].id)
  assert.equal(model.heroMode, "still")
  assert.equal(model.motionEnabled, false)

  controller.dispose()
})

test("Framer UI Lab rejects unknown fixture ids instead of inventing selection state", async () => {
  const controller = new FramerUiLabController()
  const before = controller.snapshot()

  await controller.dispatch({
    operation: "framerLab.selectProperty",
    payload: { propertyId: "not-a-property" },
  })
  await controller.dispatch({
    operation: "framerLab.selectHotspot",
    payload: { hotspotId: "not-a-hotspot" },
  })

  assert.equal(controller.snapshot().selectedPropertyId, before.selectedPropertyId)
  assert.equal(controller.snapshot().selectedHotspotId, before.selectedHotspotId)
  controller.dispose()
})


test("Framer UI Lab core component candidates keep interaction state in MVI", async () => {
  const controller = new FramerUiLabController()

  await controller.dispatch({
    operation: "framerLab.setCommandPaletteOpen",
    payload: { open: true },
  })
  await controller.dispatch({
    operation: "framerLab.setCommandQuery",
    payload: { query: "client" },
  })
  await controller.dispatch({
    operation: "framerLab.selectCoreTab",
    payload: { tab: "documents" },
  })
  await controller.dispatch({
    operation: "framerLab.setFilterQuery",
    payload: { query: "villa" },
  })
  await controller.dispatch({
    operation: "framerLab.selectDealStatus",
    payload: { status: "active" },
  })
  await controller.dispatch({
    operation: "framerLab.sortDeals",
    payload: { key: "client" },
  })
  await controller.dispatch({
    operation: "framerLab.selectDeal",
    payload: { dealId: FRAMER_LAB_DEALS[2].id },
  })
  await controller.dispatch({
    operation: "framerLab.selectProgressStep",
    payload: { stepId: FRAMER_LAB_PROGRESS_STEPS[3].id },
  })
  await controller.dispatch({
    operation: "framerLab.toggleCoreCard",
    payload: { cardId: "next-action" },
  })
  await controller.dispatch({
    operation: "framerLab.selectQuickAction",
    payload: { action: "email" },
  })

  const model = controller.snapshot()
  assert.equal(model.commandPaletteOpen, true)
  assert.equal(model.commandQuery, "client")
  assert.equal(model.coreTab, "documents")
  assert.equal(model.filterQuery, "villa")
  assert.equal(model.dealStatusFilter, "active")
  assert.equal(model.tableSortKey, "client")
  assert.equal(model.tableSortDirection, "asc")
  assert.equal(model.selectedDealId, FRAMER_LAB_DEALS[2].id)
  assert.equal(model.selectedProgressStepId, FRAMER_LAB_PROGRESS_STEPS[3].id)
  assert.equal(model.expandedCoreCardId, "next-action")
  assert.equal(model.selectedQuickAction, "email")

  await controller.dispatch({
    operation: "framerLab.setCommandPaletteOpen",
    payload: { open: false },
  })
  assert.equal(controller.snapshot().commandQuery, "")

  controller.dispose()
})

test("Framer UI Lab core candidates reject unknown fixture selections", async () => {
  const controller = new FramerUiLabController()
  const before = controller.snapshot()

  await controller.dispatch({
    operation: "framerLab.selectDeal",
    payload: { dealId: "missing-deal" },
  })
  await controller.dispatch({
    operation: "framerLab.selectProgressStep",
    payload: { stepId: "missing-step" },
  })

  assert.equal(controller.snapshot().selectedDealId, before.selectedDealId)
  assert.equal(
    controller.snapshot().selectedProgressStepId,
    before.selectedProgressStepId,
  )

  controller.dispose()
})
