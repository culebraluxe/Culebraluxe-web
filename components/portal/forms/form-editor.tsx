"use client"

import Link from "next/link"
import { useState, useTransition } from "react"

import {
  issueFormAction,
  sendIssuedFormForSignatureAction,
  updateFormAction,
} from "@/app/portal/forms/actions"
import { interpolateSectionText } from "@/lib/forms/pdf"
import type { TemplateDefinition } from "@/lib/forms/template-types"

const inputClass =
  "mt-2 block w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/50 px-3 py-2.5 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
const labelClass =
  "text-[10px] font-light uppercase tracking-[0.18em] text-black/40"
const primaryButton =
  "inline-flex min-h-9 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[11px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40"
const secondaryButton =
  "inline-flex min-h-9 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:opacity-40"

export function FormEditor({
  form,
  template,
}: {
  form: {
    id: string
    status: string
    fieldValues: Record<string, string>
    sections: Record<string, string>
  }
  template: TemplateDefinition
}) {
  const [values, setValues] = useState<Record<string, string>>(form.fieldValues)
  const [sections, setSections] = useState<Record<string, string>>(form.sections)
  const [previewKey, setPreviewKey] = useState(0)
  const [message, setMessage] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [issued, setIssued] = useState<{
    documentId: string
    issuedVersion: number
    checksum: string
  } | null>(null)
  const [isPending, startTransition] = useTransition()
  const isIssued = form.status === "issued" || issued !== null

  function persist(
    nextValues: Record<string, string>,
    nextSections: Record<string, string>,
  ) {
    setMessage("Unsaved changes")
    startTransition(async () => {
      const result = await updateFormAction(form.id, nextValues, nextSections)
      if (result.ok) {
        setMessage("Saved")
        setPreviewKey((key) => key + 1)
      } else {
        setError(result.message ?? "Could not save.")
      }
    })
  }

  function setValue(name: string, value: string) {
    const next = { ...values, [name]: value }
    setValues(next)
    persist(next, sections)
  }

  function setSection(name: string, value: string) {
    const next = { ...sections, [name]: value }
    setSections(next)
    persist(values, next)
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="font-serif text-2xl font-light text-[var(--portal-navy)]">
          {template.displayName}
        </h1>
        <span className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">
          {isIssued ? "Issued" : form.status} · v{template.version}
        </span>
      </div>

      {issued ? (
        <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
          <div className="font-serif text-lg font-light">
            Issued v{issued.issuedVersion}
          </div>
          <p className="mt-1 break-all text-xs font-light text-black/45">
            sha256 {issued.checksum}
          </p>
          <div className="mt-3 flex flex-wrap gap-2">
            <Link
              href={`/portal/documents/${issued.documentId}/download`}
              className={secondaryButton}
            >
              Download PDF
            </Link>
            <button
              type="button"
              disabled={isPending}
              onClick={() => {
                startTransition(async () => {
                  const result = await sendIssuedFormForSignatureAction(
                    issued.documentId,
                  )
                  if (result.ok) setMessage("Sent for signature")
                  else setError(result.message ?? "Could not send.")
                })
              }}
              className={primaryButton}
            >
              Send for signature
            </button>
          </div>
        </section>
      ) : null}

      <div className="grid gap-4 lg:grid-cols-2">
        <section className="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
          <div className="border-b border-[var(--portal-panel-border)] px-4 py-3">
            <h2 className="font-serif text-lg font-light">Edit</h2>
          </div>
          <div className="grid gap-4 p-4 md:grid-cols-2">
            {template.fields.map((field) => (
              <label
                key={field.name}
                className={field.type === "textarea" ? "md:col-span-2" : "block"}
              >
                <span className={labelClass}>
                  {field.label}
                  {field.required ? " *" : ""}
                </span>
                {field.type === "textarea" ? (
                  <textarea
                    rows={3}
                    value={values[field.name] ?? ""}
                    onChange={(event) => setValue(field.name, event.target.value)}
                    disabled={isIssued}
                    className={`${inputClass} resize-y`}
                  />
                ) : field.type === "select" ? (
                  <select
                    value={values[field.name] ?? ""}
                    onChange={(event) => setValue(field.name, event.target.value)}
                    disabled={isIssued}
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
                    disabled={isIssued}
                    className={inputClass}
                  />
                )}
              </label>
            ))}
          </div>

          {template.sections
            .filter((section) => section.editable)
            .map((section) => (
              <div
                key={section.name}
                className="border-t border-[var(--portal-panel-border)] p-4"
              >
                <label className="block">
                  <span className={labelClass}>{section.label}</span>
                  <textarea
                    rows={4}
                    value={
                      sections[section.name] ||
                      interpolateSectionText(
                        section,
                        values,
                        (field, raw) => raw,
                        template.fields,
                      )
                    }
                    onChange={(event) =>
                      setSection(section.name, event.target.value)
                    }
                    disabled={isIssued}
                    className={`${inputClass} resize-y`}
                  />
                </label>
              </div>
            ))}

          <div className="flex flex-wrap items-center gap-3 border-t border-[var(--portal-panel-border)] px-4 py-3">
            <button
              type="button"
              disabled={isPending || isIssued}
              onClick={() => {
                setError(null)
                startTransition(async () => {
                  const result = await issueFormAction(form.id)
                  if (result.ok) {
                    setIssued(result.data)
                    setMessage("Issued")
                    setPreviewKey((key) => key + 1)
                  } else {
                    setError(result.message ?? "Could not issue.")
                  }
                })
              }}
              className={primaryButton}
            >
              {isPending ? "Working…" : "Issue PDF"}
            </button>
            <Link href="/portal/documents" className={secondaryButton}>
              Vault
            </Link>
            {message ? (
              <span className="text-xs font-light text-black/45">{message}</span>
            ) : null}
          </div>
          {error ? (
            <p className="px-4 pb-4 text-xs font-light text-[var(--portal-archive)]">
              {error}
            </p>
          ) : null}
        </section>

        <section className="portal-glass-panel flex min-h-[70vh] flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
          <div className="border-b border-[var(--portal-panel-border)] px-4 py-3">
            <h2 className="font-serif text-lg font-light">Live preview</h2>
            <p className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">
              Same renderer as issuance
            </p>
          </div>
          <iframe
            title="Form PDF preview"
            src={`/portal/forms/${form.id}/preview?v=${previewKey}`}
            className="min-h-[70vh] w-full flex-1 bg-white"
          />
        </section>
      </div>
    </div>
  )
}
