import { useTranslation } from 'react-i18next'
import { Trash2, ChevronUp, ChevronDown, Plus, Loader2 } from 'lucide-react'
import type { EkasaRuleDraft } from './types'
import { Button } from '../../shared/ui/button'
import { Input } from '../../shared/ui/input'
import { PercentageInput } from '../../shared/ui/PercentageInput'
import { Checkbox } from '../../shared/ui/checkbox'
import { DialogFooter } from '../../shared/ui/dialog'

interface RulesStepProps {
  rules: EkasaRuleDraft[]
  defaultDeductiblePct: string
  defaultVatReclaimablePct: string
  isProcessing: boolean
  onRulesChange: (rules: EkasaRuleDraft[]) => void
  onDefaultDeductiblePctChange: (value: string) => void
  onDefaultVatReclaimablePctChange: (value: string) => void
  onConfirm: () => void | Promise<void>
  onBack: () => void
}

const makeEmptyRule = (): EkasaRuleDraft => ({ id: crypto.randomUUID(), namePattern: '', deductiblePct: '100', vatReclaimablePct: '100', privateUse: false })

const ROW_GRID = 'grid grid-cols-[1fr_7rem_7rem_5rem_2.5rem_2.5rem_2.5rem] gap-2'

const RulesStep = ({
  rules,
  defaultDeductiblePct,
  defaultVatReclaimablePct,
  isProcessing,
  onRulesChange,
  onDefaultDeductiblePctChange,
  onDefaultVatReclaimablePctChange,
  onConfirm,
  onBack,
}: RulesStepProps) => {
  const { t } = useTranslation()

  const handleAddRule = () => {
    onRulesChange([makeEmptyRule(), ...rules])
  }

  const handleDelete = (index: number) => {
    onRulesChange(rules.filter((_, i) => i !== index))
  }

  const handleMoveUp = (index: number) => {
    if (index === 0) return
    const next = [...rules]
    ;[next[index - 1], next[index]] = [next[index], next[index - 1]]
    onRulesChange(next)
  }

  const handleMoveDown = (index: number) => {
    if (index === rules.length - 1) return
    const next = [...rules]
    ;[next[index], next[index + 1]] = [next[index + 1], next[index]]
    onRulesChange(next)
  }

  const handleRuleChange = (index: number, change: Partial<EkasaRuleDraft>) => {
    const next = [...rules]
    next[index] = { ...next[index], ...change }
    onRulesChange(next)
  }

  return (
    <div className="flex flex-col gap-4">
      <Button type="button" variant="outline" size="sm" className="self-start" onClick={handleAddRule}>
        <Plus className="mr-1 h-4 w-4" />
        {t('ekasaImport.rulesStep.addRule')}
      </Button>

      <div className="flex flex-col gap-2">
        {/* Column headers */}
        <div className={`${ROW_GRID} px-1`}>
          <span className="text-xs font-medium text-muted-foreground">{t('ekasaImport.rulesStep.namePatternHeader')}</span>
          <span className="text-xs font-medium text-muted-foreground">{t('ekasaImport.rulesStep.deductiblePctHeader')}</span>
          <span className="text-xs font-medium text-muted-foreground">{t('ekasaImport.rulesStep.vatReclaimablePctHeader')}</span>
          <span className="text-xs font-medium text-muted-foreground">{t('ekasaImport.rulesStep.privateUseHeader')}</span>
        </div>

        {/* Rule rows */}
        {rules.map((rule, index) => (
          <div key={rule.id} className={`${ROW_GRID} items-center`}>
            <Input
              type="text"
              placeholder={t('ekasaImport.rulesStep.namePatternPlaceholder')}
              value={rule.namePattern}
              onChange={(e) => handleRuleChange(index, { namePattern: e.target.value })}
            />
            <PercentageInput
              type="number"
              min="0"
              max="100"
              step="1"
              value={rule.deductiblePct}
              onChange={(e) => handleRuleChange(index, { deductiblePct: e.target.value })}
              aria-label={t('ekasaImport.rulesStep.deductiblePctHeader')}
            />
            <PercentageInput
              type="number"
              min="0"
              max="100"
              step="1"
              value={rule.vatReclaimablePct}
              onChange={(e) => handleRuleChange(index, { vatReclaimablePct: e.target.value })}
              aria-label={t('ekasaImport.rulesStep.vatReclaimablePctHeader')}
            />
            <Checkbox
              checked={rule.privateUse}
              onCheckedChange={(v) => handleRuleChange(index, { privateUse: v === true })}
              aria-label={t('ekasaImport.rulesStep.privateUseHeader')}
            />
            <Button type="button" variant="ghost" size="icon" disabled={index === 0} onClick={() => handleMoveUp(index)} aria-label={t('ekasaImport.rulesStep.moveUp')}>
              <ChevronUp className="h-4 w-4" />
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              disabled={index === rules.length - 1}
              onClick={() => handleMoveDown(index)}
              aria-label={t('ekasaImport.rulesStep.moveDown')}
            >
              <ChevronDown className="h-4 w-4" />
            </Button>
            <Button type="button" variant="ghost" size="icon" onClick={() => handleDelete(index)} aria-label={t('ekasaImport.rulesStep.deleteRule')}>
              <Trash2 className="h-4 w-4" />
            </Button>
          </div>
        ))}

        {/* Default row — always present, cannot be deleted */}
        <div className={`${ROW_GRID} items-center border-t pt-2`}>
          <span className="text-sm font-medium">{t('ekasaImport.rulesStep.default')}</span>
          <PercentageInput
            type="number"
            min="0"
            max="100"
            step="1"
            value={defaultDeductiblePct}
            onChange={(e) => onDefaultDeductiblePctChange(e.target.value)}
            aria-label={t('ekasaImport.rulesStep.deductiblePctHeader')}
          />
          <PercentageInput
            type="number"
            min="0"
            max="100"
            step="1"
            value={defaultVatReclaimablePct}
            onChange={(e) => onDefaultVatReclaimablePctChange(e.target.value)}
            aria-label={t('ekasaImport.rulesStep.vatReclaimablePctHeader')}
          />
        </div>
      </div>

      <DialogFooter>
        <Button type="button" variant="outline" onClick={onBack}>
          {t('ekasaImport.rulesStep.back')}
        </Button>
        <Button type="button" onClick={onConfirm} disabled={isProcessing}>
          {isProcessing && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
          {t('ekasaImport.rulesStep.next')}
        </Button>
      </DialogFooter>
    </div>
  )
}

export default RulesStep
