import type { CouncilPanel, CouncilSubjectType } from './types'

/**
 * CulebraLuxe-owned panels. Upstream persona files may inform method names
 * via protocol.ts; they are not the public seat ids.
 */
export const PANELS: Record<string, CouncilPanel> = {
  'forge-duo': {
    id: 'forge-duo',
    seats: [
      { id: 'frame-break', method: 'elenchic-questioning', role: 'Destroy the unstated frame. 3 levels, then a position. Hemlock on re-ask.' },
      { id: 'rebuild', method: 'first-principles-reconstruction', role: 'Rebuild from observables only. Max 2 analogies. Label FACT vs ASSUMPTION.' },
    ],
  },
  'deal-desk': {
    id: 'deal-desk',
    seats: [
      { id: 'comp', method: 'empirical-comps', role: 'Sold/pending/DOM/seasonality as FACT. No narrative pricing.' },
      { id: 'buyer-fit', method: 'fit-against-stated-need', role: 'This person, this budget, this timeline. CRM prose is ASSUMPTION until a showing confirms it.' },
      { id: 'fragility', method: 'tail-risk', role: 'Insurance, flood, title, ferry, non-resident financing. Design for the tail.' },
    ],
  },
  'match-duo': {
    id: 'match-duo',
    seats: [
      { id: 'buyer-fit', method: 'fit-against-stated-need', role: 'Show / wait / swap / introduce from the person record.' },
      { id: 'listing-facts', method: 'measured-inventory', role: 'Only property + property_media facts. No special-case listings.' },
    ],
  },
  'intake-duo': {
    id: 'intake-duo',
    seats: [
      { id: 'measured', method: 'first-principles-reconstruction', role: 'What is on the property row and media roles.' },
      { id: 'unknowns', method: 'elenchic-questioning', role: 'What is UNKNOWN that would make publish irreversible.' },
    ],
  },
  'paper-duo': {
    id: 'paper-duo',
    seats: [
      { id: 'pirate', method: 'bounded-extraction', role: 'Map one idea to current git. Juice > squeeze.' },
      { id: 'reject', method: 'research-refusal', role: 'Reject if it replaces a harness, adds a lane, or becomes a lab.' },
    ],
  },
}

const SUBJECT_DEFAULT: Record<CouncilSubjectType, string> = {
  deal: 'deal-desk',
  offer: 'deal-desk',
  match: 'match-duo',
  listing_intake: 'intake-duo',
  forge_story: 'forge-duo',
  forge_architect: 'forge-duo',
  forge_lead_pre: 'forge-duo',
  paper_intake: 'paper-duo',
}

export function panelFor(
  subjectType: CouncilSubjectType,
  panelId?: string,
): CouncilPanel {
  const id = panelId ?? SUBJECT_DEFAULT[subjectType]
  const panel = PANELS[id]
  if (!panel) {
    throw new Error(`Unknown council panel: ${id}`)
  }
  return panel
}
