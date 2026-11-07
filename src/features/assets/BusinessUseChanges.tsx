import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { BusinessUseChange } from '../../shared/types'
import { bpsToPct, pctToBps } from '../../shared/utils/format'
import { Button } from '../../shared/ui/button'
import { Input } from '../../shared/ui/input'
import { Label } from '../../shared/ui/label'
import { PercentageInput } from '../../shared/ui/PercentageInput'

interface Props {
  idPrefix: string
  atPurchasePct: string
  changes: BusinessUseChange[]
  onChange: (changes: BusinessUseChange[]) => void
}

const monthLabel = (fromMonth: string, language: string) => {
  const [year, month] = fromMonth.split('-').map(Number)
  return new Date(year, month - 1, 1).toLocaleDateString(language, { year: 'numeric', month: 'long' })
}

const BusinessUseChanges = ({ idPrefix, atPurchasePct, changes, onChange }: Props) => {
  const { t, i18n } = useTranslation()
  const [adding, setAdding] = useState(false)
  const [fromMonth, setFromMonth] = useState('')
  const [pct, setPct] = useState('')

  const sorted = [...changes].sort((a, b) => a.fromMonth.localeCompare(b.fromMonth))
  const businessUsePctBps = pctToBps(pct)
  const monthIsValid = /^\d{4}-(0[1-9]|1[0-2])$/.test(fromMonth.trim())
  const monthIsTaken = changes.some((change) => change.fromMonth === fromMonth.trim())
  const canAdd = monthIsValid && !monthIsTaken && businessUsePctBps !== null

  const handleAdd = () => {
    if (businessUsePctBps === null) return
    onChange([...changes, { fromMonth: fromMonth.trim(), businessUsePctBps }])
    setAdding(false)
    setFromMonth('')
    setPct('')
  }

  return (
    <div className="flex flex-col gap-2">
      <Label>{t('modals.assetDepreciation.businessUseHistory')}</Label>
      <ul className="flex flex-col gap-1 text-sm">
        <li>{t('modals.assetDepreciation.businessUseFromPurchase', { pct: atPurchasePct })}</li>
        {sorted.map((change) => (
          <li key={change.fromMonth} className="flex items-center justify-between gap-2">
            <span>
              {t('modals.assetDepreciation.businessUseFromMonth', {
                pct: bpsToPct(change.businessUsePctBps),
                month: monthLabel(change.fromMonth, i18n.language),
              })}
            </span>
            <Button type="button" variant="ghost" size="sm" onClick={() => onChange(changes.filter((other) => other.fromMonth !== change.fromMonth))}>
              {t('modals.assetDepreciation.removeChange')}
            </Button>
          </li>
        ))}
      </ul>
      {adding ? (
        <div className="flex items-end gap-2">
          <div className="flex flex-1 flex-col gap-2">
            <Label htmlFor={`${idPrefix}-change-month`}>{t('modals.assetDepreciation.changeMonth')}</Label>
            <Input id={`${idPrefix}-change-month`} type="text" value={fromMonth} onChange={(e) => setFromMonth(e.target.value)} placeholder="2026-07" />
          </div>
          <div className="flex flex-1 flex-col gap-2">
            <Label htmlFor={`${idPrefix}-change-pct`}>{t('modals.assetDepreciation.businessUse')}</Label>
            <PercentageInput id={`${idPrefix}-change-pct`} type="number" step="0.01" min={0} max={100} value={pct} onChange={(e) => setPct(e.target.value)} />
          </div>
          <Button type="button" variant="outline" disabled={!canAdd} onClick={handleAdd}>
            {t('modals.assetDepreciation.addChange')}
          </Button>
        </div>
      ) : (
        <div>
          <Button type="button" variant="outline" size="sm" onClick={() => setAdding(true)}>
            {t('modals.assetDepreciation.changeFromMonth')}
          </Button>
        </div>
      )}
    </div>
  )
}

export default BusinessUseChanges
