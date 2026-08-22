"use client"

import { useState, useTransition } from "react"

import {
  endStructuralParticipantAction,
  setStructuralParticipantAction,
} from "@/app/portal/actions"
import { PersonSelector } from "@/components/portal/write/person-selector"
import type { DealParticipant } from "@/db/deal-workspace"

// OPS-05 — structural participant maintenance in the Deal Workspace. The
// canonical participant model (migration 034) allows at most ONE active
// structural participant (client/owner/seller) per role per deal, so setting a
// role REPLACES the current holder atomically (end old + insert new + sync the
// per-deal legacy FK mirror). The client is always replaced — never ended —
// because a deal must keep a client (deal.client_person_id is NOT NULL).
// Subject kinds are fixed per role: client/seller are people, owner is an
// internal app user.

type Result = { ok: boolean; message?: string }

function resolve(result: unknown): Result {
  return result as Result
}

const secondaryButton =
  "inline-flex min-h-11 items-center justify-center rounded-sm border border-[var(--portal-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-40"

const ghostButton =
  "inline-flex min-h-11 items-center justify-center rounded-sm px-3 text-[11px] font-light uppercase tracking-[0.14em] text-black/45 transition hover:text-[var(--portal-archive)] disabled:cursor-not-allowed disabled:opacity-40"

export function StructuralParticipantControls({
  dealId,
  participants,
  ownerCandidates,
}: {
  dealId: string
  participants: DealParticipant[]
  ownerCandidates: { id: string; displayName: string; email: string | null }[]
}) {
  const structural = participants.filter((participant) =>
    ["client", "owner", "seller"].includes(participant.roleCategory),
  )
  const byRole = new Map(
    structural.map((participant) => [
      participant.roleCategory,
      participant,
    ]),
  )

  return (
    <div className="border-t border-[var(--portal-border)] px-6 py-5">
      <p className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
        Structural roles
      </p>
      <p className="mt-1 text-xs font-light text-black/40">
        One active client, owner and seller per deal. Setting a role replaces
        the current holder.
      </p>
      <div className="mt-4 space-y-4">
        <RoleControl
          role="client"
          label="Client"
          dealId={dealId}
          current={byRole.get("client")}
          ownerCandidates={ownerCandidates}
        />
        <RoleControl
          role="owner"
          label="Owner"
          dealId={dealId}
          current={byRole.get("owner")}
          ownerCandidates={ownerCandidates}
        />
        <RoleControl
          role="seller"
          label="Seller"
          dealId={dealId}
          current={byRole.get("seller")}
          ownerCandidates={ownerCandidates}
        />
      </div>
    </div>
  )
}

function RoleControl({
  role,
  label,
  dealId,
  current,
  ownerCandidates,
}: {
  role: "client" | "owner" | "seller"
  label: string
  dealId: string
  current: DealParticipant | undefined
  ownerCandidates: { id: string; displayName: string; email: string | null }[]
}) {
  const [isPending, startTransition] = useTransition()
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(
    null,
  )
  const [personId, setPersonId] = useState("")
  const [personLabel, setPersonLabel] = useState<string | null>(null)
  const [ownerUserId, setOwnerUserId] = useState("")

  const isPersonRole = role !== "owner"

  function run(action: () => Promise<unknown>, successText: string) {
    setMessage(null)
    startTransition(async () => {
      const result = resolve(await action())
      if (result.ok) {
        setPersonId("")
        setPersonLabel(null)
        setOwnerUserId("")
        setMessage({ ok: true, text: successText })
      } else {
        setMessage({ ok: false, text: result.message ?? "Action failed." })
      }
    })
  }

  function setRole() {
    if (isPersonRole) {
      if (!personId) {
        setMessage({ ok: false, text: "Select a person first." })
        return
      }
      run(
        () =>
          setStructuralParticipantAction({
            dealId,
            role,
            personId,
          }),
        `${label} updated.`,
      )
    } else {
      if (!ownerUserId) {
        setMessage({ ok: false, text: "Choose an owner user first." })
        return
      }
      run(
        () =>
          setStructuralParticipantAction({
            dealId,
            role,
            userId: ownerUserId,
          }),
        "Owner updated.",
      )
    }
  }

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 rounded-sm border border-[var(--portal-border)] px-4 py-3">
      <div className="min-w-0">
        <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
          {label}
        </div>
        <div className="mt-1 truncate font-serif text-base font-light">
          {current ? current.name : "Not set"}
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {isPersonRole ? (
          <div className="w-52">
            <PersonSelector
              selectedLabel={personLabel}
              onSelect={(id, label) => {
                setPersonId(id)
                setPersonLabel(label || null)
              }}
              placeholder={
                current ? `Replace ${label.toLowerCase()}…` : `Set ${label.toLowerCase()}…`
              }
            />
          </div>
        ) : (
          <select
            value={ownerUserId}
            onChange={(event) => setOwnerUserId(event.target.value)}
            aria-label="Owner user"
            className="block min-h-11 w-52 rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
          >
            <option value="">
              {current ? "Replace owner…" : "Set owner…"}
            </option>
            {ownerCandidates.map((user) => (
              <option key={user.id} value={user.id}>
                {user.displayName}
                {user.email ? ` — ${user.email}` : ""}
              </option>
            ))}
          </select>
        )}

        <button
          type="button"
          onClick={setRole}
          disabled={isPending}
          className={secondaryButton}
        >
          {current ? "Replace" : "Set"}
        </button>

        {current && role !== "client" && (
          <button
            type="button"
            disabled={isPending}
            onClick={() =>
              run(
                () => endStructuralParticipantAction(current.id),
                `${label} ended.`,
              )
            }
            className={ghostButton}
          >
            End
          </button>
        )}
      </div>

      {message && (
        <div
          className={`w-full text-xs font-light ${
            message.ok ? "text-black/50" : "text-[var(--portal-archive)]"
          }`}
        >
          {message.text}
        </div>
      )}
    </div>
  )
}
