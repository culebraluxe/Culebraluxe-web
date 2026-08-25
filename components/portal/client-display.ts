// ---------------------------------------------------------------------------
// CLIENTS — shared display helpers (pure). Used by the CORE ClientManager rail
// and the Client Card so relationship wording/tokens stay consistent and are
// not duplicated across files. No React, no data access.
// ---------------------------------------------------------------------------

import type {
  ClientRole,
  ClientStatus,
  PropertyInterestStatus,
} from "@/lib/portal/types"

export function formatCurrency(value?: number) {
  if (!value) return "—"

  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(value)
}

export function formatPhone(value?: string | null): string | null {
  if (!value) return null
  const digits = value.replace(/\D/g, "")
  if (digits.length === 10) {
    return digits.replace(/(\d{3})(\d{3})(\d{4})/, "$1-$2-$3")
  }
  if (digits.length === 11) {
    return `+${digits[0]} ${digits.slice(1, 4)}-${digits.slice(4, 7)}-${digits.slice(7)}`
  }
  return value
}

export function roleLabel(role: ClientRole) {
  if (role === "both") return "Buyer & Seller"
  return role.charAt(0).toUpperCase() + role.slice(1)
}

export function statusLabel(status: ClientStatus) {
  return status.charAt(0).toUpperCase() + status.slice(1)
}

export function interestStatusLabel(status: PropertyInterestStatus) {
  switch (status) {
    case "tour_completed":
      return "Tour completed"
    case "shortlisted":
      return "Shortlisted"
    default:
      return "Interested"
  }
}

export function initials(name: string) {
  return name
    .split(" ")
    .slice(0, 2)
    .map((word) => word[0])
    .join("")
}

export const ghostBtn =
  "inline-flex min-h-9 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)]"

export function statusDot(status: string) {
  switch (status) {
    case "active":
      return "bg-[var(--portal-success)]"
    case "warm":
      return "bg-[var(--portal-navy-soft)]"
    case "referral":
      return "bg-[var(--portal-neutral)]"
    default:
      return "bg-black/25"
  }
}
