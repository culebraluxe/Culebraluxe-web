"use client"

import { useEffect, useRef, useState } from "react"
import { useRouter } from "next/navigation"

import {
  createFormAction,
  issueFormAction,
  updateFormAction,
} from "@/app/portal/forms/actions"
import { documentBodyText, formatFieldValue } from "@/lib/forms/format"
import type { TemplateDefinition } from "@/lib/forms/template-types"

const inputClass =
  "mt-1 block h-9 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-2.5 text-[13px] font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
const sectionHeadingClass =
  "font-serif text-base font-bold text-[var(--portal-navy)]"
const labelClass =
  "text-[9px] font-light uppercase tracking-[0.14em] text-black/40"
const primaryButton =
  "inline-flex min-h-8 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40"
const ghostBtn =
  "inline-flex min-h-8 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:opacity-40"

function fieldSpanClass(field: { name: string; label: string; type: string }) {
  if (field.type === "textarea") return "col-span-6"
  if (
    field.type === "date" ||
    field.type === "money" ||
    field.type === "select"
  ) {
    return "col-span-2"
  }
  if (/name|property|location|address/i.test(`${field.name} ${field.label}`)) {
    return "col-span-3"
  }
  return "col-span-2"
}

function partyName(form: {
  buyerName: string | null
  sellerName: string | null
  clientName: string | null
}) {
  return (
    [form.buyerName || form.clientName, form.sellerName]
      .filter(Boolean)
      .join(" / ") || "Untitled"
  )
}

function sessionLabel(form: {
  buyerName: string | null
  sellerName: string | null
  clientName: string | null
  propertyLabel: string | null
  updatedAt: string
}) {
  return [partyName(form), form.propertyLabel, form.updatedAt.slice(0, 10)]
    .filter(Boolean)
    .join(" · ")
}

function statusDotClass(status: string) {
  if (status === "issued") return "bg-[var(--portal-success)]"
  if (status === "ready") return "bg-[var(--portal-navy-soft)]"
  return "bg-black/25"
}

function initialDetailsText(
  template: TemplateDefinition,
  sections: Record<string, string>,
  values: Record<string, string>,
) {
  if (sections.body?.trim()) return sections.body
  return documentBodyText(template, values)
}

function fileFromPdfBytes(buffer: ArrayBuffer, filename: string): File {
  const bytes = new Uint8Array(buffer)
  const header = String.fromCharCode(...bytes.subarray(0, 5))
  if (header !== "%PDF-") {
    throw new Error("The generated file was not a PDF.")
  }
  const safeFilename = filename.endsWith(".pdf")
    ? filename
    : `${filename}.pdf`
  return new File([bytes], safeFilename, {
    type: "application/pdf",
    lastModified: Date.now(),
  })
}

function fileSafeName(value: string) {
  const cleaned = value
    .replace(/[^\w\s-]+/g, "")
    .trim()
    .replace(/\s+/g, "-")
  return cleaned || "form"
}

function isUserCancel(error: unknown) {
  return error instanceof DOMException && error.name === "AbortError"
}

export function FormEditor({
  form,
  template,
  templates,
  savedForms = [],
  issuedDocument = null,
}: {
  form: {
    id: string
    status: string
    templateId: string
    dealId: string | null
    personId: string | null
    propertyId: string | null
    fieldValues: Record<string, string>
    sections: Record<string, string>
  }
  template: TemplateDefinition
  templates: { id: string; displayName: string }[]
  savedForms?: {
    id: string
    templateId: string
    status: string
    clientName: string | null
    propertyLabel: string | null
    buyerName: string | null
    sellerName: string | null
    updatedAt: string
  }[]
  issuedDocument?: {
    documentId: string
    issuedVersion: number
    checksum: string
  } | null
}) {
  const router = useRouter()
  const [values, setValues] = useState<Record<string, string>>(form.fieldValues)
  const [sections, setSections] = useState<Record<string, string>>(form.sections)
  const [detailsText, setDetailsText] = useState(() =>
    initialDetailsText(template, form.sections, form.fieldValues),
  )
  const bodyTouched = useRef(Boolean(form.sections.body?.trim()))
  const [saved, setSaved] = useState({
    values: form.fieldValues,
    sections: form.sections,
    detailsText: initialDetailsText(
      template,
      form.sections,
      form.fieldValues,
    ),
  })
  const [message, setMessage] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [issued, setIssued] = useState<{
    documentId: string
    issuedVersion: number
    checksum: string
  } | null>(issuedDocument)
  const [busy, setBusy] = useState(false)
  const [sessionQuery, setSessionQuery] = useState("")
  const working = busy
  const vaultVersion = issued?.issuedVersion ?? null
  const savedOfType = savedForms.filter(
    (item) => item.templateId === form.templateId,
  )
  const needle = sessionQuery.trim().toLowerCase()
  const filteredSaved = needle
    ? savedOfType.filter((item) =>
        sessionLabel(item).toLowerCase().includes(needle),
      )
    : savedOfType
  const currentSaved = savedOfType.find((item) => item.id === form.id)
  const visibleSaved =
    currentSaved && !filteredSaved.some((item) => item.id === form.id)
      ? [currentSaved, ...filteredSaved]
      : filteredSaved
  const dirty =
    JSON.stringify(values) !== JSON.stringify(saved.values) ||
    detailsText !== saved.detailsText
  const statusCue = dirty
    ? "Unsaved"
    : vaultVersion
      ? `Vault v${vaultVersion}`
      : form.status === "issued"
        ? "Issued"
        : "Draft"

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const target = event.target
      if (
        working ||
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement
      ) {
        return
      }
      const ids = visibleSaved.map((item) => item.id)
      const index = ids.indexOf(form.id)
      if (event.key === "ArrowDown" && index >= 0 && index < ids.length - 1) {
        event.preventDefault()
        router.push(`/portal/forms/${ids[index + 1]}`)
      }
      if (event.key === "ArrowUp" && index > 0) {
        event.preventDefault()
        router.push(`/portal/forms/${ids[index - 1]}`)
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [form.id, router, visibleSaved, working])

  async function startNewForm(templateId: string) {
    const result = await createFormAction({
      templateId,
      dealId: form.dealId ?? undefined,
      personId: form.personId ?? undefined,
      propertyId: form.propertyId ?? undefined,
    })
    if (result.ok) router.push(`/portal/forms/${result.data.formId}`)
    else {
      setError(result.message ?? "Could not start a new form.")
      setBusy(false)
    }
  }
  const signatureGroups =
    template.signatureGroups.length > 0
      ? template.signatureGroups
      : [
          {
            role: "PARTY",
            label: "Signature",
            field: template.fields[0]?.name ?? null,
            initials: false,
          },
        ]

  function composedSections(
    nextDetails = detailsText,
  ): Record<string, string> {
    return { ...sections, body: nextDetails }
  }

  function updateField(name: string, value: string) {
    const next = { ...values, [name]: value }
    setValues(next)
    if (!bodyTouched.current) {
      setDetailsText(documentBodyText(template, next))
    }
  }

  async function saveDraft(
    nextValues = values,
    nextDetails = detailsText,
  ): Promise<boolean> {
    const nextSections = composedSections(nextDetails)
    const result = await updateFormAction(form.id, nextValues, nextSections)
    if (result.ok) {
      setSections(nextSections)
      setSaved({
        values: nextValues,
        sections: nextSections,
        detailsText: nextDetails,
      })
      setMessage("Saved")
      setError(null)
      return true
    }
    setError(result.message ?? "Could not save.")
    return false
  }

  function cancelEdits() {
    setValues(saved.values)
    setSections(saved.sections)
    setDetailsText(saved.detailsText)
    setMessage("Changes discarded")
    setError(null)
  }

  function pdfFilename() {
    const who =
      values.buyerName ||
      values.sellerName ||
      values.clientName ||
      template.displayName
    return `${fileSafeName(who)}-${fileSafeName(template.id)}.pdf`
  }

  async function savePdfToVault() {
    if (dirty) {
      const savedOk = await saveDraft()
      if (!savedOk) {
        throw new Error("Could not save the form before creating the PDF.")
      }
    }
    const result = await issueFormAction(form.id)
    if (!result.ok) {
      throw new Error(result.message ?? "Could not save the PDF to the vault.")
    }
    setIssued(result.data)
    return result.data
  }

  async function livePdfFile() {
    const response = await fetch(`/portal/forms/${form.id}/preview`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        fieldValues: values,
        sections: composedSections(),
      }),
    })
    if (!response.ok) {
      throw new Error("Could not build the PDF.")
    }
    return fileFromPdfBytes(await response.arrayBuffer(), pdfFilename())
  }

  async function savePdf() {
    setError(null)
    setBusy(true)
    try {
      const document = await savePdfToVault()
      setMessage(`Saved to vault v${document.issuedVersion}`)
    } catch (caught) {
      if (isUserCancel(caught)) return
      setError(caught instanceof Error ? caught.message : "Could not save the PDF.")
    } finally {
      setBusy(false)
    }
  }

  async function sharePdf() {
    setError(null)
    try {
      const file = await livePdfFile()
      if (typeof navigator.share !== "function") {
        setMessage(
          "This browser can't attach a PDF from the page. Save PDF, then attach that file in Mail or Messages.",
        )
        return
      }
      await navigator.share({
        title: "CulebraLuxe Document",
        text: "CulebraLuxe transaction document",
        files: [file],
      })
      setMessage("Shared")
    } catch (caught) {
      if (isUserCancel(caught)) return
      const name = caught instanceof DOMException ? caught.name : ""
      if (name === "NotAllowedError" || name === "TypeError") {
        setMessage(
          "Share needs a direct click. Try Share PDF again, or Save PDF and attach the file.",
        )
        return
      }
      setError(caught instanceof Error ? caught.message : "Could not share the PDF.")
    }
  }

  return (
    <div className="flex min-h-0 flex-col gap-3">
      {error ? (
        <p className="text-xs font-light text-[var(--portal-archive)]">{error}</p>
      ) : null}

      <div className="grid min-h-0 flex-1 gap-4 lg:h-[calc(100dvh-8.5rem)] lg:grid-cols-[220px_minmax(0,1fr)_minmax(0,1fr)]">
        <aside className="portal-glass-panel flex max-h-72 min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)] lg:max-h-none">
          <div className="shrink-0 border-b border-[var(--portal-panel-border)] p-2.5">
            <div className="mb-2 flex items-center justify-between gap-2">
              <span className="text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                Forms · {visibleSaved.length}
              </span>
              <button
                type="button"
                disabled={working}
                onClick={() => {
                  setBusy(true)
                  void startNewForm(form.templateId)
                }}
                className="inline-flex min-h-7 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:opacity-40"
              >
                New
              </button>
            </div>
            <input
              type="search"
              value={sessionQuery}
              onChange={(event) => setSessionQuery(event.target.value)}
              placeholder="Search…"
              className="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-2.5 py-1.5 text-sm font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
            />
            <select
              value={form.templateId}
              disabled={working}
              onChange={(event) => {
                const nextTemplateId = event.target.value
                if (nextTemplateId === form.templateId) return
                const latest = savedForms.find(
                  (item) => item.templateId === nextTemplateId,
                )
                setBusy(true)
                if (latest) {
                  router.push(`/portal/forms/${latest.id}`)
                  return
                }
                void startNewForm(nextTemplateId)
              }}
              className="mt-2 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-2.5 py-1.5 text-sm font-light outline-none focus:border-[var(--portal-navy)] disabled:opacity-40"
            >
              {templates.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.displayName}
                </option>
              ))}
            </select>
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto">
            {visibleSaved.length === 0 ? (
              <p className="px-3 py-6 text-sm font-light text-black/40">
                No matching {template.displayName.toLowerCase()} forms
                {needle ? "." : " yet."}
              </p>
            ) : (
              visibleSaved.map((item) => {
                const selected = item.id === form.id
                const who = partyName(item)
                return (
                  <button
                    type="button"
                    key={item.id}
                    disabled={working}
                    onClick={() => {
                      if (item.id !== form.id) {
                        router.push(`/portal/forms/${item.id}`)
                      }
                    }}
                    ref={
                      selected
                        ? (node) => node?.scrollIntoView({ block: "nearest" })
                        : undefined
                    }
                    className={[
                      "flex w-full items-center gap-2 border-b border-[var(--portal-panel-border)] px-2.5 py-2 text-left transition",
                      selected
                        ? "border-l-2 border-l-[var(--portal-gold)] bg-white/40"
                        : "border-l-2 border-l-transparent hover:bg-white/25",
                    ].join(" ")}
                  >
                    <span
                      className={`h-1.5 w-1.5 shrink-0 rounded-full ${
                        selected && dirty
                          ? "bg-[var(--portal-gold)]"
                          : statusDotClass(item.status)
                      }`}
                      aria-label={
                        selected && dirty ? "Unsaved" : item.status
                      }
                    />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[13px] font-medium text-[var(--portal-navy)]">
                        {who}
                      </div>
                      <div className="truncate text-[11px] font-light text-black/45">
                        {[item.propertyLabel, item.updatedAt.slice(0, 10)]
                          .filter(Boolean)
                          .join(" · ") || template.displayName}
                      </div>
                    </div>
                  </button>
                )
              })
            )}
          </div>
        </aside>

        <section className="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
          <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-[var(--portal-panel-border)] px-4 py-2.5">
            <div className="min-w-0">
              <h2 className="truncate font-serif text-lg font-light text-[var(--portal-navy)]">
                {template.displayName}
              </h2>
              <p className="text-[11px] font-light text-black/45">
                {message ?? statusCue}
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <button
                type="button"
                disabled={working}
                onClick={() => {
                  void savePdf()
                }}
                className={primaryButton}
              >
                {working ? "Working…" : "Save"}
              </button>
              <button
                type="button"
                disabled={working}
                onClick={() => {
                  void sharePdf()
                }}
                className={ghostBtn}
              >
                Share PDF
              </button>
              <button
                type="button"
                disabled={working || !dirty}
                onClick={cancelEdits}
                className={ghostBtn}
              >
                Cancel
              </button>
            </div>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto p-5">
          <div className="grid grid-cols-6 gap-x-3 gap-y-3.5">
            {template.fields.map((field) => (
              <label
                key={field.name}
                className={`${fieldSpanClass(field)} min-w-0`}
              >
                <span className={labelClass}>
                  {field.label}
                  {field.required ? " *" : ""}
                </span>
                {field.type === "textarea" ? (
                  <textarea
                    rows={2}
                    value={values[field.name] ?? ""}
                    onChange={(event) =>
                      updateField(field.name, event.target.value)
                    }
                    className={`${inputClass} h-auto min-h-12 resize-y py-1.5`}
                  />
                ) : field.type === "select" ? (
                  <select
                    value={values[field.name] ?? ""}
                    onChange={(event) =>
                      updateField(field.name, event.target.value)
                    }
                    className={inputClass}
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
                    onChange={(event) =>
                      updateField(field.name, event.target.value)
                    }
                    className={inputClass}
                  />
                )}
              </label>
            ))}
          </div>

          <div className="mt-6">
            <h2 className={sectionHeadingClass}>Document</h2>
            <p className="mt-1 text-xs font-light text-black/40">
              Template text from the form. Edit it like a Word document.
            </p>
            <textarea
              id="deal-details"
              rows={14}
              value={detailsText}
              placeholder="Document text…"
              onChange={(event) => {
                bodyTouched.current = true
                setDetailsText(event.target.value)
              }}
              className="mt-2 block min-h-[16rem] w-full resize-y rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/80 px-3 py-2.5 font-serif text-[15px] font-light leading-7 text-black/80 outline-none focus:border-[var(--portal-navy-soft)] disabled:opacity-60"
            />
          </div>
          </div>
        </section>

        <section className="portal-glass-panel min-h-0 overflow-hidden rounded-[var(--portal-panel-radius)]">
          <div className="h-full overflow-y-auto bg-[var(--portal-blue-pale)]/55 px-3 py-4 lg:px-5 lg:py-5">
            <article className="mx-auto min-h-[calc(100%-0.5rem)] w-full max-w-[40rem] bg-white px-10 py-11 text-black/80 shadow-[0_12px_36px_rgba(24,43,64,0.14)] ring-1 ring-black/[0.06]">
            <p className="text-[9px] font-light uppercase tracking-[0.18em] text-black/40">
              {template.rendering.issuer}
            </p>
            <h2 className="mt-1 font-serif text-xl font-bold text-[var(--portal-navy)]">
              {template.rendering.title}
            </h2>

            <p className="my-5 border-y border-black/10 py-4 font-serif text-[15px] font-light leading-7">
              {template.fields
                .map((field) => {
                  const raw = (values[field.name] ?? "").trim()
                  if (!raw) return null
                  return formatFieldValue(field, raw)
                })
                .filter((part): part is string => Boolean(part))
                .join(" · ")}
            </p>

            {detailsText.split(/\n{2,}/).map((block, index) => {
              const lines = block.split("\n")
              const heading = lines[0]?.trim() ?? ""
              const para = lines.slice(1).join("\n").trim()
              if (!heading && !para) return null
              return (
                <article key={index} className="mt-5">
                  {para ? (
                    <>
                      <h3 className={sectionHeadingClass}>{heading}</h3>
                      <p className="mt-2 whitespace-pre-wrap font-serif text-[15px] font-light leading-7">
                        {para}
                      </p>
                    </>
                  ) : (
                    <p className="whitespace-pre-wrap font-serif text-[15px] font-light leading-7">
                      {heading}
                    </p>
                  )}
                </article>
              )
            })}

            <div className="mt-10 border-t border-black/20 pt-6">
              <h3 className={sectionHeadingClass}>Signatures</h3>
              <div className="mt-4 grid gap-6 sm:grid-cols-2">
                {signatureGroups.map((group) => {
                  const name = group.field
                    ? (values[group.field] ?? "").trim()
                    : ""
                  return (
                    <div key={`${group.role}-${group.label}`}>
                      <p className={labelClass}>{group.label}</p>
                      <p className="mt-0.5 font-serif text-sm font-light text-[var(--portal-navy)]">
                        {name || "Name on file"}
                      </p>
                      <div className="mt-5 border-b border-black/45 pb-0.5 text-[10px] font-light text-black/40">
                        Signature
                      </div>
                      <div className="mt-4 flex items-end gap-5">
                        {group.initials ? (
                          <div className="w-20 border-b border-black/45 pb-0.5 text-[10px] font-light text-black/40">
                            Initials
                          </div>
                        ) : null}
                        <div className="min-w-[7rem] flex-1 border-b border-black/45 pb-0.5 text-[10px] font-light text-black/40">
                          Date
                        </div>
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>
            </article>
          </div>
        </section>
      </div>
    </div>
  )
}
