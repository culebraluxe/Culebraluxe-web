import type { ReactNode } from "react"
import { PortalHeader } from "@/components/portal/portal-header"
import { PortalSidebar } from "@/components/portal/portal-sidebar"

export default function PortalLayout({
  children,
}: {
  children: ReactNode
}) {
  return (
    <div className="min-h-screen bg-[var(--portal-bg)] text-[var(--portal-text)]">
      <div className="flex min-h-screen">
        <PortalSidebar />

        <div className="min-w-0 flex-1">
          <PortalHeader />

          <main className="px-6 py-8 lg:px-10 lg:py-10 xl:px-14">
            {children}
          </main>
        </div>
      </div>
    </div>
  )
}