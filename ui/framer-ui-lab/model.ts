export type FramerLabMotionProfile = "restrained" | "cinematic" | "expressive"
export type FramerLabHeroMode = "still" | "cinematic"
export type FramerLabCoreTab = "overview" | "activity" | "documents"
export type FramerLabFilterStatus = "all" | "active" | "attention" | "closed"
export type FramerLabSortKey = "client" | "property" | "stage" | "value"
export type FramerLabSortDirection = "asc" | "desc"
export type FramerLabCoreCardId = "client-brief" | "deal-readiness" | "next-action"
export type FramerLabQuickAction = "call" | "email" | "task" | "document"

export type FramerLabProperty = {
  id: string
  name: string
  location: string
  price: string
  facts: string
  image: string
  eyebrow: string
  summary: string
}

export type FramerLabHotspot = {
  id: string
  label: string
  detail: string
  x: number
  y: number
}

export type FramerLabCommand = {
  id: string
  group: "Navigate" | "Create" | "Find"
  label: string
  detail: string
  shortcut?: string
}

export type FramerLabDeal = {
  id: string
  client: string
  property: string
  stage: string
  status: Exclude<FramerLabFilterStatus, "all">
  value: number
  valueLabel: string
  lastTouch: string
}

export type FramerLabProgressStep = {
  id: string
  label: string
  detail: string
  state: "complete" | "current" | "upcoming"
}

export const FRAMER_LAB_PROPERTIES: readonly FramerLabProperty[] = [
  {
    id: "villa-rosada",
    name: "Villa Rosada",
    location: "Ensenada Honda",
    price: "$1,240,000",
    facts: "3 beds · 2.5 baths · sea view",
    image: "/images/property-01.png",
    eyebrow: "Private waterfront",
    summary: "A quiet waterfront residence framed around long Caribbean views and outdoor living.",
  },
  {
    id: "casa-palmera",
    name: "Casa Palmera",
    location: "Flamenco",
    price: "$875,000",
    facts: "4 beds · 3 baths · garden",
    image: "/images/property-02.png",
    eyebrow: "Island retreat",
    summary: "A warm, planted residence with generous gathering spaces and a softer resort character.",
  },
  {
    id: "blue-marlin",
    name: "Blue Marlin House",
    location: "Punta Aloe",
    price: "$1,650,000",
    facts: "4 beds · 4 baths · horizon view",
    image: "/images/property-03.png",
    eyebrow: "Elevated outlook",
    summary: "An elevated home shaped around breeze, horizon light and a dramatic arrival sequence.",
  },
]

export const FRAMER_LAB_HOTSPOTS: readonly FramerLabHotspot[] = [
  { id: "terrace", label: "Sunset terrace", detail: "West-facing terrace with uninterrupted evening light.", x: 69, y: 43 },
  { id: "pool", label: "Pool deck", detail: "Protected outdoor room with direct living-area access.", x: 52, y: 68 },
  { id: "suite", label: "Primary suite", detail: "Private wing oriented toward the water.", x: 34, y: 35 },
]

export const FRAMER_LAB_COMMANDS: readonly FramerLabCommand[] = [
  { id: "cockpit", group: "Navigate", label: "Open Cockpit", detail: "Daily command surface", shortcut: "G C" },
  { id: "clients", group: "Navigate", label: "Open Clients", detail: "People and relationship intelligence", shortcut: "G L" },
  { id: "projects", group: "Navigate", label: "Open Projects", detail: "Property work and active projects", shortcut: "G P" },
  { id: "new-client", group: "Create", label: "New client", detail: "Start a client intake", shortcut: "N C" },
  { id: "new-task", group: "Create", label: "New task", detail: "Capture work without leaving context", shortcut: "N T" },
  { id: "new-document", group: "Create", label: "New document", detail: "Open the document workflow", shortcut: "N D" },
  { id: "find-property", group: "Find", label: "Find property", detail: "Search property records and media" },
  { id: "find-contract", group: "Find", label: "Find contract", detail: "Search active and completed contracts" },
]

export const FRAMER_LAB_DEALS: readonly FramerLabDeal[] = [
  { id: "deal-01", client: "Marisol Vega", property: "Villa Rosada", stage: "Listing", status: "active", value: 1240000, valueLabel: "$1.24M", lastTouch: "18 min" },
  { id: "deal-02", client: "Daniel Rivera", property: "Casa Palmera", stage: "Photography", status: "attention", value: 875000, valueLabel: "$875K", lastTouch: "2 hr" },
  { id: "deal-03", client: "Elena Santiago", property: "Blue Marlin House", stage: "Offer", status: "active", value: 1650000, valueLabel: "$1.65M", lastTouch: "Today" },
  { id: "deal-04", client: "Rafael Ortiz", property: "Bahía Vista", stage: "Closing", status: "attention", value: 980000, valueLabel: "$980K", lastTouch: "Yesterday" },
  { id: "deal-05", client: "Sofia Morales", property: "Brisa del Mar", stage: "Closed", status: "closed", value: 720000, valueLabel: "$720K", lastTouch: "Sep 14" },
  { id: "deal-06", client: "Isabel Cruz", property: "Punta Aloe Retreat", stage: "Marketing", status: "active", value: 1425000, valueLabel: "$1.43M", lastTouch: "Sep 13" },
]

export const FRAMER_LAB_PROGRESS_STEPS: readonly FramerLabProgressStep[] = [
  { id: "intake", label: "Intake", detail: "Parties, property and representation confirmed.", state: "complete" },
  { id: "listing", label: "Listing", detail: "Agreement signed and listing file opened.", state: "complete" },
  { id: "media", label: "Media", detail: "Photography, cabinet assets and property story.", state: "current" },
  { id: "market", label: "Market", detail: "MLS, launch channels and campaign execution.", state: "upcoming" },
  { id: "close", label: "Close", detail: "Offer, signatures, accounting and archive.", state: "upcoming" },
]

export type FramerUiLabPageModel = {
  motionEnabled: boolean
  motionProfile: FramerLabMotionProfile
  heroMode: FramerLabHeroMode
  selectedPropertyId: string
  selectedHotspotId: string
  commandPaletteOpen: boolean
  commandQuery: string
  coreTab: FramerLabCoreTab
  filterQuery: string
  dealStatusFilter: FramerLabFilterStatus
  tableSortKey: FramerLabSortKey
  tableSortDirection: FramerLabSortDirection
  selectedDealId: string
  selectedProgressStepId: string
  expandedCoreCardId: FramerLabCoreCardId | null
  selectedQuickAction: FramerLabQuickAction
}

export const INITIAL_FRAMER_UI_LAB_MODEL: FramerUiLabPageModel = {
  motionEnabled: true,
  motionProfile: "cinematic",
  heroMode: "cinematic",
  selectedPropertyId: FRAMER_LAB_PROPERTIES[0].id,
  selectedHotspotId: FRAMER_LAB_HOTSPOTS[0].id,
  commandPaletteOpen: false,
  commandQuery: "",
  coreTab: "overview",
  filterQuery: "",
  dealStatusFilter: "all",
  tableSortKey: "value",
  tableSortDirection: "desc",
  selectedDealId: FRAMER_LAB_DEALS[0].id,
  selectedProgressStepId: FRAMER_LAB_PROGRESS_STEPS[2].id,
  expandedCoreCardId: "deal-readiness",
  selectedQuickAction: "task",
}

export type FramerUiLabIntentMap = {
  "framerLab.toggleMotion": { request: Record<string, never>; response: void }
  "framerLab.selectMotionProfile": { request: { profile: FramerLabMotionProfile }; response: void }
  "framerLab.selectHeroMode": { request: { mode: FramerLabHeroMode }; response: void }
  "framerLab.selectProperty": { request: { propertyId: string }; response: void }
  "framerLab.selectHotspot": { request: { hotspotId: string }; response: void }
  "framerLab.setCommandPaletteOpen": { request: { open: boolean }; response: void }
  "framerLab.setCommandQuery": { request: { query: string }; response: void }
  "framerLab.selectCoreTab": { request: { tab: FramerLabCoreTab }; response: void }
  "framerLab.setFilterQuery": { request: { query: string }; response: void }
  "framerLab.selectDealStatus": { request: { status: FramerLabFilterStatus }; response: void }
  "framerLab.sortDeals": { request: { key: FramerLabSortKey }; response: void }
  "framerLab.selectDeal": { request: { dealId: string }; response: void }
  "framerLab.selectProgressStep": { request: { stepId: string }; response: void }
  "framerLab.toggleCoreCard": { request: { cardId: FramerLabCoreCardId }; response: void }
  "framerLab.selectQuickAction": { request: { action: FramerLabQuickAction }; response: void }
}
