'use client'

import Link from 'next/link'
import { useState, useTransition } from 'react'

import { breakGlassLoginAction } from './actions'

export default function RecoveryPage() {
  const [secret, setSecret] = useState('')
  const [message, setMessage] = useState<string | null>(null)
  const [isPending, startTransition] = useTransition()

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setMessage(null)
    startTransition(async () => {
      const result = await breakGlassLoginAction(secret)
      setSecret('')
      setMessage(
        result.ok
          ? 'Authenticated. Establishing an administrative session…'
          : 'Invalid credentials.',
      )
    })
  }

  return (
    <main className="flex min-h-screen items-center justify-center bg-[#f5f2ec] px-6">
      <div className="w-full max-w-sm rounded-sm border border-[#030f23]/10 bg-white p-8">
        <h1 className="font-serif text-xl font-light">
          Emergency administrative access
        </h1>
        <p className="mt-2 text-sm font-light leading-6 text-black/50">
          For CulebraLuxe administrators only. This path is independent of the
          normal sign-in provider and is intended solely for outage recovery.
        </p>

        <form onSubmit={submit} className="mt-6 space-y-4">
          <label className="block">
            <span className="mb-1 block text-[10px] font-light uppercase tracking-[0.18em] text-black/45">
              Recovery credential
            </span>
            <input
              type="password"
              autoComplete="off"
              value={secret}
              onChange={(event) => setSecret(event.target.value)}
              className="block min-h-12 w-full rounded-sm border border-[#030f23]/15 bg-white px-4 text-sm font-light outline-none focus:border-[#030f23]"
            />
          </label>

          <button
            type="submit"
            disabled={isPending || secret.length === 0}
            className="inline-flex min-h-12 w-full items-center justify-center rounded-sm bg-[#030f23] px-4 text-xs font-light uppercase tracking-[0.16em] text-white transition disabled:opacity-40"
          >
            {isPending ? 'Verifying…' : 'Access Portal'}
          </button>
        </form>

        {message && (
          <p className="mt-4 text-xs font-light text-black/55">{message}</p>
        )}

        <div className="mt-6 text-center">
          <Link
            href="/login"
            className="text-xs font-light text-[#030f23]/45 underline-offset-2 hover:underline"
          >
            Back to sign in
          </Link>
        </div>
      </div>
    </main>
  )
}
