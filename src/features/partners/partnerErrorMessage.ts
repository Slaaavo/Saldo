import type { TFunction } from 'i18next'
import { extractErrorMessage } from '../../shared/utils/errors'

export const partnerErrorMessage = (t: TFunction, err: unknown, fallbackKey: 'errors.createPartner' | 'errors.updatePartner'): string => {
  const msg = extractErrorMessage(err)
  if (msg.includes('DUPLICATE_IBAN') || msg.toLowerCase().includes('already in use')) return t('partners.errors.duplicateIban')
  if (msg.includes('15') || msg.toLowerCase().includes('alphanumeric')) return t('partners.errors.invalidIban')
  return t(fallbackKey, { error: msg })
}
