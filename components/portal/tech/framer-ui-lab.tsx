"use client"

import Image from "next/image"
import { useEffect, useRef, useState, type CSSProperties, type PointerEvent } from "react"
import {
  ArrowUpRight,
  Expand,
  Layers3,
  MapPin,
  MousePointer2,
  Pause,
  Play,
  ScanLine,
  Sparkles,
  Type,
} from "lucide-react"

import { usePageController } from "@/ui/runtime"
import {
  FRAMER_LAB_HOTSPOTS,
  FRAMER_LAB_PROPERTIES,
  FramerUiLabController,
  type FramerLabMotionProfile,
} from "@/ui/framer-ui-lab"
import { FramerCoreComponents } from "./framer-core-components"
import styles from "./framer-ui-lab.module.css"

const PROFILE_COPY: Record<FramerLabMotionProfile, { label: string; note: string; scale: string; distance: string }> = {
  restrained: { label: "Restrained", note: "Portal-safe", scale: "1.035", distance: "10px" },
  cinematic: { label: "Cinematic", note: "Luxury default", scale: "1.065", distance: "18px" },
  expressive: { label: "Expressive", note: "Upper bound", scale: "1.095", distance: "28px" },
}

type ViewTransitionDocument = Document & {
  startViewTransition?: (callback: () => void | Promise<void>) => { finished: Promise<void> }
}

function TiltCandidate({
  enabled,
  profile,
}: {
  enabled: boolean
  profile: FramerLabMotionProfile
}) {
  const ref = useRef<HTMLDivElement | null>(null)
  const maximum = profile === "restrained" ? 2.5 : profile === "cinematic" ? 4.5 : 7

  function reset() {
    const node = ref.current
    if (!node) return
    node.style.setProperty("--tilt-x", "0deg")
    node.style.setProperty("--tilt-y", "0deg")
  }

  function move(event: PointerEvent<HTMLDivElement>) {
    if (!enabled) return reset()
    const node = ref.current
    if (!node) return
    const rect = node.getBoundingClientRect()
    const x = (event.clientX - rect.left) / rect.width - 0.5
    const y = (event.clientY - rect.top) / rect.height - 0.5
    node.style.setProperty("--tilt-x", String((-y * maximum).toFixed(2)) + "deg")
    node.style.setProperty("--tilt-y", String((x * maximum).toFixed(2)) + "deg")
  }

  return (
    <div
      ref={ref}
      onPointerMove={move}
      onPointerLeave={reset}
      className={styles.tiltCard}
    >
      <div className="relative h-[390px] overflow-hidden rounded-[22px] border border-white/15 bg-white/[0.05] shadow-2xl">
        <Image
          src="/images/guide/beaches/zoni-beach.jpg"
          alt="Zoni Beach on Culebra"
          fill
          sizes="(min-width: 1024px) 42vw, 92vw"
          className="object-cover"
        />
        <div className="absolute inset-0 bg-gradient-to-t from-[#030f23]/90 via-[#030f23]/15 to-transparent" />
        <div className={"absolute inset-x-0 bottom-0 p-7 " + styles.tiltInner}>
          <p className="text-[10px] uppercase tracking-[0.24em] text-[#c6a15b]">Depth candidate</p>
          <h3 className="mt-2 font-serif text-3xl font-light">A little physics, not a gimmick.</h3>
          <p className="mt-3 max-w-md text-sm font-light leading-6 text-white/65">
            Pointer depth is capped by the active motion profile. This treatment belongs on rare feature cards, never every row.
          </p>
        </div>
      </div>
    </div>
  )
}

export function FramerUiLab() {
  const [controller] = useState(() => new FramerUiLabController())
  const model = usePageController(controller)

  useEffect(() => () => controller.dispose(), [controller])

  const selectedProperty =
    FRAMER_LAB_PROPERTIES.find((property) => property.id === model.selectedPropertyId) ??
    FRAMER_LAB_PROPERTIES[0]
  const selectedHotspot =
    FRAMER_LAB_HOTSPOTS.find((hotspot) => hotspot.id === model.selectedHotspotId) ??
    FRAMER_LAB_HOTSPOTS[0]
  const profile = PROFILE_COPY[model.motionProfile]
  const labStyle = {
    "--hero-scale": profile.scale,
    "--motion-distance": profile.distance,
  } as CSSProperties

  function selectProperty(propertyId: string) {
    const update = () =>
      controller.dispatch({
        operation: "framerLab.selectProperty",
        payload: { propertyId },
      })

    const transition = (document as ViewTransitionDocument).startViewTransition
    if (model.motionEnabled && transition) {
      transition.call(document, update)
      return
    }
    void update()
  }

  return (
    <div
      className={styles.lab + (model.motionEnabled ? "" : " " + styles.motionOff)}
      style={labStyle}
    >
      <div className="mx-auto max-w-[1500px] px-3 pb-24 pt-6 sm:px-6 lg:px-8">
        <header className="mb-8 flex flex-col gap-5 border-b border-white/10 pb-7 xl:flex-row xl:items-end xl:justify-between">
          <div>
            <p className="text-[10px] font-medium uppercase tracking-[0.3em] text-[#c6a15b]">
              TECH / FRAMER UI LAB
            </p>
            <h1 className="mt-2 font-serif text-4xl font-light tracking-[-0.02em] text-white sm:text-5xl">
              Premium interaction candidates
            </h1>
            <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-white/58">
              MVI-controlled experiments for the CulebraLuxe motion language. The media is fixture content; nothing here writes business data.
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            {(Object.keys(PROFILE_COPY) as FramerLabMotionProfile[]).map((key) => {
              const active = model.motionProfile === key
              return (
                <button
                  key={key}
                  type="button"
                  onClick={() =>
                    void controller.dispatch({
                      operation: "framerLab.selectMotionProfile",
                      payload: { profile: key },
                    })
                  }
                  className={
                    "min-h-11 rounded-full border px-4 text-[10px] uppercase tracking-[0.16em] transition " +
                    (active
                      ? "border-[#c6a15b]/70 bg-[#c6a15b]/15 text-[#f8f5ec]"
                      : "border-white/12 bg-white/[0.04] text-white/52 hover:border-white/30 hover:text-white")
                  }
                >
                  {PROFILE_COPY[key].label}
                  <span className="ml-2 opacity-50">{PROFILE_COPY[key].note}</span>
                </button>
              )
            })}
            <button
              type="button"
              onClick={() =>
                void controller.dispatch({
                  operation: "framerLab.toggleMotion",
                  payload: {},
                })
              }
              aria-pressed={model.motionEnabled}
              className="inline-flex min-h-11 items-center gap-2 rounded-full border border-white/15 bg-white/[0.07] px-4 text-[10px] uppercase tracking-[0.16em] text-white transition hover:bg-white/[0.12]"
            >
              {model.motionEnabled ? <Pause className="h-3.5 w-3.5" /> : <Play className="h-3.5 w-3.5" />}
              Motion {model.motionEnabled ? "on" : "off"}
            </button>
          </div>
        </header>

        <section className="mb-16">
          <div className="mb-4 flex items-end justify-between gap-4">
            <div>
              <p className="text-[10px] uppercase tracking-[0.22em] text-[#c6a15b]">01 · CINEMATIC HERO</p>
              <h2 className="mt-1 font-serif text-2xl font-light">Image choreography + masked typography</h2>
            </div>
            <div className="flex rounded-full border border-white/10 bg-white/[0.04] p-1">
              {(["still", "cinematic"] as const).map((mode) => (
                <button
                  key={mode}
                  type="button"
                  onClick={() =>
                    void controller.dispatch({
                      operation: "framerLab.selectHeroMode",
                      payload: { mode },
                    })
                  }
                  className={
                    "min-h-9 rounded-full px-3 text-[10px] uppercase tracking-[0.14em] " +
                    (model.heroMode === mode ? "bg-white text-[#030f23]" : "text-white/50")
                  }
                >
                  {mode}
                </button>
              ))}
            </div>
          </div>

          <div className={styles.hero}>
            <Image
              src="/images/hero-villa.png"
              alt="Culebra residence overlooking the Caribbean"
              fill
              priority
              sizes="(min-width: 1500px) 1450px, 100vw"
              className={
                "object-cover " +
                styles.heroMedia +
                (model.heroMode === "cinematic" && model.motionEnabled ? " " + styles.heroCinematic : "")
              }
            />
            <div className="absolute inset-0 bg-gradient-to-b from-[#030f23]/20 via-transparent to-[#030f23]/88" />
            <div className="absolute inset-x-0 bottom-0 p-7 sm:p-10 lg:p-14">
              <p className={"text-[10px] uppercase tracking-[0.3em] text-[#c6a15b] " + styles.heroCopy}>
                Culebra · Puerto Rico
              </p>
              <div className={"mt-4 max-w-5xl " + styles.heroCopyDelay}>
                <div className={styles.textMask}><span className="font-serif text-5xl font-light leading-[0.95] sm:text-7xl lg:text-8xl">Space.</span></div>
                <div className={styles.textMask}><span className="font-serif text-5xl font-light leading-[0.95] sm:text-7xl lg:text-8xl">Light.</span></div>
                <div className={styles.textMask}><span className="font-serif text-5xl font-light leading-[0.95] sm:text-7xl lg:text-8xl">The sea.</span></div>
              </div>
              <div className="mt-8 flex max-w-3xl flex-col gap-4 border-t border-white/20 pt-5 sm:flex-row sm:items-center sm:justify-between">
                <p className="text-sm font-light leading-6 text-white/68">
                  Candidate for the public landing experience: restrained camera drift, editorial type reveal and no UI chrome.
                </p>
                <span className="inline-flex items-center gap-2 text-[10px] uppercase tracking-[0.18em] text-white/75">
                  Explore <ArrowUpRight className="h-4 w-4" />
                </span>
              </div>
            </div>
          </div>
        </section>

        <section className="mb-16 border-y border-white/10 py-10">
          <div className="grid gap-5 lg:grid-cols-[0.72fr_1.28fr] lg:items-start">
            <div className="lg:sticky lg:top-32">
              <p className="text-[10px] uppercase tracking-[0.22em] text-[#c6a15b]">02 · SCROLL MEDIA</p>
              <h2 className="mt-2 max-w-md font-serif text-4xl font-light leading-[1.02]">
                Let the photograph take over the room.
              </h2>
              <p className="mt-4 max-w-md text-sm font-light leading-6 text-white/55">
                The framed image expands toward full bleed as it crosses the viewport. Browsers without scroll timelines simply keep the elegant framed state.
              </p>
            </div>
            <div className={styles.scrollStage}>
              <div className={styles.scrollFrame}>
                <Image
                  src="/images/culture.png"
                  alt="Turquoise water and beach in Culebra"
                  fill
                  sizes="(min-width: 1024px) 68vw, 100vw"
                  className="object-cover"
                />
                <div className={"absolute inset-x-0 bottom-0 z-10 p-8 sm:p-12 " + styles.scrollCaption}>
                  <p className="text-[10px] uppercase tracking-[0.26em] text-white/60">03 / Island life</p>
                  <h3 className="mt-2 max-w-3xl font-serif text-4xl font-light sm:text-6xl">The island is the amenity.</h3>
                </div>
              </div>
            </div>
          </div>
        </section>

        <section className="mb-16">
          <div className="mb-5">
            <p className="text-[10px] uppercase tracking-[0.22em] text-[#c6a15b]">03 · LAYOUT TRANSITION</p>
            <h2 className="mt-2 font-serif text-3xl font-light">Property selection should feel continuous.</h2>
            <p className="mt-2 max-w-2xl text-sm font-light leading-6 text-white/55">
              Candidate interaction for cards flowing into a property story. Selection lives in the MVI model; supporting browsers use the View Transition API for the scene change.
            </p>
          </div>

          <div className="grid gap-4 lg:grid-cols-[0.72fr_1.28fr]">
            <div className="grid gap-3">
              {FRAMER_LAB_PROPERTIES.map((property) => {
                const active = property.id === selectedProperty.id
                return (
                  <button
                    key={property.id}
                    type="button"
                    onClick={() => selectProperty(property.id)}
                    className={
                      "grid min-h-[116px] grid-cols-[120px_1fr] overflow-hidden rounded-[18px] border border-white/10 bg-white/[0.04] text-left " +
                      styles.propertyCard +
                      (active ? " " + styles.propertyCardActive : "")
                    }
                  >
                    <span className="relative block h-full min-h-[116px] overflow-hidden">
                      <Image src={property.image} alt="" fill sizes="120px" className="object-cover" />
                    </span>
                    <span className="p-4">
                      <span className="text-[9px] uppercase tracking-[0.18em] text-[#c6a15b]">{property.location}</span>
                      <span className="mt-1 block font-serif text-xl font-light text-white">{property.name}</span>
                      <span className="mt-2 block text-xs font-light text-white/45">{property.facts}</span>
                    </span>
                  </button>
                )
              })}
            </div>

            <div
              key={selectedProperty.id}
              className={"relative min-h-[440px] overflow-hidden rounded-[22px] border border-white/12 " + styles.propertyFeature}
              style={{ viewTransitionName: "framer-lab-property-feature" } as CSSProperties}
            >
              <Image
                src={selectedProperty.image}
                alt={selectedProperty.name}
                fill
                sizes="(min-width: 1024px) 62vw, 100vw"
                className="object-cover"
              />
              <div className="absolute inset-0 bg-gradient-to-t from-[#030f23]/95 via-[#030f23]/15 to-transparent" />
              <div className="absolute inset-x-0 bottom-0 p-7 sm:p-9">
                <p className="text-[9px] uppercase tracking-[0.22em] text-[#c6a15b]">{selectedProperty.eyebrow}</p>
                <div className="mt-2 flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
                  <div>
                    <h3 className="font-serif text-4xl font-light sm:text-5xl">{selectedProperty.name}</h3>
                    <p className="mt-3 max-w-xl text-sm font-light leading-6 text-white/64">{selectedProperty.summary}</p>
                  </div>
                  <div className="shrink-0 text-left sm:text-right">
                    <p className="font-serif text-2xl font-light">{selectedProperty.price}</p>
                    <p className="mt-1 text-[10px] uppercase tracking-[0.14em] text-white/45">{selectedProperty.facts}</p>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </section>

        <section className="mb-16 grid gap-5 xl:grid-cols-2">
          <div className="overflow-hidden rounded-[22px] border border-white/10 bg-white/[0.04]">
            <div className="border-b border-white/10 p-6">
              <p className="text-[10px] uppercase tracking-[0.22em] text-[#c6a15b]">04 · HOTSPOTS</p>
              <h2 className="mt-2 font-serif text-3xl font-light">Let the image explain the property.</h2>
            </div>
            <div className="relative h-[430px]">
              <Image
                src="/images/coastline.png"
                alt="Culebra coastal residence"
                fill
                sizes="(min-width: 1280px) 48vw, 100vw"
                className="object-cover"
              />
              <div className="absolute inset-0 bg-gradient-to-t from-[#030f23]/68 via-transparent to-[#030f23]/10" />
              {FRAMER_LAB_HOTSPOTS.map((hotspot) => {
                const active = hotspot.id === selectedHotspot.id
                return (
                  <button
                    key={hotspot.id}
                    type="button"
                    aria-label={hotspot.label}
                    onClick={() =>
                      void controller.dispatch({
                        operation: "framerLab.selectHotspot",
                        payload: { hotspotId: hotspot.id },
                      })
                    }
                    className={styles.hotspot + (active ? " " + styles.hotspotActive : "")}
                    style={{ left: String(hotspot.x) + "%", top: String(hotspot.y) + "%" }}
                  >
                    <span className={styles.hotspotDot}>
                      <span className="h-1.5 w-1.5 rounded-full bg-current" />
                    </span>
                  </button>
                )
              })}
              <div className="absolute bottom-5 left-5 right-5 rounded-[16px] border border-white/14 bg-[#030f23]/58 p-4 backdrop-blur-xl">
                <div className="flex items-start gap-3">
                  <MapPin className="mt-0.5 h-4 w-4 shrink-0 text-[#c6a15b]" />
                  <div>
                    <p className="font-serif text-xl font-light">{selectedHotspot.label}</p>
                    <p className="mt-1 text-xs font-light leading-5 text-white/58">{selectedHotspot.detail}</p>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div className="rounded-[22px] border border-white/10 bg-white/[0.04] p-6">
            <p className="text-[10px] uppercase tracking-[0.22em] text-[#c6a15b]">05 · DEPTH / TILT</p>
            <h2 className="mt-2 font-serif text-3xl font-light">Rare interactive depth.</h2>
            <p className="mb-6 mt-2 text-sm font-light leading-6 text-white/55">
              Move the pointer over the card. The MVI motion profile sets the maximum angle; reduced motion and the lab master switch disable it.
            </p>
            <TiltCandidate enabled={model.motionEnabled} profile={model.motionProfile} />
          </div>
        </section>

        <section className="rounded-[22px] border border-white/10 bg-white/[0.04] p-6 sm:p-8">
          <div className="grid gap-7 xl:grid-cols-[0.9fr_1.1fr]">
            <div>
              <p className="text-[10px] uppercase tracking-[0.22em] text-[#c6a15b]">06 · MOTION LANGUAGE</p>
              <h2 className="mt-2 font-serif text-3xl font-light">The pieces worth promoting.</h2>
              <p className="mt-3 max-w-xl text-sm font-light leading-6 text-white/55">
                These are candidates for a future shared Rust/DOM motion layer. The lab intentionally keeps them independent of the production public-site and portal components.
              </p>
            </div>
            <div className="grid gap-3 sm:grid-cols-2">
              {[
                [Sparkles, "Cinematic media", "Slow image/video choreography for hero moments."],
                [Expand, "Scroll expansion", "Framed editorial media that grows to full bleed."],
                [Layers3, "Layout transitions", "Selections that preserve spatial continuity."],
                [ScanLine, "Hotspots", "Interactive annotations for property photography."],
                [MousePointer2, "Depth", "Very restrained pointer physics on feature surfaces."],
                [Type, "Masked type", "Line/word reveal for major editorial headings only."],
              ].map(([Icon, title, body]) => {
                const Glyph = Icon as typeof Sparkles
                return (
                  <div key={String(title)} className="rounded-[16px] border border-white/10 bg-black/10 p-4">
                    <Glyph className="h-4 w-4 text-[#c6a15b]" />
                    <h3 className="mt-3 font-serif text-lg font-light">{String(title)}</h3>
                    <p className="mt-1 text-xs font-light leading-5 text-white/48">{String(body)}</p>
                  </div>
                )
              })}
            </div>
          </div>
        </section>

        <FramerCoreComponents controller={controller} model={model} />
      </div>
    </div>
  )
}
