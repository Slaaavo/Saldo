import { useTranslation } from 'react-i18next'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'
import { Input } from '../../shared/ui/input'
import { PercentageInput } from '../../shared/ui/PercentageInput'
import { Label } from '../../shared/ui/label'
import { Checkbox } from '../../shared/ui/checkbox'
import { Button } from '../../shared/ui/button'
import NumberValue from '../../shared/ui/NumberValue'
import { toMinorUnits, todayIso, getMinorUnitsStep } from '../../shared/utils/format'
import { Trash2 } from 'lucide-react'
import { SplitLegDraft, makeEmptyLeg } from './splitLegDraft'

const VAT_QUICK_FILLS = ['0', '5', '10', '20', '23']
const VAT_DEDUCTIBLE_QUICK_FILLS = ['100', '50']
const EXPENSE_DEDUCTIBLE_QUICK_FILLS = ['100', '80', '50']

interface Props {
  eventType: 'revenue' | 'expense'
  currencyCode: string
  currencyMinorUnits: number
  legs: SplitLegDraft[]
  onLegsChange: (legs: SplitLegDraft[]) => void
  groupNote: string
  onGroupNoteChange: (note: string) => void
}

const TaxableEventSplitEditor = ({ eventType, currencyCode, currencyMinorUnits, legs, onLegsChange, groupNote, onGroupNoteChange }: Props) => {
  const { t } = useTranslation()

  const updateLeg = (index: number, patch: Partial<SplitLegDraft>) => {
    onLegsChange(legs.map((l, i) => (i === index ? { ...l, ...patch } : l)))
  }

  const removeLeg = (index: number) => {
    onLegsChange(legs.filter((_, i) => i !== index))
  }

  const addLeg = () => {
    const date = legs[0]?.eventDate ?? todayIso()
    onLegsChange([...legs, makeEmptyLeg(date)])
  }

  const total = legs.reduce((sum, leg) => {
    const trimmed = leg.amount.trim()
    if (!trimmed) return sum
    const parsed = parseFloat(trimmed)
    return sum + (isNaN(parsed) ? 0 : toMinorUnits(trimmed, currencyMinorUnits))
  }, 0)

  return (
    <div className="flex flex-col gap-4">
      {legs.map((leg, index) => (
        <div key={index} className="flex flex-col gap-2 p-3 rounded-md border border-border bg-muted/30">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium">{t('modals.editTaxableSplitGroup.legHeader', { n: index + 1 })}</span>
            <Button type="button" variant="ghost" size="sm" disabled={legs.length <= 2} onClick={() => removeLeg(index)}>
              <Trash2 className="h-4 w-4 mr-1" />
              {t('modals.createRevenue.removeLeg')}
            </Button>
          </div>

          {/* Amount */}
          <div className="flex flex-col gap-1">
            <Label className="text-xs">{t('modals.createRevenue.amount')}</Label>
            <CurrencyInput
              type="number"
              step={getMinorUnitsStep(currencyMinorUnits)}
              value={leg.amount}
              onChange={(e) => updateLeg(index, { amount: e.target.value })}
              placeholder="0"
              currencyCode={currencyCode}
              required
            />
          </div>

          {/* VAT rate */}
          <div className="flex flex-col gap-1">
            <Label className="text-xs">{t('modals.createRevenue.vatRate')}</Label>
            <PercentageInput
              type="number"
              step="0.01"
              min={0}
              max={100}
              value={leg.vatRate}
              onChange={(e) => updateLeg(index, { vatRate: e.target.value })}
              placeholder={t('modals.createRevenue.vatRatePlaceholder')}
            />
            <div className="flex gap-1 flex-wrap">
              {VAT_QUICK_FILLS.map((v) => (
                <button key={v} type="button" onClick={() => updateLeg(index, { vatRate: v })} className="text-xs px-2 py-0.5 rounded bg-muted hover:bg-accent transition-colors">
                  {v}%
                </button>
              ))}
            </div>
          </div>

          {/* Expense-only fields */}
          {eventType === 'expense' && (
            <>
              <div className="flex flex-col gap-1">
                <Label className="text-xs">{t('modals.createExpense.vatReclaimablePct')}</Label>
                <PercentageInput
                  type="number"
                  step="0.01"
                  min={0}
                  max={100}
                  value={leg.vatReclaimablePct}
                  onChange={(e) => updateLeg(index, { vatReclaimablePct: e.target.value })}
                  placeholder={t('modals.createExpense.vatReclaimablePctPlaceholder')}
                />
                <div className="flex gap-1 flex-wrap">
                  {VAT_DEDUCTIBLE_QUICK_FILLS.map((v) => (
                    <button
                      key={v}
                      type="button"
                      onClick={() => updateLeg(index, { vatReclaimablePct: v })}
                      className="text-xs px-2 py-0.5 rounded bg-muted hover:bg-accent transition-colors"
                    >
                      {v}%
                    </button>
                  ))}
                </div>
              </div>

              <div className="flex flex-col gap-1">
                <Label className="text-xs">{t('modals.createExpense.expenseDeductiblePct')}</Label>
                <PercentageInput
                  type="number"
                  step="0.01"
                  min={0}
                  max={100}
                  value={leg.expenseDeductiblePct}
                  onChange={(e) => updateLeg(index, { expenseDeductiblePct: e.target.value })}
                  placeholder={t('modals.createExpense.expenseDeductiblePctPlaceholder')}
                />
                <div className="flex gap-1 flex-wrap">
                  {EXPENSE_DEDUCTIBLE_QUICK_FILLS.map((v) => (
                    <button
                      key={v}
                      type="button"
                      onClick={() => updateLeg(index, { expenseDeductiblePct: v })}
                      className="text-xs px-2 py-0.5 rounded bg-muted hover:bg-accent transition-colors"
                    >
                      {v}%
                    </button>
                  ))}
                </div>
              </div>

              {/* VAT reclaimed */}
              <div className="flex items-center gap-2">
                <Checkbox id={`leg-reclaimed-vat-${index}`} checked={leg.reclaimedVat === true} onCheckedChange={(v) => updateLeg(index, { reclaimedVat: v === true })} />
                <Label htmlFor={`leg-reclaimed-vat-${index}`} className="text-xs">
                  {t('modals.createExpense.reclaimedVat')}
                </Label>
              </div>
            </>
          )}

          {/* Note */}
          <div className="flex flex-col gap-1">
            <Label className="text-xs">{t('modals.createRevenue.note')}</Label>
            <Input type="text" value={leg.note} onChange={(e) => updateLeg(index, { note: e.target.value })} placeholder={t('modals.createRevenue.notePlaceholder')} />
          </div>
        </div>
      ))}

      {/* Add leg */}
      <Button type="button" variant="outline" size="sm" onClick={addLeg} className="self-start">
        {t('modals.createRevenue.addLeg')}
      </Button>

      {/* Group note */}
      <div className="flex flex-col gap-1">
        <Label className="text-sm">{t('modals.createRevenue.groupNote')}</Label>
        <Input type="text" value={groupNote} onChange={(e) => onGroupNoteChange(e.target.value)} placeholder={t('modals.createRevenue.groupNote')} />
      </div>

      {/* Running total */}
      <div className="flex items-center justify-between py-2 px-3 rounded-md bg-muted text-sm">
        <span className="text-muted-foreground">{t('modals.createRevenue.total')}</span>
        <NumberValue value={total} currencyCode={currencyCode} minorUnits={currencyMinorUnits} className="font-semibold" />
      </div>
    </div>
  )
}

export default TaxableEventSplitEditor
