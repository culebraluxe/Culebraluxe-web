'use client'

import { FramerUiLab } from '@/components/portal/tech/framer-ui-lab'
import { TypeScriptUiLab } from '@/components/portal/tech/typescript-ui-lab'

import type { IslandRenderer } from './island-host'

// ---------------------------------------------------------------------------
// Every vendor widget a Yew screen can place with `<Island kind="...">`, by kind. A renderer draws from its props and
// reports events with `emit`; it reads no DOM outside its node and calls no API (the screen does).
// ---------------------------------------------------------------------------

export const ISLAND_RENDERERS: Record<string, IslandRenderer> = {
  // TECH · UI Lab — the preserved galleries, unchanged.
  'ui-lab-gallery': () => <TypeScriptUiLab />,
  'ui-lab-motion': () => <FramerUiLab />,
}
