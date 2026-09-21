import Link from "next/link"
import { redirect } from "next/navigation"

import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import {
  listRecentErrors,
  type AppErrorRow,
  type ErrorLevel,
} from "@/db/app-error"

export const dynamic = "force-dynamic"

const LEVELS: ErrorLevel[] = ["info", "warn", "error", "fatal"]

const LEVEL_TONE: Record<ErrorLevel, string> = {
  info: "bg-[var(--portal-navy)]/5 text-black/60",
  warn: "bg-amber-500/10 text-amber-700",
  error: "bg-red-500/10 text-red-700",
  fatal: "bg-red-600/20 text-red-800",
}

function asTime(value: unknown): string {
  if (value instanceof Date) return value.toLocaleString()
  if (typeof value === "string") return new Date(value).toLocaleString()
  if (typeof value === "number") return new Date(value).toLocaleString()
  return ""
}

function clip(text: string | null | undefined, len: number): string {
  if (!text) return "—"
  return text.length > len ? `${text.slice(0, len)}…` : text
}

function LevelChips({ selected }: { selected: ErrorLevel | "all" }) {
  const options: Array<{ key: ErrorLevel | "all"; label: string }> = [
    { key: "all", label: "All" },
    ...LEVELS.map((l) => ({ key: l as ErrorLevel | "all", label: l })),
  ]
  return (
    <div className="flex flex-wrap items-center gap-2">
      {options.map((o) => {
        const href = o.key === "all" ? "/portal/tech/app-errors" : `/portal/tech/app-errors?level=${o.key}`
        const active = selected === o.key
        return (
          <Link
            key={o.key}
            href={href}
            className={`rounded-full border px-3 py-1 text-xs font-medium uppercase tracking-[0.08em] transition-colors ${
              active
                ? "border-[var(--portal-navy)] bg-[var(--portal-navy)] text-white"
                : "border-black/10 text-black/55 hover:border-[var(--portal-navy)]/40 hover:text-[var(--portal-navy)]"
            }`}
          >
            {o.label}
          </Link>
        )
      })}
    </div>
  )
}

function ErrorRow({ row }: { row: AppErrorRow }) {
  const level = (LEVELS.includes(row.level as ErrorLevel) ? row.level : "error") as ErrorLevel
  return (
    <tr className="border-b border-black/5 align-top">
      <td className="px-3 py-2.5">
        <span className={`inline-block rounded px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide ${LEVEL_TONE[level]}`}>
          {row.level}
        </span>
      </td>
      <td className="px-3 py-2.5 whitespace-nowrap text-xs tabular-nums text-black/45">{asTime(row.created_at)}</td>
      <td className="px-3 py-2.5 text-xs font-medium text-[var(--portal-navy)]">{row.kind}</td>
      <td className="px-3 py-2.5 text-xs text-black/60">{clip(row.operation, 42)}</td>
      <td className="max-w-[26rem] px-3 py-2.5 text-xs leading-5 text-black/70">{clip(row.message, 160)}</td>
      <td className="px-3 py-2.5 text-[11px] text-black/35">{clip(row.route, 30)}</td>
    </tr>
  )
}

export default async function AppErrorsPage({
  searchParams,
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>
}) {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    "tech.access",
  )
  if (!access.ok) redirect(access.redirectTo)

  const params = await searchParams
  const rawLevel = typeof params.level === "string" ? params.level : null
  const selected: ErrorLevel | "all" =
    rawLevel && (LEVELS as string[]).includes(rawLevel)
      ? (rawLevel as ErrorLevel)
      : "all"

  const rows = await listRecentErrors(100, selected === "all" ? undefined : selected)

  return (
    <div className="mx-auto max-w-6xl space-y-5 p-4 lg:p-6">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="font-serif text-2xl font-light text-[var(--portal-navy)]">
            App Error Capture
          </h1>
          <p className="mt-1 max-w-3xl text-sm font-light leading-6 text-black/55">
            Recent durable <code className="rounded bg-black/5 px-1">app_error</code> rows
            (Log4j-style capture: gateway DB failures, unhandled service exceptions,
            and route/server-action errors). Filter by severity; intended business
            outcomes (validation, authorization denials) are deliberately absent.
          </p>
        </div>
        <Link
          href="/portal/tech"
          className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)]/60 hover:text-[var(--portal-navy)]"
        >
          ← Tech
        </Link>
      </div>

      <LevelChips selected={selected} />

      <div className="overflow-x-auto rounded-[var(--portal-panel-radius)] border border-black/10 bg-white/60">
        {rows.length === 0 ? (
          <p className="p-6 text-sm font-light text-black/45">
            No captured errors{selected === "all" ? "" : ` at level “${selected}”`} yet.
          </p>
        ) : (
          <table className="w-full border-collapse text-left">
            <thead>
              <tr className="border-b border-black/10 text-[10px] font-semibold uppercase tracking-[0.12em] text-black/40">
                <th className="px-3 py-2">Level</th>
                <th className="px-3 py-2">When</th>
                <th className="px-3 py-2">Kind</th>
                <th className="px-3 py-2">Operation</th>
                <th className="px-3 py-2">Message</th>
                <th className="px-3 py-2">Route</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <ErrorRow key={row.id} row={row} />
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  )
}
