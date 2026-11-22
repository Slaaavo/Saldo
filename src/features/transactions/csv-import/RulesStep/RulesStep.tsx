import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Trash2, ChevronUp, ChevronDown } from 'lucide-react'
import type { ImportRule } from '../../../../shared/types'
import { Button } from '../../../../shared/ui/button'
import { DialogFooter } from '../../../../shared/ui/dialog'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../../../shared/ui/select'
import SignFromColumnForm from './SignFromColumnForm'
import OverrideDateFromDescriptionForm from './OverrideDateFromDescriptionForm'
import ColumnPatternForm from './ColumnPatternForm'

const ADD_NONE = '__none__'

type RuleType = ImportRule['type']

const RULE_LABEL_KEYS: Record<RuleType, string> = {
  sign_from_column: 'import.rules.signFromColumnLabel',
  override_date_from_description: 'import.rules.overrideDateFromDescriptionLabel',
  pending_from_column: 'import.rules.pendingFromColumnLabel',
  declined_from_column: 'import.rules.declinedFromColumnLabel',
}

const RULE_TYPES = Object.keys(RULE_LABEL_KEYS) as RuleType[]

const emptyRule = (type: RuleType, sortOrder: number): ImportRule => {
  switch (type) {
    case 'sign_from_column':
      return { type, sortOrder, typeColumn: '', negativeType: '' }
    case 'override_date_from_description':
      return { type, sortOrder, descriptionColumn: '', conditionRegex: '', dateRegex: '' }
    case 'pending_from_column':
    case 'declined_from_column':
      return { type, sortOrder, column: '', pattern: '' }
  }
}

const isInvalidRegex = (pattern: string): boolean => {
  try {
    new RegExp(pattern)
    return false
  } catch {
    return true
  }
}

interface ValidationErrors {
  [ruleIndex: number]: {
    conditionRegex?: string
    dateRegex?: string
    pattern?: string
  }
}

interface RulesStepProps {
  rules: ImportRule[]
  csvHeaders: string[]
  onRulesChange: (rules: ImportRule[]) => void
  onNext: () => void
  onBack: () => void
  onCancel: () => void
}

const RulesStep = ({ rules, csvHeaders, onRulesChange, onNext, onBack, onCancel }: RulesStepProps) => {
  const { t } = useTranslation()
  const [addType, setAddType] = useState<string>(ADD_NONE)
  const [validationErrors, setValidationErrors] = useState<ValidationErrors>({})

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

  const handleRuleChange = (index: number, updated: ImportRule) => {
    const next = [...rules]
    next[index] = updated
    onRulesChange(next)
    // clear errors for this index when rule changes
    if (validationErrors[index]) {
      setValidationErrors((prev) => {
        const copy = { ...prev }
        delete copy[index]
        return copy
      })
    }
  }

  const handleAdd = () => {
    if (addType === ADD_NONE) return
    onRulesChange([...rules, emptyRule(addType as RuleType, rules.length)])
  }

  const handleNext = () => {
    const errors: ValidationErrors = {}

    rules.forEach((rule, i) => {
      if (rule.type === 'override_date_from_description') {
        const ruleErrors: { conditionRegex?: string; dateRegex?: string } = {}
        if (rule.conditionRegex !== '' && isInvalidRegex(rule.conditionRegex)) {
          ruleErrors.conditionRegex = t('import.rules.invalidRegex')
        }
        if (isInvalidRegex(rule.dateRegex)) {
          ruleErrors.dateRegex = t('import.rules.invalidRegex')
        }
        if (Object.keys(ruleErrors).length > 0) {
          errors[i] = ruleErrors
        }
      } else if ((rule.type === 'pending_from_column' || rule.type === 'declined_from_column') && isInvalidRegex(rule.pattern)) {
        errors[i] = { pattern: t('import.rules.invalidRegex') }
      }
    })

    if (Object.keys(errors).length > 0) {
      setValidationErrors(errors)
      return
    }
    setValidationErrors({})
    onNext()
  }

  return (
    <>
      <div className="flex flex-col gap-4">
        <div>
          <p className="text-sm font-medium">{t('import.rulesStep.title')}</p>
          <p className="text-sm text-muted-foreground">{t('import.rulesStep.description')}</p>
        </div>

        {rules.length === 0 ? (
          <p className="text-sm text-muted-foreground italic">{t('import.rulesStep.empty')}</p>
        ) : (
          <ol className="flex flex-col gap-3">
            {rules.map((rule, index) => (
              <li key={index} className="flex flex-col gap-2 rounded-[var(--radius)] border p-3">
                <div className="flex items-center justify-between gap-2">
                  <span className="text-xs font-semibold text-muted-foreground">
                    {index + 1}. {t(RULE_LABEL_KEYS[rule.type])}
                  </span>
                  <div className="flex items-center gap-1">
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7"
                      title={t('common.moveUp')}
                      aria-label={t('common.moveUp')}
                      onClick={() => handleMoveUp(index)}
                      disabled={index === 0}
                    >
                      <ChevronUp className="h-4 w-4" />
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7"
                      title={t('common.moveDown')}
                      aria-label={t('common.moveDown')}
                      onClick={() => handleMoveDown(index)}
                      disabled={index === rules.length - 1}
                    >
                      <ChevronDown className="h-4 w-4" />
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7 text-destructive hover:text-destructive"
                      title={t('common.delete')}
                      aria-label={t('common.delete')}
                      onClick={() => handleDelete(index)}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                </div>

                {rule.type === 'sign_from_column' && (
                  <SignFromColumnForm rule={rule} csvHeaders={csvHeaders} onChange={(updated: ImportRule) => handleRuleChange(index, updated)} />
                )}
                {rule.type === 'override_date_from_description' && (
                  <OverrideDateFromDescriptionForm
                    rule={rule}
                    csvHeaders={csvHeaders}
                    onChange={(updated: ImportRule) => handleRuleChange(index, updated)}
                    errors={validationErrors[index]}
                  />
                )}
                {(rule.type === 'pending_from_column' || rule.type === 'declined_from_column') && (
                  <ColumnPatternForm
                    rule={rule}
                    csvHeaders={csvHeaders}
                    onChange={(updated: ImportRule) => handleRuleChange(index, updated)}
                    error={validationErrors[index]?.pattern}
                  />
                )}
              </li>
            ))}
          </ol>
        )}

        <div className="flex items-center gap-2">
          <Select value={addType} onValueChange={setAddType}>
            <SelectTrigger className="w-64">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={ADD_NONE}>{t('import.rulesStep.selectType')}</SelectItem>
              {RULE_TYPES.map((type) => (
                <SelectItem key={type} value={type}>
                  {t(RULE_LABEL_KEYS[type])}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button type="button" variant="outline" size="sm" onClick={handleAdd} disabled={addType === ADD_NONE}>
            {t('import.rulesStep.add')}
          </Button>
        </div>
      </div>

      <DialogFooter>
        <Button type="button" variant="outline" onClick={onBack}>
          {t('import.back')}
        </Button>
        <Button type="button" variant="outline" onClick={onCancel}>
          {t('modals.confirm.cancel')}
        </Button>
        <Button type="button" onClick={handleNext}>
          {t('import.next')}
        </Button>
      </DialogFooter>
    </>
  )
}

export default RulesStep
