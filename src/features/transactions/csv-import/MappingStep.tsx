import { useTranslation } from 'react-i18next'
import type { CashflowFieldKey, ColumnMapping } from './types'
import { autoDetectMapping } from './csvParser'
import { Button } from '../../../shared/ui/button'
import { DialogFooter } from '../../../shared/ui/dialog'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../../shared/ui/select'

interface MappingStepProps {
  csvHeaders: string[]
  columnMapping: ColumnMapping
  onMappingChange: (field: CashflowFieldKey, csvColumn: string | null) => void
  onNext: () => void
  onBack: () => void
  onCancel: () => void
  canProceed: boolean
}

const FIELDS: { key: CashflowFieldKey; required: boolean }[] = [
  { key: 'date', required: true },
  { key: 'amount', required: true },
  { key: 'partner', required: false },
  { key: 'currency', required: false },
  { key: 'fxRate', required: false },
  { key: 'note', required: false },
]

const SKIP_VALUE = '__skip__'

export default function MappingStep({ csvHeaders, columnMapping, onMappingChange, onNext, onBack, onCancel, canProceed }: MappingStepProps) {
  const { t } = useTranslation()

  const autoDetectedMapping = autoDetectMapping(csvHeaders)

  const handleChange = (field: CashflowFieldKey, value: string) => {
    onMappingChange(field, value === SKIP_VALUE ? null : value)
  }

  const getSelectValue = (field: CashflowFieldKey): string => {
    return columnMapping[field] ?? SKIP_VALUE
  }

  const getHeaderLabel = (header: string): string => {
    const match = header.match(/^no-header-column-(\d+)$/)
    if (match) {
      return t('import.mappingStep.noHeader', { column: match[1] })
    }
    return header
  }

  return (
    <>
      <div className="flex flex-col gap-4">
        <p className="text-sm text-muted-foreground">{t('import.mappingStep.description')}</p>

        <div className="grid grid-cols-[1fr_1fr] items-center gap-x-4 gap-y-3">
          {FIELDS.map(({ key, required }) => (
            <div key={key} className="contents">
              <div className="flex items-center gap-1.5">
                <span className="text-sm font-medium">{t(`import.mappingStep.field.${key}`)}</span>
                {required && <span className="text-xs text-muted-foreground">{t('import.mappingStep.required')}</span>}
                {columnMapping[key] !== null && columnMapping[key] === autoDetectedMapping[key] && (
                  <span className="text-xs text-muted-foreground italic">{t('import.mappingStep.autoDetected')}</span>
                )}
              </div>
              <Select value={getSelectValue(key)} onValueChange={(v) => handleChange(key, v)}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={SKIP_VALUE}>{t('import.mappingStep.skip')}</SelectItem>
                  {csvHeaders.map((header) => (
                    <SelectItem key={header} value={header}>
                      {getHeaderLabel(header)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          ))}
        </div>
      </div>

      <DialogFooter>
        <Button type="button" variant="outline" onClick={onBack}>
          {t('import.back')}
        </Button>
        <Button type="button" variant="outline" onClick={onCancel}>
          {t('modals.confirm.cancel')}
        </Button>
        <Button type="button" onClick={onNext} disabled={!canProceed}>
          {t('import.next')}
        </Button>
      </DialogFooter>
    </>
  )
}
