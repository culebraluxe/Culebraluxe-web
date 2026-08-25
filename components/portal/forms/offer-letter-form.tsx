"use client"

import Link from "next/link"
import { useState, useTransition } from "react"

import {
  issueFormAction,
  updateFormAction,
} from "@/app/portal/forms/actions"
import { formatDate, formatMoney } from "@/lib/forms/format"
import type { TemplateDefinition } from "@/lib/forms/template-types"

const inputClass =
  "mt-2 block w-full rounded-[var(--portal-panel-radius)] portal-glass-panel px-3 py-2.5 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
const labelClass = "text-[10px] font-light uppercase tracking-[0.18em] text-black/40"
const primaryButton =
  "inline-flex min-h-11 items-center justify-center rounded-sm bg-[var(--portal-navy)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40"
const secondaryButton =
  "inline-flex min-h-11 items-center justify-center rounded-sm border border-[var(--portal-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-40"

function renderPreviewValue(
  field: TemplateDefinition["fields"][number],
  value: string,
): string {
  if (field.type === "money" && value.trim()) return formatMoney(value)
  if (field.type === "date" && value.trim()) return formatDate(value)
  return value
}

export function OfferLetterForm({
  form,
  template,
}: {
  form: {
    id: string
    status: string
    fieldValues: Record<string, string>
    sections: Record<string, string>
    dealId: string
  }
  template: TemplateDefinition
}) {
  const [values, setValues] = useState<Record<string, string>>(form.fieldValues)
  const [sections, setSections] = useState<Record<string, string>>(form.sections)
  const [message, setMessage] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [issued, setIssued] = useState<{
    documentId: string
    issuedVersion: number
    checksum: string
  } | null>(null)
  const [isPending, startTransition] = useTransition()
  const isIssued = form.status === "issued" || issued !== null

  function setValue(name: string, value: string) {
    const next = { ...values, [name]: value }
    setValues(next)
    setMessage("Unsaved changes")
    startTransition(async () => {
      const result = await updateFormAction(form.id, next, sections)
      if (result.ok) setMessage("Saved")
      else setError(result.message ?? "Could not save.")
    })
  }

  function setSection(name: string, value: string) {
    const next = { ...sections, [name]: value }
    setSections(next)
    setMessage("Unsaved changes")
    startTransition(async () => {
      const result = await updateFormAction(form.id, values, next)
      if (result.ok) setMessage("Saved")
      else setError(result.message ?? "Could not save.")
    })
  }

  function issue() {
    setError(null)
    setMessage(null)
    startTransition(async () => {
      const result = await issueFormAction(form.id)
      if (result.ok) {
        setIssued(result.data)
        setMessage("Issued")
      } else {
        setError(result.message ?? "Could not issue the document.")
      }
    })
  }

  return (
    <div>
      <header className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          NEXUS · Forms
        </p>
        <div className="mt-3 flex items-baseline gap-4">
          <h1 className="font-serif text-4xl font-light leading-[1.1]">
            {template.displayName}
          </h1>
          <span
            className={`rounded-full px-3 py-1 text-xs font-light uppercase tracking-[0.16em] ${
              isIssued
                ? "border border-[var(--portal-blue-gray)]/40 text-[var(--portal-navy-soft)]"
                : "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]"
            }`}
          >
            {isIssued ? "Issued" : form.status}
          </span>
        </div>
        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Structured fields + one bounded editable section. Issuing creates an
          immutable PDF in the Documents repository — editing afterward starts
          a new version, never overwriting the issued artifact.
        </p>
      </header>

      {issued && (
        <section className="mb-6 rounded-sm border border-[var(--portal-blue-gray)]/40 bg-white p-6">
          <h2 className="font-serif text-xl font-light">
            Document issued — v{issued.issuedVersion}
          </h2>
          <p className="mt-2 break-all text-xs font-light text-black/50">
            sha256 {issued.checksum}
          </p>
          <Link
            href={`/portal/documents/${issued.documentId}/download`}
            className="mt-4 inline-flex text-sm font-light text-[var(--portal-navy)] underline underline-offset-4 hover:text-[var(--portal-archive)]"
          >
            Download the issued PDF
          </Link>
        </section>
      )}

      <div className="grid gap-6 lg:grid-cols-2">
// __PART2__
        <section className="rounded-[var(--portal-panel-radius)] portal-glass-panel">
          <div className="border-b border-[var(--portal-border)] px-6 py-5">
            <h2 className="font-serif text-2xl font-light">Structured terms</h2>
          </div>
          <div className="grid gap-4 p-6 md:grid-cols-2">
            {template.fields.map((field) => (
              <label key={field.name} className="block">
                <span className={labelClass}>
                  {field.label}
                  {field.required ? " *" : ""}
                </span>
                {field.type === "textarea" ? (
                  <textarea
                    rows={3}
                    value={values[field.name] ?? ""}
                    onChange={(event) => setValue(field.name, event.target.value)}
                    className={`${inputClass} resize-y`}
                  />
                ) : field.type === "select" ? (
                  <select
                    value={values[field.name] ?? ""}
                    onChange={(event) => setValue(field.name, event.target.value)}
                    className={`${inputClass} min-h-11`}
                  >
                    <option value="">—</option>
                    {(field.options ?? []).map((option) => (
                      <option key={option} value={option}>
                        {option}
                      </option>
                    ))}
                  </select>
                ) : (
                  <input
                    type={field.type === "date" ? "date" : "text"}
                    value={values[field.name] ?? ""}
                    onChange={(event) => setValue(field.name, event.target.value)}
                    className={inputClass}
                  />
                )}
              </label>
            ))}
          </div>

          {template.sections.map((section) => (
            <div key={section.name} className="border-t border-[var(--portal-border)] p-6">
              <label className="block">
                <span className={labelClass}>{section.label}</span>
                <textarea
                  rows={4}
                  value={sections[section.name] ?? ""}
                  onChange={(event) => setSection(section.name, event.target.value)}
                  className={`${inputClass} resize-y`}
                />
              </label>
            </div>
          ))}

          <div className="flex items-center gap-3 border-t border-[var(--portal-border)] px-6 py-5">
            <button
              type="button"
              disabled={isPending || isIssued}
              onClick={issue}
              className={primaryButton}
            >
              {isPending ? "Working…" : "Issue PDF"}
            </button>
            <Link href="/portal/documents" className={secondaryButton}>
              Documents
            </Link>
            {message && (
              <span className="text-xs font-light text-black/50">{message}</span>
            )}
          </div>
          {error && (
            <p className="px-6 pb-5 text-xs font-light text-[var(--portal-archive)]">{error}</p>
          )}
        </section>
// __PART3__
        <section className="rounded-[var(--portal-panel-radius)] portal-glass-panel">
          <div className="border-b border-[var(--portal-border)] px-6 py-5">
            <h2 className="font-serif text-2xl font-light">Preview</h2>
          </div>
          <div className="bg-black/[0.03] p-6">
            <div className="bg-white p-10 shadow-sm">
              <div className="text-[9px] font-light uppercase tracking-[0.3em] text-black/40">
                {template.rendering.issuer}
              </div>
              <h3 className="mt-2 font-serif text-3xl font-light">
                {template.rendering.title}
              </h3>
              <div className="mt-4 h-px w-full bg-[var(--portal-navy)]/20" />
              <div className="mt-6 space-y-4">
                {template.fields.map((field) => {
                  const value = (values[field.name] ?? "").trim()
                  if (!value && !field.required) return null
                  return (
                    <div key={field.name}>
                      <div className="text-[9px] font-light uppercase tracking-[0.18em] text-black/40">
                        {field.label}
                      </div>
                      <div className="mt-0.5 text-sm font-light text-black/70">
                        {value ? renderPreviewValue(field, value) : "—"}
                      </div>
                    </div>
                  )
                })}
                {template.sections.map((section) => {
                  const value = (sections[section.name] ?? "").trim()
                  if (!value) return null
                  return (
                    <div key={section.name}>
                      <div className="text-[9px] font-light uppercase tracking-[0.18em] text-black/40">
                        {section.label}
                      </div>
                      <p className="mt-0.5 text-sm font-light leading-6 text-black/70">
                        {value}
                      </p>
                    </div>
                  )
                })}
              </div>
            </div>
          </div>
        </section>
      </div>
    </div>
  )
}


