import { useTranslation } from 'react-i18next'
import type { ImportRule } from '../../../../shared/types'
import { Input } from '../../../../shared/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../../../shared/ui/select'

interface ColumnPatternFormProps {
  rule: ImportRule & { type: 'pending_from_column' | 'declined_from_column' }
  csvHeaders: string[]
  onChange: (rule: ImportRule) => void
  error?: string
}

const EXAMPLE_KEYS = {
  pending_from_column: 'import.rules.columnPattern.pendingExample',
  declined_from_column: 'import.rules.columnPattern.declinedExample',
} as const

const ColumnPatternForm = ({ rule, csvHeaders, onChange, error }: ColumnPatternFormProps) => {
  const { t } = useTranslation()

  const columnMissing = rule.column !== '' && !csvHeaders.includes(rule.column)

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">{t('import.rules.columnPattern.column')}</span>
        <Select value={rule.column !== '' ? rule.column : '__none__'} onValueChange={(value) => onChange({ ...rule, column: value === '__none__' ? '' : value })}>
          <SelectTrigger aria-label={t('import.rules.columnPattern.column')}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__none__">—</SelectItem>
            {csvHeaders.map((h) => (
              <SelectItem key={h} value={h}>
                {h}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {columnMissing && <p className="text-xs text-amber-600 dark:text-amber-400">{t('import.rules.unmatchedColumn', { name: rule.column })}</p>}
      </div>

      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">{t('import.rules.columnPattern.pattern')}</span>
        <Input aria-label={t('import.rules.columnPattern.pattern')} value={rule.pattern} onChange={(e) => onChange({ ...rule, pattern: e.target.value })} />
        <p className="text-xs text-muted-foreground">{t(EXAMPLE_KEYS[rule.type])}</p>
        {error && <p className="text-xs text-destructive">{error}</p>}
      </div>
    </div>
  )
}

export default ColumnPatternForm
