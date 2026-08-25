'use client'

import Link from 'next/link'
import { useRouter } from 'next/navigation'
import { useState, useTransition } from 'react'

import {
  cancelTaskAction,
  completeTaskAction,
  createTaskAction,
  setPropertyHeroAction,
  setPropertyMediaOrderAction,
  unlinkPropertyMediaAction,
  updateMediaMetadataAction,
  updatePropertyFactsAction,
  updatePropertyVisibilityAction,
  updateTaskDueAction,
} from '@/app/portal/actions'
import { PropertyArchiveButton } from '@/components/portal/write/property-archive-button'
import type {
  PropertyFactsView,
  PropertyWorkspace,
  WorkspaceDeal,
  WorkspaceEnquiry,
  WorkspaceInterest,
  WorkspaceMedia,
  WorkspaceSeller,
  WorkspaceShowing,
} from '@/db/portal-property'

// Listing / Seller Operations V1 workspace. One cohesive per-property surface:
// facts editing, visibility, media order/hero/metadata, seller context, listing
// tasks, activity timeline, interest/enquiries, and showings/deals cross-links.

type Result = { ok: boolean; message?: string }

function resolve(result: unknown): Result {
  return result as Result
}

const primaryButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm bg-[var(--portal-navy)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40'

const secondaryButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm border border-[var(--portal-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-40'

const ghostButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm px-3 text-[11px] font-light uppercase tracking-[0.14em] text-black/45 transition hover:text-[var(--portal-archive)] disabled:cursor-not-allowed disabled:opacity-40'

const dangerButton =
  'inline-flex min-h-11 items-center justify-center rounded-sm border border-[var(--portal-danger)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-archive)] transition hover:bg-[var(--portal-archive)] hover:text-white disabled:cursor-not-allowed disabled:opacity-40'

const fieldInput =
  'mt-1 block min-h-11 w-full rounded-[var(--portal-panel-radius)] portal-glass-panel px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]'

const fieldTextarea =
  'mt-1 block min-h-11 w-full rounded-[var(--portal-panel-radius)] portal-glass-panel px-3 py-2 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]'

const labelClass =
  'mb-1 block text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]'

// Mirrors db/portal-property constants (kept local so the client bundle never
// pulls in the Neon client).
const EDITABLE_STATUSES = ['prospect', 'coming_soon', 'active', 'off_market']
const CRM14_STATUSES = ['under_contract', 'sold']

const STATUS_LABELS: Record<string, string> = {
  prospect: 'Prospect',
  coming_soon: 'Coming Soon',
  active: 'Active',
  off_market: 'Off Market',
  under_contract: 'Under Contract',
  sold: 'Sold',
  archived: 'Archived',
}

function statusLabel(status: string) {
  return STATUS_LABELS[status] ?? status
}

const STAGE_LABELS: Record<string, string> = {
  new_lead: 'New Lead',
  qualified: 'Qualified',
  showing: 'Showing',
  offer: 'Offer',
  under_contract: 'Under Contract',
  closed: 'Closed',
}

const SHOWING_STATUS_LABELS: Record<string, string> = {
  requested: 'Requested',
  scheduled: 'Scheduled',
  completed: 'Completed',
  cancelled: 'Cancelled',
}

const INTEREST_STATUS_LABELS: Record<string, string> = {
  interested: 'Interested',
  shortlisted: 'Shortlisted',
  tour_completed: 'Tour Completed',
}

const ENQUIRY_STATUS_LABELS: Record<string, string> = {
  received: 'Received',
  processing: 'Processing',
  resolution_required: 'Needs Decision',
  completed: 'Completed',
  rejected: 'Rejected',
}

function friendlyLabel(labels: Record<string, string>, value: string) {
  return labels[value] ?? value
}

function field({ label, children, className }: { label: string; children: React.ReactNode; className?: string }) {
  return (
    <label className={`block ${className ?? ''}`}>
      <span className={labelClass}>{label}</span>
      {children}
    </label>
  )
}

function Section({
  title,
  subtitle,
  children,
  right,
}: {
  title: string
  subtitle?: string
  children: React.ReactNode
  right?: React.ReactNode
}) {
  return (
    <section className="overflow-hidden rounded-[var(--portal-panel-radius)] portal-glass-panel">
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-[var(--portal-border)] px-6 py-5">
        <div>
          <h2 className="font-serif text-2xl font-light">{title}</h2>
          {subtitle && (
            <p className="mt-1 max-w-2xl text-xs font-light text-black/40">
              {subtitle}
            </p>
          )}
        </div>
        {right}
      </div>
      <div className="p-6">{children}</div>
    </section>
  )
}

function ActionMessage({ message }: { message: { ok: boolean; text: string } | null }) {
  if (!message) return null
  return (
    <span
      className={`min-h-5 text-xs font-light ${
        message.ok ? 'text-[var(--portal-success)]' : 'text-[var(--portal-archive)]'
      }`}
    >
      {message.text}
    </span>
  )
}

export function PropertyAdminWorkspace({
  workspace,
}: {
  workspace: PropertyWorkspace
}) {
  const router = useRouter()
  const property = workspace.property as PropertyFactsView
  const refresh = () => router.refresh()

  return (
    <div className="space-y-6">
      <div className="mb-8">
        <Link
          href="/portal/property-admin"
          className="text-xs font-light uppercase tracking-[0.18em] text-[var(--portal-navy-soft)] underline-offset-4 hover:underline"
        >
          ← Property Administration
        </Link>

        <p className="mt-6 text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Operations / Listing
        </p>

        <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
          {property.name}
        </h1>

        <div className="mt-4 flex flex-wrap items-center gap-3">
          <span className="inline-flex min-h-8 items-center rounded-sm bg-[var(--portal-blue-pale)] px-3 text-xs font-light uppercase tracking-[0.14em] text-[var(--portal-navy)]">
            {statusLabel(property.status)}
            {property.featured ? ' · Featured' : ''}
          </span>
          {property.slug && (
            <Link
              href={`/properties/${property.slug}`}
              className="inline-flex min-h-8 items-center text-xs font-light text-[var(--portal-navy-soft)] underline-offset-2 hover:underline"
            >
              View public listing
            </Link>
          )}
          <PropertyArchiveButton
            propertyId={property.id}
            name={property.name}
            archived={property.archived}
          />
        </div>

        <p className="mt-3 text-xs font-light text-black/45">
          {property.archived ? 'Archived' : 'Active in brokerage'} · Created{' '}
          {property.createdAtLabel ?? '—'} · Updated{' '}
          {property.updatedAtLabel ?? '—'}
        </p>
      </div>

      <div className="grid gap-6 xl:grid-cols-2">
        <FactsForm property={property} onSaved={refresh} />
        <VisibilityControls property={property} onSaved={refresh} />
      </div>

      <MediaManager propertyId={property.id} media={workspace.media} onSaved={refresh} />

      <div className="grid gap-6 xl:grid-cols-2">
        <SellerPanel seller={workspace.seller} />
        <TasksPanel propertyId={property.id} openTasks={workspace.openTasks} onSaved={refresh} />
      </div>

      <ActivityPanel activity={workspace.activity} />

      <div className="grid gap-6 xl:grid-cols-2">
        <InterestPanel interests={workspace.interests} enquiries={workspace.enquiries} />
        <ShowingsDealsPanel showings={workspace.showings} deals={workspace.deals} />
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Story 1 — Listing facts editing
// ---------------------------------------------------------------------------

function toNullableNumber(value: string): number | null {
  const trimmed = value.trim()
  if (trimmed === '') return null
  return Number(trimmed)
}

function FactsForm({
  property,
  onSaved,
}: {
  property: PropertyFactsView
  onSaved: () => void
}) {
  const [isPending, startTransition] = useTransition()
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null)

  const [name, setName] = useState(property.name)
  const [slug, setSlug] = useState(property.slug ?? '')
  const [propertyType, setPropertyType] = useState(property.propertyType ?? '')
  const [listPrice, setListPrice] = useState(property.listPrice?.toString() ?? '')
  const [location, setLocation] = useState(property.location ?? '')
  const [city, setCity] = useState(property.city ?? '')
  const [stateOrProvince, setStateOrProvince] = useState(property.stateOrProvince ?? '')
  const [neighborhood, setNeighborhood] = useState(property.neighborhood ?? '')
  const [latitude, setLatitude] = useState(property.latitude?.toString() ?? '')
  const [longitude, setLongitude] = useState(property.longitude?.toString() ?? '')
  const [bedrooms, setBedrooms] = useState(property.bedrooms?.toString() ?? '')
  const [bathrooms, setBathrooms] = useState(property.bathrooms?.toString() ?? '')
  const [squareFeet, setSquareFeet] = useState(property.squareFeet?.toString() ?? '')
  const [lotSize, setLotSize] = useState(property.lotSize?.toString() ?? '')
  const [lotSizeUnits, setLotSizeUnits] = useState(property.lotSizeUnits ?? '')
  const [yearBuilt, setYearBuilt] = useState(property.yearBuilt?.toString() ?? '')
  const [stories, setStories] = useState(property.stories?.toString() ?? '')
  const [parkingSpaces, setParkingSpaces] = useState(property.parkingSpaces?.toString() ?? '')
  const [shortDescription, setShortDescription] = useState(property.shortDescription ?? '')
  const [editorialDescription, setEditorialDescription] = useState(property.editorialDescription ?? '')

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await updatePropertyFactsAction(property.id, {
          name,
          slug: slug.trim() || null,
          propertyType: propertyType.trim() || null,
          listPrice: toNullableNumber(listPrice),
          location: location.trim() || null,
          city: city.trim() || null,
          stateOrProvince: stateOrProvince.trim() || null,
          neighborhood: neighborhood.trim() || null,
          latitude: toNullableNumber(latitude),
          longitude: toNullableNumber(longitude),
          bedrooms: toNullableNumber(bedrooms),
          bathrooms: toNullableNumber(bathrooms),
          squareFeet: toNullableNumber(squareFeet),
          lotSize: toNullableNumber(lotSize),
          lotSizeUnits: lotSizeUnits.trim() || null,
          yearBuilt: toNullableNumber(yearBuilt),
          stories: toNullableNumber(stories),
          parkingSpaces: toNullableNumber(parkingSpaces),
          shortDescription: shortDescription.trim() || null,
          editorialDescription: editorialDescription.trim() || null,
        }),
      )
      if (result.ok) {
        setMessage({ ok: true, text: 'Listing facts saved.' })
        onSaved()
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not save listing facts.' })
      }
    })
  }

  return (
    <Section
      title="Listing facts"
      subtitle="Canonical property fields. Numeric values can be left blank to clear."
      right={<ActionMessage message={message} />}
    >
      <form onSubmit={submit} className="space-y-4">
        <div className="grid gap-4 md:grid-cols-2">
          {field({ label: 'Name', children: <input className={fieldInput} value={name} onChange={(e) => setName(e.target.value)} /> })}
          {field({ label: 'Slug (public URL)', children: <input className={fieldInput} value={slug} onChange={(e) => setSlug(e.target.value)} /> })}
          {field({ label: 'Property type', children: <input className={fieldInput} value={propertyType} onChange={(e) => setPropertyType(e.target.value)} /> })}
          {field({ label: 'List price (USD)', children: <input className={fieldInput} inputMode="decimal" value={listPrice} onChange={(e) => setListPrice(e.target.value)} /> })}
        </div>

        <div className="grid gap-4 md:grid-cols-3">
          {field({ label: 'Location', children: <input className={fieldInput} value={location} onChange={(e) => setLocation(e.target.value)} /> })}
          {field({ label: 'City', children: <input className={fieldInput} value={city} onChange={(e) => setCity(e.target.value)} /> })}
          {field({ label: 'Neighborhood', children: <input className={fieldInput} value={neighborhood} onChange={(e) => setNeighborhood(e.target.value)} /> })}
        </div>

        <div className="grid gap-4 md:grid-cols-3">
          {field({ label: 'State / Province', children: <input className={fieldInput} value={stateOrProvince} onChange={(e) => setStateOrProvince(e.target.value)} /> })}
          {field({ label: 'Latitude', children: <input className={fieldInput} inputMode="decimal" value={latitude} onChange={(e) => setLatitude(e.target.value)} /> })}
          {field({ label: 'Longitude', children: <input className={fieldInput} inputMode="decimal" value={longitude} onChange={(e) => setLongitude(e.target.value)} /> })}
        </div>

        <div className="grid gap-4 md:grid-cols-4">
          {field({ label: 'Bedrooms', children: <input className={fieldInput} inputMode="decimal" value={bedrooms} onChange={(e) => setBedrooms(e.target.value)} /> })}
          {field({ label: 'Bathrooms', children: <input className={fieldInput} inputMode="decimal" value={bathrooms} onChange={(e) => setBathrooms(e.target.value)} /> })}
          {field({ label: 'Square feet', children: <input className={fieldInput} inputMode="numeric" value={squareFeet} onChange={(e) => setSquareFeet(e.target.value)} /> })}
          {field({ label: 'Year built', children: <input className={fieldInput} inputMode="numeric" value={yearBuilt} onChange={(e) => setYearBuilt(e.target.value)} /> })}
        </div>

        <div className="grid gap-4 md:grid-cols-4">
          {field({ label: 'Lot size', children: <input className={fieldInput} inputMode="decimal" value={lotSize} onChange={(e) => setLotSize(e.target.value)} /> })}
          {field({ label: 'Lot size units', children: <input className={fieldInput} value={lotSizeUnits} onChange={(e) => setLotSizeUnits(e.target.value)} /> })}
          {field({ label: 'Stories', children: <input className={fieldInput} inputMode="decimal" value={stories} onChange={(e) => setStories(e.target.value)} /> })}
          {field({ label: 'Parking spaces', children: <input className={fieldInput} inputMode="numeric" value={parkingSpaces} onChange={(e) => setParkingSpaces(e.target.value)} /> })}
        </div>

        {field({
          label: 'Short description',
          children: <textarea className={fieldTextarea} rows={2} value={shortDescription} onChange={(e) => setShortDescription(e.target.value)} />,
        })}
        {field({
          label: 'Editorial description',
          children: <textarea className={fieldTextarea} rows={4} value={editorialDescription} onChange={(e) => setEditorialDescription(e.target.value)} />,
        })}

        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-[var(--portal-border)] pt-5">
          <p className="text-xs font-light text-black/40">
            Saving updates the canonical listing that powers the public site.
          </p>
          <button type="submit" disabled={isPending} className={primaryButton}>
            {isPending ? 'Saving…' : 'Save listing facts'}
          </button>
        </div>
      </form>
    </Section>
  )
}

// ---------------------------------------------------------------------------
// Story 2 — Visibility / status (bounded, CRM-14 statuses read-only)
// ---------------------------------------------------------------------------

function VisibilityControls({
  property,
  onSaved,
}: {
  property: PropertyFactsView
  onSaved: () => void
}) {
  const [isPending, startTransition] = useTransition()
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null)
  const [featured, setFeatured] = useState(property.featured)
  const [status, setStatus] = useState(property.status)

  const isCrm14 = CRM14_STATUSES.includes(property.status)
  const canEditStatus = EDITABLE_STATUSES.includes(property.status)

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await updatePropertyVisibilityAction(property.id, { featured, status }),
      )
      if (result.ok) {
        setMessage({ ok: true, text: 'Visibility saved.' })
        onSaved()
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not save visibility.' })
      }
    })
  }

  return (
    <Section
      title="Visibility & status"
      subtitle="Editorial visibility controls. Transaction states are managed separately."
      right={<ActionMessage message={message} />}
    >
      <form onSubmit={submit} className="space-y-5">
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

        <div>
          <span className={labelClass}>Status</span>
          {canEditStatus ? (
            <select
              value={status}
              onChange={(event) => setStatus(event.target.value)}
              className={fieldInput}
            >
              {EDITABLE_STATUSES.map((value) => (
                <option key={value} value={value}>
                  {statusLabel(value)}
                </option>
              ))}
            </select>
          ) : (
            <div className="mt-1 flex min-h-11 items-center gap-3 rounded-sm border border-[var(--portal-border)] bg-[var(--portal-blue-pale)]/40 px-3">
              <span className="text-sm font-light text-black/60">
                {statusLabel(property.status)}
              </span>
              <span className="text-xs font-light text-[var(--portal-archive)]">
                {isCrm14
                  ? 'Transaction status — managed by CRM-14 workflow.'
                  : 'Read-only in listing administration.'}
              </span>
            </div>
          )}
        </div>

        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-[var(--portal-border)] pt-5">
          <p className="text-xs font-light text-black/40">
            Featured and status feed the buyers inventory and homepage directly.
          </p>
          <button type="submit" disabled={isPending} className={primaryButton}>
            {isPending ? 'Saving…' : 'Save visibility'}
          </button>
        </div>
      </form>
    </Section>
  )
}

// ---------------------------------------------------------------------------
// Stories 3 & 4 — Media order / hero / unlink / metadata
// ---------------------------------------------------------------------------

function MediaThumb({ media }: { media: WorkspaceMedia }) {
  if (media.mediaType === 'image') {
    return (
      <img
        src={`/api/media/${media.id}`}
        alt={media.altText ?? media.filename ?? 'Property media'}
        className="h-14 w-20 shrink-0 rounded-sm border border-[var(--portal-border)] object-cover"
      />
    )
  }
  return (
    <div className="flex h-14 w-20 shrink-0 items-center justify-center rounded-sm border border-[var(--portal-border)] bg-[var(--portal-blue-pale)]/50 text-[10px] font-light uppercase tracking-[0.14em] text-black/45">
      {media.mediaType}
    </div>
  )
}

function MediaManager({
  propertyId,
  media,
  onSaved,
}: {
  propertyId: string
  media: WorkspaceMedia[]
  onSaved: () => void
}) {
  const [ordered, setOrdered] = useState(media)
  const [isPending, startTransition] = useTransition()
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null)
  const [confirmUnlink, setConfirmUnlink] = useState<string | null>(null)

  const orderDirty =
    ordered.length !== media.length ||
    ordered.some((item, index) => item.id !== media[index]?.id)

  function move(index: number, delta: number) {
    setOrdered((prev) => {
      const next = [...prev]
      const target = index + delta
      if (target < 0 || target >= next.length) return prev
      const [item] = next.splice(index, 1)
      next.splice(target, 0, item)
      return next
    })
  }

  function saveOrder() {
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await setPropertyMediaOrderAction(propertyId, ordered.map((item) => item.id)),
      )
      if (result.ok) {
        setMessage({ ok: true, text: 'Media order saved.' })
        onSaved()
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not save media order.' })
      }
    })
  }

  function makeHero(mediaId: string) {
    setMessage(null)
    startTransition(async () => {
      const result = resolve(await setPropertyHeroAction(propertyId, mediaId))
      if (result.ok) {
        setMessage({ ok: true, text: 'Hero updated.' })
        onSaved()
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not update hero.' })
      }
    })
  }

  function unlink(mediaId: string) {
    setMessage(null)
    startTransition(async () => {
      const result = resolve(await unlinkPropertyMediaAction(propertyId, mediaId))
      setConfirmUnlink(null)
      if (result.ok) {
        setMessage({ ok: true, text: 'Media removed from this listing.' })
        onSaved()
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not remove media.' })
      }
    })
  }

  return (
    <Section
      title="Media order & hero"
      subtitle="Control how existing media appears publicly. Removing a media link keeps the underlying file."
      right={
        <div className="flex items-center gap-3">
          <ActionMessage message={message} />
          {orderDirty && (
            <button type="button" disabled={isPending} onClick={saveOrder} className={primaryButton}>
              {isPending ? 'Saving…' : 'Save order'}
            </button>
          )}
        </div>
      }
    >
      {ordered.length === 0 ? (
        <p className="text-sm font-light text-black/40">
          No media linked to this property yet. Add media from the Media Audit /
          upload surface.
        </p>
      ) : (
        <div className="space-y-3">
          {ordered.map((item, index) => (
            <div
              key={item.id}
              className="flex flex-col gap-3 rounded-sm border border-[var(--portal-border)] p-4 lg:flex-row lg:items-center"
            >
              <MediaThumb media={item} />

              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-serif text-base font-light">{item.filename ?? 'Untitled'}</span>
                  <span className="rounded-sm bg-[var(--portal-blue-pale)] px-2 py-0.5 text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy)]">
                    {item.role}
                  </span>
                  {item.mediaType === 'video' && item.muxPlaybackId ? (
                    <span className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">
                      Video
                    </span>
                  ) : null}
                </div>
                <p className="mt-1 text-xs font-light text-black/45">
                  Position {index + 1} · {item.mediaType}
                </p>
              </div>

              <div className="flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  disabled={isPending || index === 0}
                  onClick={() => move(index, -1)}
                  className={secondaryButton}
                  aria-label="Move earlier"
                >
                  ↑
                </button>
                <button
                  type="button"
                  disabled={isPending || index === ordered.length - 1}
                  onClick={() => move(index, 1)}
                  className={secondaryButton}
                  aria-label="Move later"
                >
                  ↓
                </button>
                {item.mediaType === 'image' && item.role !== 'hero' && (
                  <button
                    type="button"
                    disabled={isPending}
                    onClick={() => makeHero(item.id)}
                    className={secondaryButton}
                  >
                    Make hero
                  </button>
                )}
                {confirmUnlink === item.id ? (
                  <button
                    type="button"
                    disabled={isPending}
                    onClick={() => unlink(item.id)}
                    className={dangerButton}
                  >
                    Confirm remove
                  </button>
                ) : (
                  <button
                    type="button"
                    disabled={isPending}
                    onClick={() => setConfirmUnlink(item.id)}
                    className={ghostButton}
                  >
                    Remove
                  </button>
                )}
              </div>

              <MediaMetadataEditor mediaId={item.id} altText={item.altText} caption={item.caption} onSaved={onSaved} />
            </div>
          ))}
        </div>
      )}
    </Section>
  )
}

function MediaMetadataEditor({
  mediaId,
  altText,
  caption,
  onSaved,
}: {
  mediaId: string
  altText: string | null
  caption: string | null
  onSaved: () => void
}) {
  const [isPending, startTransition] = useTransition()
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null)
  const [alt, setAlt] = useState(altText ?? '')
  const [cap, setCap] = useState(caption ?? '')

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await updateMediaMetadataAction(mediaId, {
          altText: alt.trim() || null,
          caption: cap.trim() || null,
        }),
      )
      if (result.ok) {
        setMessage({ ok: true, text: 'Metadata saved.' })
        onSaved()
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not save metadata.' })
      }
    })
  }

  return (
    <form onSubmit={submit} className="w-full lg:w-72 lg:shrink-0">
      <div className="grid gap-2 lg:grid-cols-1">
        {field({
          label: 'Alt text',
          children: <input className={fieldInput} value={alt} onChange={(event) => setAlt(event.target.value)} />,
        })}
        {field({
          label: 'Caption',
          children: <input className={fieldInput} value={cap} onChange={(event) => setCap(event.target.value)} />,
        })}
      </div>
      <div className="mt-2 flex items-center justify-between gap-2">
        <ActionMessage message={message} />
        <button type="submit" disabled={isPending} className={secondaryButton}>
          {isPending ? 'Saving…' : 'Save'}
        </button>
      </div>
    </form>
  )
}

// ---------------------------------------------------------------------------
// Story 6 — Seller context (read-only)
// ---------------------------------------------------------------------------

function SellerPanel({ seller }: { seller: WorkspaceSeller | null }) {
  return (
    <Section
      title="Seller context"
      subtitle="Who owns or sells this listing, and what is our relationship?"
      right={
        seller ? (
          <Link href={`/portal/clients/${seller.id}`} className={secondaryButton}>
            Open relationship dossier
          </Link>
        ) : undefined
      }
    >
      {seller ? (
        <dl className="grid gap-x-6 gap-y-4 md:grid-cols-2">
          <div>
            <dt className={labelClass}>Name</dt>
            <dd className="font-serif text-lg font-light">{seller.name}</dd>
          </div>
          <div>
            <dt className={labelClass}>Relationship</dt>
            <dd className="text-sm font-light capitalize text-black/70">
              {seller.role} · {seller.status}
            </dd>
          </div>
          {seller.email && (
            <div>
              <dt className={labelClass}>Email</dt>
              <dd className="text-sm font-light text-black/70">{seller.email}</dd>
            </div>
          )}
          {seller.phone && (
            <div>
              <dt className={labelClass}>Phone</dt>
              <dd className="text-sm font-light text-black/70">{seller.phone}</dd>
            </div>
          )}
          {seller.location && (
            <div>
              <dt className={labelClass}>Location</dt>
              <dd className="text-sm font-light text-black/70">{seller.location}</dd>
            </div>
          )}
          <div>
            <dt className={labelClass}>Assigned agent</dt>
            <dd className="text-sm font-light text-black/70">
              {seller.assignedAgent ?? 'Unassigned'}
            </dd>
          </div>
        </dl>
      ) : (
        <p className="text-sm font-light leading-6 text-black/45">
          No seller is assigned to this listing. Seller assignment is read-only
          here and managed through the relationship/transaction workflows.
        </p>
      )}
    </Section>
  )
}

// ---------------------------------------------------------------------------
// Story 5 — Listing tasks (reuses existing task actions)
// ---------------------------------------------------------------------------

function TasksPanel({
  propertyId,
  openTasks,
  onSaved,
}: {
  propertyId: string
  openTasks: PropertyWorkspace['openTasks']
  onSaved: () => void
}) {
  const [isPending, startTransition] = useTransition()
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null)
  const [title, setTitle] = useState('')
  const [detail, setDetail] = useState('')
  const [dueAt, setDueAt] = useState('')

  function create(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!title.trim()) {
      setMessage({ ok: false, text: 'Task title is required.' })
      return
    }
    setMessage(null)
    startTransition(async () => {
      const result = resolve(
        await createTaskAction({
          title,
          detail: detail.trim() || null,
          propertyId,
          dueAt: dueAt || null,
        }),
      )
      if (result.ok) {
        setTitle('')
        setDetail('')
        setDueAt('')
        setMessage({ ok: true, text: 'Task created.' })
        onSaved()
      } else {
        setMessage({ ok: false, text: result.message ?? 'Could not create task.' })
      }
    })
  }

  function run(action: () => Promise<unknown>, successText: string) {
    setMessage(null)
    startTransition(async () => {
      const result = resolve(await action())
      if (result.ok) {
        setMessage({ ok: true, text: successText })
        onSaved()
      } else {
        setMessage({ ok: false, text: result.message ?? 'Action failed.' })
      }
    })
  }

  return (
    <Section
      title="Listing tasks"
      subtitle="Property-scoped open work. Uses the same task actions as the rest of the Portal."
      right={<ActionMessage message={message} />}
    >
      <form onSubmit={create} className="mb-5 grid gap-3 rounded-sm border border-[var(--portal-border)] bg-[var(--portal-blue-pale)]/30 p-4 md:grid-cols-2">
        {field({
          label: 'New task title',
          children: <input className={fieldInput} value={title} onChange={(event) => setTitle(event.target.value)} placeholder="e.g. Confirm title search" />,
        })}
        <div className="grid gap-3 md:grid-cols-2">
          {field({
            label: 'Due (local)',
            children: <input type="datetime-local" className={fieldInput} value={dueAt} onChange={(event) => setDueAt(event.target.value)} />,
          })}
          <div className="flex items-end">
            <button type="submit" disabled={isPending} className={primaryButton}>
              {isPending ? 'Adding…' : 'Add task'}
            </button>
          </div>
        </div>
        <div className="md:col-span-2">
          {field({
            label: 'Detail (optional)',
            children: <input className={fieldInput} value={detail} onChange={(event) => setDetail(event.target.value)} />,
          })}
        </div>
      </form>

      {openTasks.length === 0 ? (
        <p className="text-sm font-light text-black/40">No open tasks for this listing.</p>
      ) : (
        <ul className="divide-y divide-[var(--portal-border)]">
          {openTasks.map((task) => (
            <li key={task.id} className="py-4">
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div className="min-w-0 flex-1">
                  <p className="font-serif text-base font-light">{task.title}</p>
                  {task.detail && (
                    <p className="mt-1 text-sm font-light text-black/55">{task.detail}</p>
                  )}
                  <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs font-light text-black/45">
                    {task.dueAtLabel ? (
                      <span className={task.isOverdue ? 'text-[var(--portal-archive)]' : undefined}>
                        Due {task.dueAtLabel}
                        {task.isOverdue ? ' · overdue' : ''}
                      </span>
                    ) : (
                      <span>No due date</span>
                    )}
                    {task.personName && <span>· {task.personName}</span>}
                    {task.dealName && <span>· deal {task.dealName}</span>}
                  </div>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  <button
                    type="button"
                    disabled={isPending}
                    onClick={() => run(() => completeTaskAction(task.id), 'Task completed.')}
                    className={secondaryButton}
                  >
                    Complete
                  </button>
                  <button
                    type="button"
                    disabled={isPending}
                    onClick={() => run(() => cancelTaskAction(task.id), 'Task cancelled.')}
                    className={ghostButton}
                  >
                    Cancel
                  </button>
                </div>
              </div>
            </li>
          ))}
        </ul>
      )}
    </Section>
  )
}

// ---------------------------------------------------------------------------
// Story 7 — Property activity timeline
// ---------------------------------------------------------------------------

const ACTIVITY_LABELS: Record<string, string> = {
  interaction: 'Interaction',
  showing: 'Showing',
  intake: 'Enquiry',
  task: 'Task',
  deal: 'Deal',
}

function ActivityPanel({ activity }: { activity: PropertyWorkspace['activity'] }) {
  return (
    <Section
      title="Operational timeline"
      subtitle="What has happened around this listing, from canonical records."
      right={
        <span className="text-xs font-light text-black/35">
          {activity.length} events
        </span>
      }
    >
      {activity.length === 0 ? (
        <p className="text-sm font-light text-black/40">No activity recorded for this listing yet.</p>
      ) : (
        <ol className="space-y-4">
          {activity.map((entry) => (
            <li key={`${entry.kind}-${entry.id}`} className="flex gap-4">
              <div className="mt-0.5 w-20 shrink-0 text-[10px] font-light uppercase tracking-[0.14em] text-black/40">
                {ACTIVITY_LABELS[entry.kind] ?? entry.kind}
              </div>
              <div className="min-w-0 flex-1">
                <p className="font-serif text-base font-light">{entry.title}</p>
                {entry.summary && (
                  <p className="mt-1 line-clamp-2 text-sm font-light text-black/55">
                    {entry.summary}
                  </p>
                )}
                <p className="mt-1 text-xs font-light text-black/45">
                  {entry.atLabel}
                  {entry.personName ? ` · ${entry.personName}` : ''}
                  {entry.personId && entry.kind !== 'intake' ? (
                    <span>
                      {' '}
                      ·{' '}
                      <Link
                        href={`/portal/clients/${entry.personId}`}
                        className="text-[var(--portal-navy-soft)] underline-offset-2 hover:underline"
                      >
                        Dossier
                      </Link>
                    </span>
                  ) : null}
                  {entry.dealId ? (
                    <span>
                      {' '}
                      ·{' '}
                      <Link
                        href={`/portal/deals/${entry.dealId}`}
                        className="text-[var(--portal-navy-soft)] underline-offset-2 hover:underline"
                      >
                        Deal
                      </Link>
                    </span>
                  ) : null}
                </p>
              </div>
            </li>
          ))}
        </ol>
      )}
    </Section>
  )
}

// ---------------------------------------------------------------------------
// Story 8 — Interest / enquiries
// ---------------------------------------------------------------------------

function InterestPanel({
  interests,
  enquiries,
}: {
  interests: WorkspaceInterest[]
  enquiries: WorkspaceEnquiry[]
}) {
  const unresolved = enquiries.filter(
    (item) => item.status === 'received' || item.status === 'resolution_required',
  )

  return (
    <Section
      title="Interest & enquiries"
      subtitle="Who has shown interest in this property?"
      right={
        unresolved.length > 0 ? (
          <Link href="/portal/needs-review" className={secondaryButton}>
            {unresolved.length} in Needs Review
          </Link>
        ) : undefined
      }
    >
      {interests.length > 0 && (
        <div className="mb-6">
          <h3 className={labelClass}>Tracked interest</h3>
          <ul className="divide-y divide-[var(--portal-border)]">
            {interests.map((item) => (
              <li key={item.id} className="flex flex-wrap items-center justify-between gap-2 py-3">
                <div>
                  <Link
                    href={`/portal/clients/${item.personId}`}
                    className="font-serif text-base font-light text-[var(--portal-navy-soft)] underline-offset-2 hover:underline"
                  >
                    {item.personName}
                  </Link>
                  <p className="mt-0.5 text-xs font-light text-black/45">
                    {friendlyLabel(INTEREST_STATUS_LABELS, item.status)}
                    {item.ranking ? ` · rank ${item.ranking}` : ''} · since {item.createdAtLabel}
                  </p>
                </div>
                <Link href={`/portal/clients/${item.personId}`} className="text-xs font-light text-[var(--portal-navy-soft)] underline-offset-2 hover:underline">
                  Dossier
                </Link>
              </li>
            ))}
          </ul>
        </div>
      )}

      <h3 className={labelClass}>Website enquiries</h3>
      {enquiries.length === 0 ? (
        <p className="text-sm font-light text-black/40">No website enquiries for this listing.</p>
      ) : (
        <ul className="divide-y divide-[var(--portal-border)]">
          {enquiries.map((item) => (
            <li key={item.id} className="flex flex-wrap items-center justify-between gap-2 py-3">
              <div className="min-w-0 flex-1">
                <p className="font-serif text-base font-light">{item.displayName}</p>
                <p className="mt-0.5 text-xs font-light text-black/45">
                  {item.requestType === 'private_viewing' ? 'Private viewing' : 'Property information'} ·{' '}
                  {item.email} · {item.receivedAtLabel}
                </p>
                {item.message && (
                  <p className="mt-1 line-clamp-2 text-sm font-light text-black/55">{item.message}</p>
                )}
              </div>
              <span
                className={`rounded-sm px-2 py-1 text-[10px] font-light uppercase tracking-[0.14em] ${
                  item.status === 'received' || item.status === 'resolution_required'
                    ? 'bg-[var(--portal-archive)]/10 text-[var(--portal-archive)]'
                    : 'bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]'
                }`}
              >
                {friendlyLabel(ENQUIRY_STATUS_LABELS, item.status)}
              </span>
            </li>
          ))}
        </ul>
      )}
    </Section>
  )
}

// ---------------------------------------------------------------------------
// Story 9 — Showings & deals cross-linking
// ---------------------------------------------------------------------------

function ShowingsDealsPanel({
  showings,
  deals,
}: {
  showings: WorkspaceShowing[]
  deals: WorkspaceDeal[]
}) {
  return (
    <Section
      title="Showings & deals"
      subtitle="Operational context linking this listing to live activity."
      right={
        <Link href="/portal/showings" className="secondaryButton">
          Open Showings
        </Link>
      }
    >
      <h3 className={labelClass}>Showings</h3>
      {showings.length === 0 ? (
        <p className="mb-5 text-sm font-light text-black/40">No showings for this listing.</p>
      ) : (
        <ul className="mb-6 divide-y divide-[var(--portal-border)]">
          {showings.map((item) => (
            <li key={item.id} className="flex flex-wrap items-center justify-between gap-2 py-3">
              <div>
                <Link
                  href={`/portal/clients/${item.personId}`}
                  className="font-serif text-base font-light text-[var(--portal-navy-soft)] underline-offset-2 hover:underline"
                >
                  {item.personName}
                </Link>
                <p className="mt-0.5 text-xs font-light text-black/45">
                  {friendlyLabel(SHOWING_STATUS_LABELS, item.status)}
                  {item.scheduledAtLabel ? ` · ${item.scheduledAtLabel}` : ''}
                  {item.completedAtLabel ? ` · ${item.completedAtLabel}` : ''}
                </p>
              </div>
              {item.dealId && (
                <Link
                  href={`/portal/deals/${item.dealId}`}
                  className="text-xs font-light text-[var(--portal-navy-soft)] underline-offset-2 hover:underline"
                >
                  Deal
                </Link>
              )}
            </li>
          ))}
        </ul>
      )}

      <h3 className={labelClass}>Deals</h3>
      {deals.length === 0 ? (
        <p className="text-sm font-light text-black/40">No deals linked to this listing.</p>
      ) : (
        <ul className="divide-y divide-[var(--portal-border)]">
          {deals.map((item) => (
            <li key={item.id} className="flex flex-wrap items-center justify-between gap-2 py-3">
              <div>
                <Link
                  href={`/portal/deals/${item.id}`}
                  className="font-serif text-base font-light text-[var(--portal-navy-soft)] underline-offset-2 hover:underline"
                >
                  {friendlyLabel(STAGE_LABELS, item.stage)} · {item.clientName}
                </Link>
                <p className="mt-0.5 text-xs font-light text-black/45">
                  {item.showingCount} showings · {item.offerCount} offers · updated {item.updatedAtLabel}
                </p>
              </div>
              <Link
                href={`/portal/deals/${item.id}`}
                className="text-xs font-light text-[var(--portal-navy-soft)] underline-offset-2 hover:underline"
              >
                Deal Workspace
              </Link>
            </li>
          ))}
        </ul>
      )}
    </Section>
  )
}
