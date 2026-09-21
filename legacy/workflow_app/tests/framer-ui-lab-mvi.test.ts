import assert from "node:assert/strict"
import { test } from "node:test"

import {
  FRAMER_LAB_HOTSPOTS,
  FRAMER_LAB_PROPERTIES,
  FramerUiLabController,
} from "@/ui/framer-ui-lab"

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
