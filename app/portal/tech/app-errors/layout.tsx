import type { Metadata } from 'next'

export const metadata: Metadata = { title: 'App Error Capture' }

export default function AppErrorsLayout({ children }: { children: React.ReactNode }) {
  return children
}
