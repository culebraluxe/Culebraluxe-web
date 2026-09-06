import type { ClientRelationshipChannel } from '@/lib/portal/types'
import type {
  ClientLensChannelModel,
  ClientLensChannelSlot,
} from './model'

type SlotDefinition = {
  slot: ClientLensChannelSlot
  label: string
  matches(channel: ClientRelationshipChannel): boolean
}

function sourceIncludes(channel: ClientRelationshipChannel, token: string): boolean {
  return channel.source.toLowerCase().includes(token)
}

const SLOT_DEFINITIONS: readonly SlotDefinition[] = [
  {
    slot: 'phone',
    label: 'Phone',
    matches: (channel) =>
      (channel.channel === 'call' || sourceIncludes(channel, 'phone') || sourceIncludes(channel, 'call')) &&
      !sourceIncludes(channel, 'facetime'),
  },
  {
    slot: 'imessage',
    label: 'iMessage',
    matches: (channel) =>
      channel.channel === 'imessage' || sourceIncludes(channel, 'apple_messages'),
  },
  {
    slot: 'whatsapp',
    label: 'WhatsApp',
    matches: (channel) =>
      channel.channel === 'whatsapp' || sourceIncludes(channel, 'whatsapp'),
  },
  {
    slot: 'gmail',
    label: 'Email',
    matches: (channel) =>
      sourceIncludes(channel, 'gmail') ||
      (channel.channel === 'email' && !sourceIncludes(channel, 'calendar')),
  },
  {
    slot: 'facetime',
    label: 'FaceTime',
    matches: (channel) =>
      channel.channel === 'facetime' || sourceIncludes(channel, 'facetime'),
  },
  {
    slot: 'calendar',
    label: 'Apple Calendar',
    matches: (channel) =>
      channel.channel === 'calendar' ||
      sourceIncludes(channel, 'calendar') ||
      sourceIncludes(channel, 'eventkit'),
  },
]

export function projectClientLensChannels(
  channels: readonly ClientRelationshipChannel[],
): ClientLensChannelModel[] {
  return SLOT_DEFINITIONS.map((definition) => {
    const channel = channels.find(definition.matches)
    return {
      slot: definition.slot,
      label: definition.label,
      connected: Boolean(channel),
      source: channel?.source ?? null,
      channel: channel?.channel ?? null,
      firstObservedAt: channel?.firstObservedAt ?? null,
      lastContactAt: channel?.lastContactAt ?? null,
      lastInboundAt: channel?.lastInboundAt ?? null,
      lastOutboundAt: channel?.lastOutboundAt ?? null,
      inboundCount: channel?.inboundCount ?? 0,
      outboundCount: channel?.outboundCount ?? 0,
      totalCount: channel?.totalCount ?? 0,
      twoWay: channel?.twoWay ?? false,
      lastContext: channel?.lastContext ?? null,
      lastContextAt: channel?.lastContextAt ?? null,
      lastContextDirection: channel?.lastContextDirection ?? null,
    }
  })
}
