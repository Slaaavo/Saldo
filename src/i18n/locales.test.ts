import { describe, it, expect } from 'vitest'
import i18next from 'i18next'
import en from './locales/en.yml'
import sk from './locales/sk.yml'
import type { MetricKind, NotCountedReason } from '../shared/types'

const METRIC_KINDS = { netWorth: true, liquid: true, totalBalance: true, leftToSpend: true } satisfies Record<MetricKind, true>
const NOT_COUNTED_REASONS = { noRate: true, linkedToAsset: true, earmark: true, notLiquid: true } satisfies Record<NotCountedReason, true>

const translator = (lng: 'en' | 'sk') => {
  const instance = i18next.createInstance()
  void instance.init({ resources: { en: { translation: en }, sk: { translation: sk } }, lng, fallbackLng: false, interpolation: { escapeValue: false } })
  return instance
}

describe.each(['en', 'sk'] as const)('%s locale', (lng) => {
  const i18n = translator(lng)

  it.each(Object.keys(METRIC_KINDS))('labels the %s metric', (kind) => {
    expect(i18n.exists(`metrics.${kind}`)).toBe(true)
  })

  it.each(Object.keys(NOT_COUNTED_REASONS))('explains the %s reason in words', (reason) => {
    expect(i18n.exists(`metrics.breakdown.reason.${reason}`)).toBe(true)
  })

  it('has the time machine, rate warning and breakdown copy', () => {
    const keys = [
      'timeMachine.live',
      'timeMachine.viewing',
      'timeMachine.backToLive',
      'metrics.openBreakdown',
      'metrics.breakdown.description',
      'metrics.breakdown.counted',
      'metrics.breakdown.notCounted',
      'metrics.breakdown.total',
      'metrics.breakdown.nothingCounted',
      'metrics.breakdown.noConversion',
      'metrics.breakdown.manual',
      'metrics.breakdown.columns.account',
      'metrics.breakdown.columns.amount',
      'metrics.breakdown.columns.rate',
      'metrics.breakdown.columns.value',
      'metrics.breakdown.part.linkedAccount',
      'metrics.breakdown.part.taggedCashflows',
      'metrics.breakdown.part.bucketTotal',
      'rateWarnings.noRate',
      'accounts.noRateTooltip',
      'buckets.notCounted',
    ]
    expect(keys.filter((key) => !i18n.exists(key))).toEqual([])
  })
})

describe('plural forms of an old rate', () => {
  it('counts days in English', () => {
    const i18n = translator('en')
    const warning = { currency: 'USD', date: '12 October 2026' }

    expect(i18n.t('rateWarnings.stale', { ...warning, count: 1 })).toBe('uses a USD rate from 12 October 2026, 1 day old.')
    expect(i18n.t('rateWarnings.stale', { ...warning, count: 13 })).toBe('uses a USD rate from 12 October 2026, 13 days old.')
    expect(i18n.t('metrics.breakdown.rateAge', { count: 8 })).toBe('8 days old')
    expect(i18n.t('accounts.staleRateTooltip', { date: '12 October 2026', count: 1 })).toBe('Rate from 12 October 2026, 1 day old')
  })

  it('counts days in Slovak with one, few and many forms', () => {
    const i18n = translator('sk')

    expect(i18n.t('metrics.breakdown.rateAge', { count: 1 })).toBe('starý 1 deň')
    expect(i18n.t('metrics.breakdown.rateAge', { count: 3 })).toBe('starý 3 dni')
    expect(i18n.t('metrics.breakdown.rateAge', { count: 13 })).toBe('starý 13 dní')
    expect(i18n.t('rateWarnings.stale', { currency: 'USD', date: '12. októbra 2026', count: 9 })).toBe('používa kurz USD z 12. októbra 2026, starý 9 dní.')
    expect(i18n.t('accounts.staleRateTooltip', { date: '12. októbra 2026', count: 2 })).toBe('Kurz z 12. októbra 2026, starý 2 dni')
  })
})
