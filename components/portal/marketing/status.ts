import type { PlacementStatus } from '@/lib/syndication/channels'

export const STATUS_TONE: Record<
  PlacementStatus,
  { label: string; className: string; dot: string }
> = {
  draft: { label: 'Draft', className: 'text-black/45', dot: 'bg-black/25' },
  ready: { label: 'Ready', className: 'text-[var(--portal-navy)]', dot: 'bg-sky-400' },
  pending_manual: { label: 'Awaiting confirm', className: 'text-amber-700', dot: 'bg-amber-400' },
  live: { label: 'Live', className: 'text-emerald-700', dot: 'bg-emerald-400' },
  expired: { label: 'Expired', className: 'text-orange-700', dot: 'bg-orange-400' },
  failed: { label: 'Failed', className: 'text-rose-700', dot: 'bg-rose-400' },
  withdrawn: { label: 'Withdrawn', className: 'text-black/40', dot: 'bg-black/20' },
}

export const EVENT_LABEL: Record<string, string> = {
  pack_generated: 'Pack generated',
  publish_requested: 'Publish requested',
  marked_live: 'Marked live',
  confirmed: 'Confirmed live',
  failed: 'Failed',
  renewed: 'Renewed',
  withdrawn: 'Withdrawn',
  note: 'Note',
  ttl_lapsed: 'TTL expired',
}
