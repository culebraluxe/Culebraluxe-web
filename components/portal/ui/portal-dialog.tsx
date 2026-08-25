"use client"

import { Dialog } from "@base-ui/react/dialog"
import { X } from "lucide-react"
import type { ComponentProps, ReactNode } from "react"

import { cn } from "@/lib/utils"

export const portalDialogTriggerClass =
  "inline-flex min-h-11 items-center justify-center gap-2 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-4 text-[11px] font-medium uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-gold)]/60"

export function PortalDialog({
  actions,
  children,
  description,
  title,
  trigger,
  triggerClassName,
}: {
  actions?: ReactNode
  children?: ReactNode
  description?: ReactNode
  title: ReactNode
  trigger: ReactNode
  triggerClassName?: string
}) {
  return (
    <Dialog.Root>
      <Dialog.Trigger type="button" className={cn(portalDialogTriggerClass, triggerClassName)}>
        {trigger}
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Backdrop className="fixed inset-0 z-[80] bg-[var(--portal-navy)]/45 backdrop-blur-sm" />
        <Dialog.Viewport className="fixed inset-0 z-[81] grid overflow-y-auto p-4 sm:p-8">
          <Dialog.Popup className="portal-glass-panel m-auto w-full max-w-md rounded-[var(--portal-panel-radius)] bg-white/95 p-6 shadow-[var(--portal-feature-shadow)] outline-none">
            <header className="flex items-start justify-between gap-4">
              <div className="min-w-0">
                <Dialog.Title className="font-serif text-2xl font-light text-[var(--portal-navy)]">
                  {title}
                </Dialog.Title>
                {description ? (
                  <Dialog.Description className="mt-2 text-sm font-light leading-6 text-black/55">
                    {description}
                  </Dialog.Description>
                ) : null}
              </div>
              <Dialog.Close
                type="button"
                className="flex h-11 w-11 shrink-0 items-center justify-center rounded-[var(--portal-tab-radius)] text-black/40 transition hover:bg-[var(--portal-rail-hover-bg)] hover:text-[var(--portal-navy)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-gold)]/60"
                aria-label="Close dialog"
              >
                <X className="h-4 w-4" />
              </Dialog.Close>
            </header>
            {children ? <div className="mt-5">{children}</div> : null}
            {actions ? <div className="mt-6 flex flex-wrap justify-end gap-2">{actions}</div> : null}
          </Dialog.Popup>
        </Dialog.Viewport>
      </Dialog.Portal>
    </Dialog.Root>
  )
}

export function PortalDialogClose({
  children,
  className,
  ...props
}: ComponentProps<typeof Dialog.Close>) {
  return (
    <Dialog.Close type="button" className={className} {...props}>
      {children}
    </Dialog.Close>
  )
}
