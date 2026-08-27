'use client'

import { Mic } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'

// ---------------------------------------------------------------------------
// ARA-VOICE-01 — Reusable microphone control for the Ara command input.
//
// Voice is INPUT ONLY: it converges into the SAME editable prompt text used by
// the keyboard, and submission ALWAYS happens through the existing typed
// command handler (Go / Enter). This control never submits anything on its own
// — recognition ending only fills the prompt for the operator to review.
//
// Dependency-free browser speech recognition (SpeechRecognition /
// webkitSpeechRecognition) with capability detection. When the browser has no
// speech support, the control renders a quiet, disabled-looking mic (never a
// fake listening state) and typed input keeps working untouched.
//
// Interaction:
//   IDLE      -> mic icon; tap starts listening (visible LISTENING state)
//   LISTENING -> gold/pale wash + "Listening"; tap again stops
//   recognized words call onTranscript(text); the parent merges into prompt
//   permission denied / no-speech / recognition error -> listening ends honestly
// ---------------------------------------------------------------------------

type SpeechRecognitionLike = {
  lang: string
  interimResults: boolean
  continuous: boolean
  onresult:
    | ((event: {
        results: ArrayLike<ArrayLike<{ transcript: string }>>
      }) => void)
    | null
  onerror: (() => void) | null
  onend: (() => void) | null
  start: () => void
  stop: () => void
}

function getSpeechRecognition(): (new () => SpeechRecognitionLike) | null {
  if (typeof window === 'undefined') return null
  const speech = window as Window & {
    SpeechRecognition?: new () => SpeechRecognitionLike
    webkitSpeechRecognition?: new () => SpeechRecognitionLike
  }
  return speech.SpeechRecognition ?? speech.webkitSpeechRecognition ?? null
}

export function AraMicButton({
  onTranscript,
  ariaLabel = 'Use microphone',
}: {
  /** Called with recognized speech so the parent can fill its editable prompt. */
  onTranscript: (text: string) => void
  ariaLabel?: string
}) {
  const [listening, setListening] = useState(false)
  const [unsupported, setUnsupported] = useState(false)
  const recognitionRef = useRef<SpeechRecognitionLike | null>(null)

  // Never leak a running recognizer when the control unmounts.
  useEffect(() => {
    return () => {
      recognitionRef.current?.stop()
    }
  }, [])

  function toggle() {
    if (listening) {
      recognitionRef.current?.stop()
      return
    }
    const Recognition = getSpeechRecognition()
    if (!Recognition) {
      setUnsupported(true)
      return
    }
    const recognition = new Recognition()
    recognition.lang = 'en-US'
    recognition.interimResults = false
    recognition.continuous = false
    recognition.onresult = (event) => {
      const spoken = event.results[0]?.[0]?.transcript?.trim()
      if (spoken) onTranscript(spoken)
    }
    recognition.onerror = () => {
      setListening(false)
      setUnsupported(false)
    }
    recognition.onend = () => {
      setListening(false)
      recognitionRef.current = null
    }
    recognitionRef.current = recognition
    setUnsupported(false)
    setListening(true)
    recognition.start()
  }

  const status =
    unsupported
      ? 'Voice input is not supported in this browser — type instead.'
      : listening
        ? 'Listening — tap to stop'
        : ariaLabel

  return (
    <button
      type="button"
      onClick={toggle}
      aria-pressed={listening}
      aria-label={status}
      title={status}
      className={[
        'inline-flex h-10 min-w-10 items-center justify-center gap-1.5 rounded-[var(--portal-tab-radius)] border px-2.5 transition',
        unsupported
          ? 'cursor-not-allowed border-[var(--portal-panel-border)] opacity-50'
          : listening
            ? 'border-[var(--portal-gold)] bg-[var(--portal-gold-pale)] text-[var(--portal-navy)]'
            : 'border-[var(--portal-panel-border)] text-[var(--portal-navy-soft)] hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)]',
      ].join(' ')}
    >
      <Mic className="h-4 w-4 shrink-0" aria-hidden />
      {listening ? (
        <span className="text-[9px] font-medium uppercase tracking-[0.1em]">
          Listening
        </span>
      ) : null}
    </button>
  )
}
