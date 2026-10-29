import { describe, it, expect } from 'vitest'
import i18next from 'i18next'
import en from './locales/en.yml'
import sk from './locales/sk.yml'
import { ENTRY_KINDS, ENTRY_NOT_COUNTED_REASONS, FORECAST_LINES, VALUE_SOURCES, type MetricKind, type NotCountedReason } from '../shared/types'

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

  it.each(FORECAST_LINES)('labels the %s forecast line', (line) => {
    expect(i18n.exists(`taxModels.forecast.line.${line}`)).toBe(true)
  })

  it.each(VALUE_SOURCES)('labels the %s value source', (source) => {
    expect(i18n.exists(`taxModels.forecast.source.${source}`)).toBe(true)
  })

  it.each(ENTRY_KINDS)('labels the %s entry kind', (kind) => {
    expect(i18n.exists(`taxModels.forecast.kind.${kind}`)).toBe(true)
  })

  it.each(ENTRY_NOT_COUNTED_REASONS)('explains the %s reason an entry is not counted', (reason) => {
    expect(i18n.exists(`taxModels.forecast.reason.${reason}`)).toBe(true)
  })

  it('has the tax forecast copy', () => {
    const keys = [
      'taxModels.forecast.column.soFar',
      'taxModels.forecast.column.rest',
      'taxModels.forecast.column.year',
      'taxModels.forecast.actualAndProjected',
      'taxModels.forecast.wholeYearActual',
      'taxModels.forecast.wholeYearProjected',
      'taxModels.forecast.corporateTaxAt',
      'taxModels.forecast.minimumTaxApplies',
      'taxModels.forecast.dividendFromYear',
      'taxModels.forecast.dividendTax',
      'taxModels.forecast.dividendTaxAt',
      'taxModels.forecast.profitLeftFrom',
      'taxModels.forecast.maximumDividend',
      'taxModels.updateFigures',
      'taxModels.figures.title',
      'taxModels.figures.actualsThrough',
      'taxModels.figures.noMonthsYet',
      'taxModels.figures.emptyHint',
      'taxModels.figures.payrollMonthly',
      'taxModels.figures.earlierProfitTitle',
      'taxModels.figures.earlierProfitRow',
      'taxModels.figures.year',
      'taxModels.figures.amount',
      'taxModels.figures.dividendTax',
      'taxModels.figures.addYear',
      'taxModels.figures.remove',
      'taxModels.figures.save',
      'taxModels.figures.cancel',
      'taxModels.figures.validation.amountInvalid',
      'taxModels.figures.validation.amountNegative',
      'taxModels.figures.validation.yearInvalid',
      'taxModels.figures.validation.rateInvalid',
      'taxModels.figures.validation.yearNotBefore',
      'taxModels.figures.validation.yearDuplicate',
      'taxModels.minimumTaxBandRow',
      'taxModels.minimumTax',
      'taxModels.minimumTaxUpTo',
      'taxModels.minimumTaxAmount',
      'taxModels.addMinimumTaxBand',
      'taxModels.removeMinimumTaxBand',
      'taxModels.validation.minimumTax.amountInvalid',
      'taxModels.validation.minimumTax.upperBoundRequired',
      'taxModels.validation.minimumTax.upperBoundInvalid',
      'taxModels.validation.minimumTax.upperBoundNotIncreasing',
      'taxModels.validation.minimumTax.lastUpperBoundNotAllowed',
    ]
    expect(keys.filter((key) => !i18n.exists(key))).toEqual([])
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

describe('forecast header', () => {
  const months = { from: 'January', to: 'September', restFrom: 'October', restTo: 'December' }

  it('names the actual and projected months in English', () => {
    expect(translator('en').t('taxModels.forecast.actualAndProjected', months)).toBe('Actual January to September, projected October to December')
  })

  it('names the actual and projected months in Slovak', () => {
    const slovak = { from: 'január', to: 'september', restFrom: 'október', restTo: 'december' }

    expect(translator('sk').t('taxModels.forecast.actualAndProjected', slovak)).toBe('Skutočnosť január až september, odhad október až december')
  })

  it('names a rate in the corporate tax row', () => {
    expect(translator('en').t('taxModels.forecast.corporateTaxAt', { rate: '21%' })).toBe('Corporate income tax at 21%')
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
