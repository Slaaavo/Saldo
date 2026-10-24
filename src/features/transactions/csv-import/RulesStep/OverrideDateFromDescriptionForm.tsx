import { useTranslation } from 'react-i18next'
import type { ImportRule } from '../../../../shared/types'
import { Input } from '../../../../shared/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../../../shared/ui/select'

interface OverrideDateFromDescriptionFormProps {
  rule: ImportRule & { type: 'override_date_from_description' }
  csvHeaders: string[]
  onChange: (rule: ImportRule) => void
  errors?: { conditionRegex?: string; dateRegex?: string }
}

const REGEX_HELP_ROWS: Array<{ key: string; pattern: string; examplePattern: string; exampleText?: string }> = [
  { key: 'digit', pattern: '\\d', examplePattern: '\\d\\d', exampleText: '42' },
  { key: 'digits', pattern: '\\d+', examplePattern: '\\d+', exampleText: '123' },
  { key: 'dot', pattern: '\\.', examplePattern: '\\d{2}\\.\\d{2}', exampleText: '15.03' },
  { key: 'count', pattern: '\\d{N}', examplePattern: '\\d{4}', exampleText: '2026' },
  { key: 'group', pattern: '(...)', examplePattern: '(\\d{2}\\.\\d{2}\\.\\d{4})', exampleText: '15.03.2026' },
  { key: 'any', pattern: '.*', examplePattern: '.*(\\d{2}\\.\\d{2}\\.\\d{4})' },
  { key: 'start', pattern: '^', examplePattern: '^1234' },
  { key: 'asterisk', pattern: '\\*', examplePattern: '\\d{4}\\*{4}\\d{4}', exampleText: '1234****5678' },
]

const OverrideDateFromDescriptionForm = ({ rule, csvHeaders, onChange, errors }: OverrideDateFromDescriptionFormProps) => {
  const { t } = useTranslation()

  const descColumnMissing = rule.descriptionColumn !== '' && !csvHeaders.includes(rule.descriptionColumn)

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">{t('import.rules.overrideDateFromDescription.descriptionColumn')}</span>
        <Select
          value={rule.descriptionColumn !== '' ? rule.descriptionColumn : '__none__'}
          onValueChange={(value) =>
            onChange({
              ...rule,
              descriptionColumn: value === '__none__' ? '' : value,
            })
          }
        >
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
        {descColumnMissing && <p className="text-xs text-amber-600 dark:text-amber-400">{t('import.rules.unmatchedColumn', { name: rule.descriptionColumn })}</p>}
      </div>

      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">{t('import.rules.overrideDateFromDescription.conditionRegex')}</span>
        <Input value={rule.conditionRegex} onChange={(e) => onChange({ ...rule, conditionRegex: e.target.value })} />
        <p className="text-xs text-muted-foreground">{t('import.rules.overrideDateFromDescription.conditionExample')}</p>
        {errors?.conditionRegex && <p className="text-xs text-destructive">{errors.conditionRegex}</p>}
      </div>

      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">{t('import.rules.overrideDateFromDescription.dateRegex')}</span>
        <Input value={rule.dateRegex} onChange={(e) => onChange({ ...rule, dateRegex: e.target.value })} />
        <p className="text-xs text-muted-foreground">{t('import.rules.overrideDateFromDescription.dateExample')}</p>
        {errors?.dateRegex && <p className="text-xs text-destructive">{errors.dateRegex}</p>}
      </div>

      <details className="text-sm">
        <summary className="cursor-pointer select-none text-muted-foreground hover:text-foreground">{t('import.rules.overrideDateFromDescription.regexHint')}</summary>
        <table className="mt-2 w-full text-xs border-collapse">
          <thead>
            <tr className="text-left border-b">
              <th className="py-1 pr-4 font-medium">{t('import.rules.overrideDateFromDescription.regexTable.pattern')}</th>
              <th className="py-1 pr-4 font-medium">{t('import.rules.overrideDateFromDescription.regexTable.meaning')}</th>
              <th className="py-1 font-medium">{t('import.rules.overrideDateFromDescription.regexTable.example')}</th>
            </tr>
          </thead>
          <tbody className="text-muted-foreground">
            {REGEX_HELP_ROWS.map(({ key, pattern, examplePattern, exampleText }) => (
              <tr key={key} className="border-b border-border/40 last:border-b-0">
                <td className="py-1 pr-4 font-mono">{pattern}</td>
                <td className="py-1 pr-4">{t(`import.rules.overrideDateFromDescription.regexTable.rows.${key}.meaning`)}</td>
                <td className="py-1 font-mono">{t(`import.rules.overrideDateFromDescription.regexTable.rows.${key}.example`, { pattern: examplePattern, text: exampleText })}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </div>
  )
}

export default OverrideDateFromDescriptionForm
