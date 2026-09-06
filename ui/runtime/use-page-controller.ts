"use client"

import { useSyncExternalStore } from 'react'
import type { PageStore } from './types'

/** React adapter for the transport/framework-neutral page runtime. */
export function usePageController<TModel>(controller: PageStore<TModel>): Readonly<TModel> {
  return useSyncExternalStore(
    controller.subscribe,
    controller.snapshot,
    controller.snapshot,
  )
}
