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
