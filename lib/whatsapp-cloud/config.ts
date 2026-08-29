import { normalizePhone } from '../crm-intake-normalization'

import type { MetaWhatsAppConfiguration } from './types'

function required(env: NodeJS.ProcessEnv, key: string): string {
  const value = env[key]?.trim()
  if (!value) throw new Error(`${key} is not configured.`)
  return value
}

export function loadWhatsAppVerifyToken(
  env: NodeJS.ProcessEnv = process.env,
): string {
  return required(env, 'WHATSAPP_VERIFY_TOKEN')
}

export function loadMetaWhatsAppConfiguration(
  env: NodeJS.ProcessEnv = process.env,
): MetaWhatsAppConfiguration {
  return {
    appSecret: required(env, 'WHATSAPP_APP_SECRET'),
    phoneNumberId: required(env, 'WHATSAPP_PHONE_NUMBER_ID'),
    ownedPhoneE164: normalizePhone(
      required(env, 'WHATSAPP_OWNED_PHONE_E164'),
    ),
  }
}
