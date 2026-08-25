"use client"

import { Combobox } from "@base-ui/react/combobox"
import { Check, ChevronDown, X } from "lucide-react"
import { useId } from "react"

import { cn } from "@/lib/utils"

import {
  PortalField,
  PortalFieldDescription,
  PortalFieldError,
  PortalFieldLabel,
  portalControlClass,
} from "./portal-field"

export type PortalComboboxOption = {
  value: string
  label: string
  description?: string
}

export function PortalCombobox({
  className,
  defaultValue,
  description,
  disabled = false,
  error,
  label,
  name,
  onValueChange,
  options,
  placeholder = "Choose an option…",
}: {
  className?: string
  defaultValue?: string
  description?: string
  disabled?: boolean
  error?: string
  label: string
  name?: string
  onValueChange?: (value: string | null) => void
  options: PortalComboboxOption[]
  placeholder?: string
}) {
  const generatedId = useId()
  const inputId = `portal-combobox-${generatedId}`
  const descriptionId = description ? `${inputId}-description` : undefined
  const errorId = error ? `${inputId}-error` : undefined
  const selectedOption = options.find((option) => option.value === defaultValue)

  return (
    <PortalField className={className}>
      <PortalFieldLabel htmlFor={inputId}>{label}</PortalFieldLabel>
      {description ? (
        <PortalFieldDescription id={descriptionId}>{description}</PortalFieldDescription>
      ) : null}
      <Combobox.Root
        items={options}
        defaultValue={selectedOption}
        disabled={disabled}
        name={name}
        onValueChange={(nextValue) => {
          const option = Array.isArray(nextValue) ? nextValue[0] : nextValue
          onValueChange?.(option?.value ?? null)
        }}
      >
        <Combobox.InputGroup className="relative">
          <Combobox.Input
            id={inputId}
            aria-describedby={[descriptionId, errorId].filter(Boolean).join(" ") || undefined}
            aria-invalid={Boolean(error) || undefined}
            className={cn(portalControlClass, "pr-20")}
            placeholder={placeholder}
          />
          <div className="absolute inset-y-0 right-1 flex items-center">
            <Combobox.Clear
              className="flex h-9 w-9 items-center justify-center rounded-md text-black/35 transition hover:bg-[var(--portal-rail-hover-bg)] hover:text-[var(--portal-navy)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-gold)]/60"
              aria-label="Clear selection"
            >
              <X className="h-3.5 w-3.5" />
            </Combobox.Clear>
            <Combobox.Trigger
              className="flex h-9 w-9 items-center justify-center rounded-md text-[var(--portal-blue-gray)] transition hover:bg-[var(--portal-rail-hover-bg)] hover:text-[var(--portal-navy)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-gold)]/60"
              aria-label="Open options"
            >
              <ChevronDown className="h-4 w-4" />
            </Combobox.Trigger>
          </div>
        </Combobox.InputGroup>
        <Combobox.Portal>
          <Combobox.Positioner className="z-[70] outline-none" sideOffset={6} align="start">
            <Combobox.Popup className="max-h-[min(20rem,var(--available-height))] w-[max(var(--anchor-width),18rem)] overflow-hidden rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white/95 p-1.5 shadow-[var(--portal-panel-shadow)] backdrop-blur-xl outline-none">
              <Combobox.Empty className="px-3 py-4 text-sm font-light text-black/45">
                No matching options.
              </Combobox.Empty>
              <Combobox.List className="max-h-[min(18rem,var(--available-height))] overflow-y-auto overscroll-contain">
                {(option: PortalComboboxOption) => (
                  <Combobox.Item
                    key={option.value}
                    value={option}
                    className="group flex min-h-11 cursor-default items-center gap-3 rounded-md px-3 py-2 text-sm text-[var(--portal-navy-soft)] outline-none transition data-[highlighted]:bg-[var(--portal-blue-pale)] data-[selected]:text-[var(--portal-navy)]"
                  >
                    <span className="min-w-0 flex-1">
                      <span className="block font-medium">{option.label}</span>
                      {option.description ? (
                        <span className="mt-0.5 block truncate text-xs font-light text-black/45">
                          {option.description}
                        </span>
                      ) : null}
                    </span>
                    <Combobox.ItemIndicator className="text-[var(--portal-navy)]">
                      <Check className="h-4 w-4" />
                    </Combobox.ItemIndicator>
                  </Combobox.Item>
                )}
              </Combobox.List>
            </Combobox.Popup>
          </Combobox.Positioner>
        </Combobox.Portal>
      </Combobox.Root>
      {error ? <PortalFieldError id={errorId}>{error}</PortalFieldError> : null}
    </PortalField>
  )
}
