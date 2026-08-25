"use client"

import { Menu } from "@base-ui/react/menu"
import { MoreHorizontal, type LucideIcon } from "lucide-react"

import { cn } from "@/lib/utils"

export type PortalRowMenuItem = {
  label: string
  icon?: LucideIcon
  onSelect?: () => void
  tone?: "default" | "danger"
}

export function PortalRowMenu({
  ariaLabel = "Row actions",
  items,
}: {
  ariaLabel?: string
  items: PortalRowMenuItem[]
}) {
  return (
    <Menu.Root>
      <Menu.Trigger
        aria-label={ariaLabel}
        className="flex h-11 w-11 items-center justify-center rounded-[var(--portal-tab-radius)] border border-transparent text-[var(--portal-blue-gray)] transition hover:border-[var(--portal-panel-border)] hover:bg-white hover:text-[var(--portal-navy)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-gold)]/60 data-[popup-open]:border-[var(--portal-panel-border)] data-[popup-open]:bg-white"
      >
        <MoreHorizontal className="h-4 w-4" />
      </Menu.Trigger>
      <Menu.Portal>
        <Menu.Positioner className="z-[70] outline-none" sideOffset={6} align="end">
          <Menu.Popup className="w-52 rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white/95 p-1.5 shadow-[var(--portal-panel-shadow)] backdrop-blur-xl outline-none">
            {items.map(({ icon: Icon, label, onSelect, tone = "default" }) => (
              <Menu.Item
                key={label}
                onClick={onSelect}
                className={cn(
                  "flex min-h-11 cursor-default items-center gap-2 rounded-md px-3 text-sm font-light outline-none transition data-[highlighted]:bg-[var(--portal-blue-pale)]",
                  tone === "danger"
                    ? "text-[var(--portal-archive)]"
                    : "text-[var(--portal-navy-soft)] data-[highlighted]:text-[var(--portal-navy)]",
                )}
              >
                {Icon ? <Icon className="h-3.5 w-3.5" /> : null}
                {label}
              </Menu.Item>
            ))}
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  )
}
