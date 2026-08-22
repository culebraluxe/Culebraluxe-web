"use client"

import { useRouter } from "next/navigation"
import { useState, useTransition } from "react"
import { createPropertyAction } from "@/app/portal/actions"

// OPS-03 — "New property": create a canonical property record from the
// Property Administration index. Seeds the operational identity fields from
// the Portal UI contract; the per-listing workspace owns the long tail. On
// success the operator lands on the new workspace to finish the facts.
export function PropertyCreatePanel() {
  const router = useRouter()
  const [isPending, startTransition] = useTransition()
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(
    null,
  )
  const [open, setOpen] = useState(false)

  const [name, setName] = useState("")
  const [slug, setSlug] = useState("")
  const [status, setStatus] = useState("prospect")
  const [featured, setFeatured] = useState(false)
  const [propertyType, setPropertyType] = useState("")
  const [listPrice, setListPrice] = useState("")
  const [location, setLocation] = useState("")
  const [city, setCity] = useState("")
  const [stateOrProvince, setStateOrProvince] = useState("")
  const [neighborhood, setNeighborhood] = useState("")
  const [bedrooms, setBedrooms] = useState("")
  const [bathrooms, setBathrooms] = useState("")
  const [squareFeet, setSquareFeet] = useState("")

  function numberOrNull(value: string) {
    const trimmed = value.trim()
    if (trimmed === "") return null
    return Number(trimmed)
  }

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setMessage(null)
    startTransition(async () => {
      const result = await createPropertyAction({
        name,
        slug: slug.trim() || null,
        status: status as "prospect" | "coming_soon" | "active" | "off_market",
        featured,
        propertyType: propertyType.trim() || null,
        listPrice: numberOrNull(listPrice),
        location: location.trim() || null,
        city: city.trim() || null,
        stateOrProvince: stateOrProvince.trim() || null,
        neighborhood: neighborhood.trim() || null,
        bedrooms: numberOrNull(bedrooms),
        bathrooms: numberOrNull(bathrooms),
        squareFeet: numberOrNull(squareFeet),
      })
      if (!result.ok) {
        setMessage({ ok: false, text: result.message })
        return
      }
      router.push(`/portal/property-admin/${result.data.id}`)
      router.refresh()
    })
  }

  const inputClass =
    "mt-1 block min-h-11 w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
  const labelClass =
    "mb-1 block text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]"

  function Field({
    label,
    children,
    span = false,
  }: {
    label: string
    children: React.ReactNode
    span?: boolean
  }) {
    return (
      <label className={`block ${span ? "md:col-span-2" : ""}`}>
        <span className={labelClass}>{label}</span>
        {children}
      </label>
    )
  }

  return (
    <section className="mb-6 rounded-sm border border-[var(--portal-border)] bg-white">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--portal-border)] px-6 py-5">
        <div>
          <h2 className="font-serif text-2xl font-light">New property</h2>
          <p className="mt-1 text-xs font-light text-black/40">
            Create a canonical listing record, then open its workspace to
            finish the facts and media.
          </p>
        </div>
        <button
          type="button"
          onClick={() => setOpen(!open)}
          className="inline-flex min-h-11 items-center justify-center rounded-sm bg-[var(--portal-navy)] px-5 text-[11px] font-light uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)]"
        >
          {open ? "Close" : "New property"}
        </button>
      </div>

      {open && (
        <form onSubmit={submit} className="space-y-4 p-6">
          <div className="grid gap-4 md:grid-cols-2">
            <Field label="Name *">
              <input
                className={inputClass}
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="e.g. Casa del Sol"
              />
            </Field>
            <Field label="Slug (public URL)">
              <input
                className={inputClass}
                value={slug}
                onChange={(event) => setSlug(event.target.value)}
                placeholder="casa-del-sol"
              />
            </Field>
          </div>

          <div className="grid gap-4 md:grid-cols-3">
            <Field label="Status">
              <select
                className={inputClass}
                value={status}
                onChange={(event) => setStatus(event.target.value)}
              >
                <option value="prospect">Prospect</option>
                <option value="coming_soon">Coming Soon</option>
                <option value="active">Active</option>
                <option value="off_market">Off Market</option>
              </select>
            </Field>
            <Field label="Property type">
              <input
                className={inputClass}
                value={propertyType}
                onChange={(event) => setPropertyType(event.target.value)}
                placeholder="e.g. Villa, Land"
              />
            </Field>
            <Field label="List price (USD)">
              <input
                className={inputClass}
                inputMode="decimal"
                value={listPrice}
                onChange={(event) => setListPrice(event.target.value)}
                placeholder="1250000"
              />
            </Field>
          </div>

          <div className="grid gap-4 md:grid-cols-3">
            <Field label="Location">
              <input
                className={inputClass}
                value={location}
                onChange={(event) => setLocation(event.target.value)}
                placeholder="e.g. Flamenco Beach"
              />
            </Field>
            <Field label="City">
              <input
                className={inputClass}
                value={city}
                onChange={(event) => setCity(event.target.value)}
                placeholder="Culebra"
              />
            </Field>
            <Field label="State / Province">
              <input
                className={inputClass}
                value={stateOrProvince}
                onChange={(event) => setStateOrProvince(event.target.value)}
                placeholder="PR"
              />
            </Field>
          </div>

          <Field label="Neighborhood">
            <input
              className={inputClass}
              value={neighborhood}
              onChange={(event) => setNeighborhood(event.target.value)}
            />
          </Field>

          <div className="grid gap-4 md:grid-cols-3">
            <Field label="Bedrooms">
              <input
                className={inputClass}
                inputMode="decimal"
                value={bedrooms}
                onChange={(event) => setBedrooms(event.target.value)}
              />
            </Field>
            <Field label="Bathrooms">
              <input
                className={inputClass}
                inputMode="decimal"
                value={bathrooms}
                onChange={(event) => setBathrooms(event.target.value)}
              />
            </Field>
            <Field label="Square feet">
              <input
                className={inputClass}
                inputMode="numeric"
                value={squareFeet}
                onChange={(event) => setSquareFeet(event.target.value)}
              />
            </Field>
          </div>

          <label className="flex min-h-11 cursor-pointer items-center gap-3">
            <input
              type="checkbox"
              checked={featured}
              onChange={(event) => setFeatured(event.target.checked)}
              className="h-5 w-5 accent-[var(--portal-navy)]"
            />
            <span className="text-sm font-light text-black/70">
              Featured property (Selected Properties)
            </span>
          </label>

          <div className="flex flex-wrap items-center justify-between gap-3 border-t border-[var(--portal-border)] pt-5">
            {message ? (
              <span
                className={`min-h-5 text-xs font-light ${
                  message.ok ? "text-[#40584b]" : "text-[#8a4b2a]"
                }`}
              >
                {message.text}
              </span>
            ) : (
              <p className="text-xs font-light text-black/40">
                City and state default to Culebra, PR when left blank.
              </p>
            )}
            <button
              type="submit"
              disabled={isPending}
              className="inline-flex min-h-11 items-center justify-center rounded-sm bg-[var(--portal-navy)] px-5 text-[11px] font-light uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40"
            >
              {isPending ? "Creating…" : "Create property"}
            </button>
          </div>
        </form>
      )}
    </section>
  )
}
