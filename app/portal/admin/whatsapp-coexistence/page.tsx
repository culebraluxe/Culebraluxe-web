"use client"

import { useEffect, useMemo, useState } from "react"

declare global {
  interface Window {
    FB?: {
      init: (options: {
        appId: string
        cookie?: boolean
        xfbml?: boolean
        version: string
      }) => void
      login: (
        callback: (response: {
          authResponse?: { code?: string }
          status?: string
        }) => void,
        options: Record<string, unknown>,
      ) => void
    }
    fbAsyncInit?: () => void
  }
}

const META_APP_ID = "1573618894304413"
const GRAPH_VERSION = "v26.0"
const CONFIG_STORAGE_KEY = "culebraluxe.whatsapp.embeddedSignupConfigId"

type SessionEvent = {
  type?: string
  event?: string
  version?: number
  data?: {
    waba_id?: string
    phone_number_id?: string
    [key: string]: unknown
  }
  [key: string]: unknown
}

export default function WhatsAppCoexistencePage() {
  const [sdkReady, setSdkReady] = useState(false)
  const [configId, setConfigId] = useState("")
  const [status, setStatus] = useState("Loading Meta SDK…")
  const [sessionEvent, setSessionEvent] = useState<SessionEvent | null>(null)
  const [authorizationCodeReceived, setAuthorizationCodeReceived] = useState(false)

  useEffect(() => {
    const stored = window.localStorage.getItem(CONFIG_STORAGE_KEY)
    if (stored) setConfigId(stored)

    const handleMessage = (event: MessageEvent) => {
      if (event.origin !== "https://www.facebook.com" && event.origin !== "https://web.facebook.com") {
        return
      }

      let payload: unknown = event.data
      if (typeof payload === "string") {
        try {
          payload = JSON.parse(payload)
        } catch {
          return
        }
      }

      if (!payload || typeof payload !== "object") return
      const candidate = payload as SessionEvent
      if (candidate.type !== "WA_EMBEDDED_SIGNUP") return

      setSessionEvent(candidate)
      if (candidate.event === "FINISH_WHATSAPP_BUSINESS_APP_ONBOARDING") {
        setStatus("Coexistence onboarding finished in Meta.")
      } else {
        setStatus(`Meta Embedded Signup event: ${candidate.event ?? "unknown"}`)
      }
    }

    window.addEventListener("message", handleMessage)

    window.fbAsyncInit = () => {
      window.FB?.init({
        appId: META_APP_ID,
        cookie: true,
        xfbml: false,
        version: GRAPH_VERSION,
      })
      setSdkReady(true)
      setStatus("Meta SDK ready. Enter the Embedded Signup Configuration ID.")
    }

    if (document.getElementById("facebook-jssdk")) {
      if (window.FB) window.fbAsyncInit()
    } else {
      const script = document.createElement("script")
      script.id = "facebook-jssdk"
      script.async = true
      script.defer = true
      script.crossOrigin = "anonymous"
      script.src = "https://connect.facebook.net/en_US/sdk.js"
      document.body.appendChild(script)
    }

    return () => {
      window.removeEventListener("message", handleMessage)
    }
  }, [])

  const canLaunch = useMemo(() => sdkReady && configId.trim().length > 0, [sdkReady, configId])

  function launchCoexistence() {
    const normalizedConfigId = configId.trim()
    if (!window.FB || !sdkReady || !normalizedConfigId) return

    window.localStorage.setItem(CONFIG_STORAGE_KEY, normalizedConfigId)
    setStatus("Opening Meta Coexistence Embedded Signup…")
    setSessionEvent(null)
    setAuthorizationCodeReceived(false)

    // Important: FB.login must run synchronously from this click handler or browsers can block the popup.
    window.FB.login(
      (response) => {
        if (response.authResponse?.code) {
          setAuthorizationCodeReceived(true)
          setStatus((current) =>
            current === "Coexistence onboarding finished in Meta."
              ? current
              : "Meta returned the Embedded Signup authorization code.",
          )
          return
        }

        setStatus(response.status ? `Meta login status: ${response.status}` : "Meta signup was closed or cancelled.")
      },
      {
        config_id: normalizedConfigId,
        response_type: "code",
        override_default_response_type: true,
        extras: {
          setup: {},
          featureType: "whatsapp_business_app_onboarding",
          sessionInfoVersion: "3",
        },
      },
    )
  }

  return (
    <main className="min-h-screen bg-brand-navy px-6 py-12 text-brand-ivory">
      <div className="mx-auto max-w-3xl space-y-8">
        <div>
          <p className="text-xs uppercase tracking-[0.28em] text-brand-gold">Private diagnostic</p>
          <h1 className="mt-3 font-serif text-3xl">WhatsApp Coexistence Launcher</h1>
          <p className="mt-3 text-sm text-brand-ivory/70">
            Launches Meta Embedded Signup in WhatsApp Business App coexistence mode. This page does not
            disconnect, migrate, register, or modify a phone number by itself.
          </p>
        </div>

        <section className="rounded-2xl border border-brand-gold/25 bg-white/5 p-6 space-y-5">
          <div className="grid gap-4 sm:grid-cols-2">
            <div>
              <p className="text-xs uppercase tracking-wider text-brand-ivory/50">Meta App ID</p>
              <p className="mt-1 font-mono text-sm">{META_APP_ID}</p>
            </div>
            <div>
              <p className="text-xs uppercase tracking-wider text-brand-ivory/50">Launch mode</p>
              <p className="mt-1 text-sm text-brand-gold">WhatsApp Business App coexistence</p>
            </div>
          </div>

          <label className="block">
            <span className="text-xs uppercase tracking-wider text-brand-ivory/50">
              Embedded Signup Configuration ID
            </span>
            <input
              value={configId}
              onChange={(event) => setConfigId(event.target.value)}
              placeholder="Paste Meta configuration ID"
              className="mt-2 w-full rounded-xl border border-brand-gold/25 bg-black/20 px-4 py-3 font-mono text-sm text-brand-ivory outline-none focus:border-brand-gold/60"
              autoComplete="off"
            />
          </label>

          <button
            type="button"
            disabled={!canLaunch}
            onClick={launchCoexistence}
            className="rounded-xl border border-brand-gold/40 bg-brand-gold/10 px-5 py-3 text-sm font-medium text-brand-gold disabled:cursor-not-allowed disabled:opacity-40"
          >
            Connect Existing WhatsApp Business Number
          </button>

          <div className="rounded-xl border border-white/10 bg-black/15 p-4">
            <p className="text-xs uppercase tracking-wider text-brand-ivory/50">Status</p>
            <p className="mt-2 text-sm">{status}</p>
          </div>
        </section>

        <section className="rounded-2xl border border-brand-gold/25 bg-white/5 p-6">
          <h2 className="font-medium">What this launcher sends to Meta</h2>
          <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2">
            <div>
              <dt className="text-brand-ivory/50">featureType</dt>
              <dd className="mt-1 font-mono text-xs text-brand-gold">whatsapp_business_app_onboarding</dd>
            </div>
            <div>
              <dt className="text-brand-ivory/50">sessionInfoVersion</dt>
              <dd className="mt-1 font-mono text-xs">3</dd>
            </div>
            <div>
              <dt className="text-brand-ivory/50">response_type</dt>
              <dd className="mt-1 font-mono text-xs">code</dd>
            </div>
            <div>
              <dt className="text-brand-ivory/50">Authorization code returned</dt>
              <dd className="mt-1 text-xs">{authorizationCodeReceived ? "Yes" : "Not yet"}</dd>
            </div>
          </dl>
        </section>

        {sessionEvent ? (
          <section className="rounded-2xl border border-brand-gold/25 bg-white/5 p-6">
            <h2 className="font-medium">Latest Meta session event</h2>
            <pre className="mt-4 overflow-x-auto whitespace-pre-wrap break-words rounded-xl bg-black/20 p-4 text-xs text-brand-ivory/80">
              {JSON.stringify(sessionEvent, null, 2)}
            </pre>
          </section>
        ) : null}

        <section className="rounded-2xl border border-amber-400/30 bg-amber-400/10 p-6">
          <p className="text-sm text-brand-ivory/80">
            Do not use Meta&apos;s ordinary phone migration/disconnect flow for the existing Business App number.
            This launcher explicitly requests the coexistence onboarding branch.
          </p>
        </section>
      </div>
    </main>
  )
}
