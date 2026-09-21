export type FramerLabMotionProfile = "restrained" | "cinematic" | "expressive"
export type FramerLabHeroMode = "still" | "cinematic"

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

export type FramerUiLabPageModel = {
  motionEnabled: boolean
  motionProfile: FramerLabMotionProfile
  heroMode: FramerLabHeroMode
  selectedPropertyId: string
  selectedHotspotId: string
}

export const INITIAL_FRAMER_UI_LAB_MODEL: FramerUiLabPageModel = {
  motionEnabled: true,
  motionProfile: "cinematic",
  heroMode: "cinematic",
  selectedPropertyId: FRAMER_LAB_PROPERTIES[0].id,
  selectedHotspotId: FRAMER_LAB_HOTSPOTS[0].id,
}

export type FramerUiLabIntentMap = {
  "framerLab.toggleMotion": { request: Record<string, never>; response: void }
  "framerLab.selectMotionProfile": { request: { profile: FramerLabMotionProfile }; response: void }
  "framerLab.selectHeroMode": { request: { mode: FramerLabHeroMode }; response: void }
  "framerLab.selectProperty": { request: { propertyId: string }; response: void }
  "framerLab.selectHotspot": { request: { hotspotId: string }; response: void }
}
