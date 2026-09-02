export function syndicationLiveEnabled(): boolean {
  return process.env.SYNDICATION_LIVE === 'true'
}

export function readEnv(name: string): string | null {
  const value = process.env[name]?.trim()
  return value ? value : null
}

export function missingEnv(names: string[]): string[] {
  return names.filter((name) => !readEnv(name))
}

/** Page /feed is required for live. Catalog id is optional. Never return secrets. */
export function facebookReadiness() {
  const hasToken = Boolean(readEnv('META_ACCESS_TOKEN'))
  const hasPageId = Boolean(readEnv('META_PAGE_ID'))
  const hasCatalogId = Boolean(readEnv('META_PRODUCT_CATALOG_ID'))
  const liveEnabled = syndicationLiveEnabled()
  const requiredMissing = missingEnv(['META_ACCESS_TOKEN', 'META_PAGE_ID'])
  return {
    liveEnabled,
    hasToken,
    hasPageId,
    hasCatalogId,
    requiredMissing,
    readyToPost: liveEnabled && requiredMissing.length === 0,
  }
}

export type FacebookReadiness = ReturnType<typeof facebookReadiness>
