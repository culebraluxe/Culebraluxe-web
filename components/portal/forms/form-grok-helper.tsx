"use client"

import { useEffect, useRef, useState } from "react"

type SpeechRecognitionLike = {
  lang: string
  interimResults: boolean
  continuous: boolean
  onresult: ((event: { results: ArrayLike<ArrayLike<{ transcript: string }>> }) => void) | null
  onerror: (() => void) | null
  onend: (() => void) | null
  start: () => void
  stop: () => void
}

function getSpeechRecognition(): (new () => SpeechRecognitionLike) | null {
  if (typeof window === "undefined") return null
  const speech = window as Window & {
    SpeechRecognition?: new () => SpeechRecognitionLike
    webkitSpeechRecognition?: new () => SpeechRecognitionLike
  }
  return speech.SpeechRecognition ?? speech.webkitSpeechRecognition ?? null
}

export function FormGrokHelper({
  formTitle,
  busy = false,
  onAsk,
}: {
  formTitle: string
  busy?: boolean
  onAsk: (prompt: string) => Promise<string>
}) {
  const [prompt, setPrompt] = useState("")
  const [listening, setListening] = useState(false)
  const [note, setNote] = useState<string | null>(null)
  const [working, setWorking] = useState(false)
  const recognitionRef = useRef<SpeechRecognitionLike | null>(null)

  useEffect(() => {
    return () => {
      recognitionRef.current?.stop()
    }
  }, [])

  function startListening() {
    const Recognition = getSpeechRecognition()
    if (!Recognition) {
      setNote("This browser can’t take voice yet. Type, or try Safari / Chrome.")
      return
    }
    const recognition = new Recognition()
    recognition.lang = "en-US"
    recognition.interimResults = false
    recognition.continuous = false
    recognition.onresult = (event) => {
      const spoken = event.results[0]?.[0]?.transcript?.trim()
      if (spoken) setPrompt((current) => (current ? `${current} ${spoken}` : spoken))
    }
    recognition.onerror = () => setListening(false)
    recognition.onend = () => {
      setListening(false)
      recognitionRef.current = null
    }
    recognitionRef.current = recognition
    setListening(true)
    setNote(null)
    recognition.start()
  }

  async function submitPrompt() {
    const text = prompt.trim()
    if (!text) {
      setNote("Tell Grok what happened on the deal, then tap Go.")
      return
    }
    setWorking(true)
    try {
      const reply = await onAsk(text)
      setNote(reply)
    } catch {
      setNote("Grok could not fill the form right now. Try again in a moment.")
    } finally {
      setWorking(false)
    }
  }

  return (
    <div className="w-full">
      <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] px-4 py-3 sm:px-5">
        <div className="flex flex-wrap items-end gap-2">
          <div className="min-w-0 flex-1">
            <p className="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">
              Grok · {formTitle}
            </p>
            <input
              value={prompt}
              onChange={(event) => setPrompt(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault()
                  void submitPrompt()
                }
              }}
              placeholder="Tell Grok what happened — or tap the mic"
              className="mt-1.5 block h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 font-serif text-[15px] font-light text-[var(--portal-navy)] outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
            />
          </div>
          <button
            type="button"
            onClick={() => {
              if (listening) {
                recognitionRef.current?.stop()
                return
              }
              startListening()
            }}
            className={[
              "inline-flex h-10 min-w-10 items-center justify-center rounded-[var(--portal-tab-radius)] border px-3 text-[10px] font-medium uppercase tracking-[0.12em] transition",
              listening
                ? "border-[var(--portal-gold)] bg-[var(--portal-gold-pale)] text-[var(--portal-navy)]"
                : "border-[var(--portal-panel-border)] text-[var(--portal-navy-soft)] hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)]",
            ].join(" ")}
            aria-pressed={listening}
          >
            {listening ? "Listening" : "Mic"}
          </button>
          <button
            type="button"
            disabled={busy || working}
            onClick={() => {
              void submitPrompt()
            }}
            className="inline-flex h-10 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:opacity-40"
          >
            {working ? "Grok…" : "Go"}
          </button>
        </div>
        <p className="mt-2 text-center text-[11px] font-light text-black/45">
          {note ??
            "Say it like you would to Grok. She can fill the fields; you still Save and Send."}
        </p>
      </section>
    </div>
  )
}
