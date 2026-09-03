export type LoopIntent = 'repair' | 'grow' | 'stop' | 'unknown'

export type PacketLoop = {
  intent: LoopIntent
  parentRun: string | null
  failedCommands: string[]
  loopLabel: string | null
  raw: string
}

export function parsePacketLoop(raw: string | null | undefined): PacketLoop {
  const text = raw?.trim() ?? ''
  const intentMatch = text.match(/\bintent:\s*(repair|grow|stop)\b/i)
  const parentMatch = text.match(/\bparent_run:\s*(\S+)/i)
  const loopMatch = text.match(/\bloop:\s*(\S+)/i)
  const failed: string[] = []
  const failBlock = text.split(/failed_commands:\s*/i)[1]
  if (failBlock) {
    for (const line of failBlock.split(/\n/)) {
      if (/^(intent|parent_run|loop):/i.test(line.trim())) break
      const cmd = line.replace(/^[-*]\s*/, '').trim()
      if (cmd) failed.push(cmd)
    }
  }
  return {
    intent: (intentMatch?.[1]?.toLowerCase() as LoopIntent) ?? (text ? 'unknown' : 'unknown'),
    parentRun: parentMatch?.[1] ?? null,
    failedCommands: failed,
    loopLabel: loopMatch?.[1] ?? null,
    raw: text,
  }
}
