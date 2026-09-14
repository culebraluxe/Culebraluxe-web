import type { Metadata } from 'next'

// The recorder's tab name is its own name — not the TECH default.
export const metadata: Metadata = { title: 'Flight Recorder' }

export default function FlightRecorderLayout({ children }: { children: React.ReactNode }) {
  return children
}
