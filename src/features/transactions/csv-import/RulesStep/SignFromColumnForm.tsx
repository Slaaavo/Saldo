import { useTranslation } from 'react-i18next'
import type { ImportRule } from '../../../../shared/types'
import { Input } from '../../../../shared/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../../../shared/ui/select'

interface SignFromColumnFormProps {
  rule: ImportRule & { type: 'sign_from_column' }
  csvHeaders: string[]
  onChange: (rule: ImportRule) => void
}

const SignFromColumnForm = ({ rule, csvHeaders, onChange }: SignFromColumnFormProps) => {
  const { t } = useTranslation()

  const typeColumnMissing = rule.typeColumn !== '' && !csvHeaders.includes(rule.typeColumn)

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">{t('import.rules.signFromColumn.typeColumn')}</span>
        <Select value={rule.typeColumn !== '' ? rule.typeColumn : '__none__'} onValueChange={(value) => onChange({ ...rule, typeColumn: value === '__none__' ? '' : value })}>
          <SelectTrigger>
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
        {typeColumnMissing && <p className="text-xs text-amber-600 dark:text-amber-400">{t('import.rules.unmatchedColumn', { name: rule.typeColumn })}</p>}
      </div>

      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">{t('import.rules.signFromColumn.negativeType')}</span>
        <Input value={rule.negativeType} onChange={(e) => onChange({ ...rule, negativeType: e.target.value })} />
      </div>
    </div>
  )
}

export default SignFromColumnForm
