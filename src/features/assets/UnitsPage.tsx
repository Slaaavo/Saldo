import { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useQueryClient } from '@tanstack/react-query'
import type { FxRateRow, Currency } from '../../shared/types'
import { listCustomUnits, listFxRates, getConsolidationCurrency, setFxRateManual } from '../../shared/api'
import { extractErrorMessage } from '../../shared/utils/errors'
import { formatPrice, parsePriceAsRate } from './unitPricing'
import { cn } from '@/shared/lib/utils'

const UnitsPage = () => {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [units, setUnits] = useState<Currency[]>([])
  const [rates, setRates] = useState<FxRateRow[]>([])
  const [consolidationCurrency, setConsolidationCurrency] = useState<Currency | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [editingCell, setEditingCell] = useState<{ date: string; code: string } | null>(null)
  const [editValue, setEditValue] = useState('')

  const loadData = useCallback(
    () =>
      Promise.all([listCustomUnits(), listFxRates(), getConsolidationCurrency()])
        .then(([u, r, c]) => {
          setUnits(u)
          setRates(r)
          setConsolidationCurrency(c)
        })
        .catch((err) => setError(extractErrorMessage(err))),
    [],
  )

  useEffect(() => {
    void loadData()
  }, [loadData])

  // Only prices quoted in the current consolidation currency apply; rates from before a currency change stay in the database
  const unitCodes = new Set(units.map((u) => u.code))
  const unitRates = rates.filter((r) => r.fromCurrencyCode === consolidationCurrency?.code && unitCodes.has(r.toCurrencyCode))

  // Build pivot: dates × unit codes
  const dates = [...new Set(unitRates.map((r) => r.date))].sort((a, b) => b.localeCompare(a))

  // Key: `${date}:${toCurrencyCode}`
  const rateMap = new Map<string, FxRateRow>()
  for (const r of unitRates) {
    rateMap.set(`${r.date}:${r.toCurrencyCode}`, r)
  }

  const handleCellClick = (date: string, code: string, row: FxRateRow | undefined) => {
    setEditingCell({ date, code })
    setEditValue(row ? formatPrice(row) : '')
  }

  const handleCellSave = async (date: string, code: string, value: string) => {
    setEditingCell(null)
    const trimmed = value.trim()
    if (!trimmed) return
    setError(null)
    const parsed = parsePriceAsRate(trimmed)
    if (!parsed) {
      setError(t('units.invalidPrice'))
      return
    }
    const toId = units.find((u) => u.code === code)?.id
    if (!consolidationCurrency || toId === undefined) {
      setError(t('fxRates.currencyNotFound'))
      return
    }
    try {
      await setFxRateManual(consolidationCurrency.id, toId, date, parsed.mantissa, parsed.exponent, true)
      await loadData()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['fx-rates'] })
    } catch (err) {
      setError(extractErrorMessage(err))
    }
  }

  return (
    <div className="px-4 md:px-10 py-8">
      <div className="mb-6">{consolidationCurrency && <p className="text-sm text-muted-foreground">{t('units.subtitle', { currency: consolidationCurrency.name })}</p>}</div>

      {error && <div className="mb-4 rounded-md border border-destructive bg-destructive/10 px-4 py-2 text-sm text-destructive">{error}</div>}

      {units.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('units.empty')}</p>
      ) : dates.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('units.noPrices')}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm border-collapse">
            <thead>
              <tr className="border-b border-border">
                <th className="sticky left-0 z-[1] bg-card text-left py-2 pr-4 font-semibold text-muted-foreground">{t('fxRates.date')}</th>
                {units.map((u) => (
                  <th key={u.id} className="text-right py-2 px-3 font-semibold">
                    {u.code}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {dates.map((date) => (
                <tr key={date} className="group border-b border-border last:border-0 even:bg-muted/50 hover:bg-muted/30">
                  <td className="sticky left-0 z-[1] bg-card group-even:bg-muted/50 py-2 pr-4 text-muted-foreground font-mono text-sm">{date}</td>
                  {units.map((u) => {
                    const row = rateMap.get(`${date}:${u.code}`)
                    const isEditing = editingCell?.date === date && editingCell?.code === u.code
                    return (
                      <td
                        key={u.id}
                        className={cn('text-right py-1 px-3 font-mono text-sm cursor-pointer', row?.isManual && 'font-bold bg-amber-50 dark:bg-amber-900/20')}
                        onClick={() => {
                          if (!isEditing) handleCellClick(date, u.code, row)
                        }}
                      >
                        {isEditing ? (
                          <input
                            autoFocus
                            type="text"
                            value={editValue}
                            onChange={(e) => setEditValue(e.target.value)}
                            onBlur={(e) => handleCellSave(date, u.code, e.target.value)}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') {
                                e.preventDefault()
                                handleCellSave(date, u.code, editValue)
                              } else if (e.key === 'Escape') {
                                setEditingCell(null)
                              }
                            }}
                            className="w-24 text-right font-mono text-sm border border-border rounded px-1 bg-background"
                          />
                        ) : (
                          <>
                            {row ? formatPrice(row) : '—'}
                            {row?.isManual && <span className="ml-1 text-amber-600 text-xs">({t('fxRates.isManual')})</span>}
                          </>
                        )}
                      </td>
                    )
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}

export default UnitsPage
