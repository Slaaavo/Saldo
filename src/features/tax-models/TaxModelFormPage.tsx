import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { useNavigate, useParams } from '@tanstack/react-router'
import { Button } from '../../shared/ui/button'
import { Input } from '../../shared/ui/input'
import { Label } from '../../shared/ui/label'
import { Card, CardContent, CardHeader, CardTitle } from '../../shared/ui/card'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'
import { PercentageInput } from '../../shared/ui/PercentageInput'
import { DatePicker } from '../../shared/ui/date-picker'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../shared/ui/select'
import BracketEditor from './BracketEditor'
import { useTaxModelForm } from './useTaxModelForm'
import { listPersons, getTaxModel } from '../../shared/api'
import { usePageTitle } from '../../app/usePageHeader'

const TaxModelFormPage = () => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const params = useParams({ strict: false }) as { modelId?: string }
  const modelId = params.modelId ? Number(params.modelId) : undefined

  const { data: persons = [], isLoading: personsLoading } = useQuery({
    queryKey: ['persons'],
    queryFn: listPersons,
  })

  const { data: modelData, isLoading: modelLoading } = useQuery({
    queryKey: ['taxModel', modelId],
    queryFn: () => getTaxModel(modelId!),
    enabled: modelId !== undefined,
  })

  const isLoading = personsLoading || (modelId !== undefined && modelLoading)

  const {
    name,
    setName,
    calendarYear,
    setCalendarYear,
    personId,
    setPersonId,
    vatStatus,
    setVatStatus,
    vatFromDate,
    setVatFromDate,
    reserveFundCurrentMinor,
    setReserveFundCurrentMinor,
    reserveFundPctBps,
    setReserveFundPctBps,
    reserveFundMaxMinor,
    setReserveFundMaxMinor,
    dividendTaxRateBps,
    setDividendTaxRateBps,
    brackets,
    setBrackets,
    selectedPersonType,
    handleSave,
  } = useTaxModelForm(modelId, persons, modelData)

  const title = modelId ? t('taxModels.formTitleEdit') : t('taxModels.formTitleCreate')
  usePageTitle(title)

  return (
    <div className="px-4 md:px-10 py-8 max-w-2xl mx-auto">
      <div className="flex items-center mb-6">
        <Button variant="ghost" size="sm" onClick={() => navigate({ to: '/tax-models' })}>
          ← {t('taxModels.backToList')}
        </Button>
      </div>

      {isLoading ? null : (
        <div className="space-y-6">
          {/* Section 1: General Settings */}
          <Card>
            <CardHeader>
              <CardTitle className="text-base">{t('taxModels.generalSettings')}</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div>
                <Label className="text-sm mb-1 block">{t('taxModels.nameLabel')}</Label>
                <Input value={name} onChange={(e) => setName(e.target.value)} placeholder={t('taxModels.nameLabel')} />
              </div>
              <div>
                <Label className="text-sm mb-1 block">{t('taxModels.yearLabel')}</Label>
                <Input type="number" value={calendarYear} onChange={(e) => setCalendarYear(Number(e.target.value))} min={2000} max={2100} />
              </div>
              <div>
                <Label className="text-sm mb-1 block">{t('taxModels.personLabel')}</Label>
                <Select value={personId !== null ? String(personId) : undefined} onValueChange={(val) => setPersonId(Number(val))}>
                  <SelectTrigger>
                    <SelectValue placeholder={t('taxModels.personLabel')} />
                  </SelectTrigger>
                  <SelectContent>
                    {persons.map((p) => (
                      <SelectItem key={p.id} value={String(p.id)}>
                        {p.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </CardContent>
          </Card>

          {/* Section 2: Tax Settings */}
          <Card>
            <CardHeader>
              <CardTitle className="text-base">{t('taxModels.taxSettings')}</CardTitle>
            </CardHeader>
            <CardContent>
              <BracketEditor brackets={brackets} onBracketsChange={setBrackets} />
            </CardContent>
          </Card>

          {/* Section 3: Legal Person Settings (only for legal persons) */}
          {selectedPersonType === 'legal' && (
            <Card>
              <CardHeader>
                <CardTitle className="text-base">{t('taxModels.legalPersonSettings')}</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <div>
                  <Label className="text-sm mb-1 block">{t('taxModels.reserveFundCurrent')}</Label>
                  <CurrencyInput
                    type="number"
                    step="0.01"
                    min="0"
                    value={reserveFundCurrentMinor}
                    onChange={(e) => setReserveFundCurrentMinor(e.target.value)}
                    placeholder="0.00"
                  />
                </div>
                <div>
                  <Label className="text-sm mb-1 block">{t('taxModels.reserveFundPct')}</Label>
                  <PercentageInput
                    type="number"
                    step="0.01"
                    min="0"
                    max="100"
                    value={reserveFundPctBps}
                    onChange={(e) => setReserveFundPctBps(e.target.value)}
                    placeholder="0.00"
                  />
                </div>
                <div>
                  <Label className="text-sm mb-1 block">{t('taxModels.reserveFundMax')}</Label>
                  <CurrencyInput type="number" step="0.01" min="0" value={reserveFundMaxMinor} onChange={(e) => setReserveFundMaxMinor(e.target.value)} placeholder="0.00" />
                </div>
                <div>
                  <Label className="text-sm mb-1 block">{t('taxModels.dividendTaxRate')}</Label>
                  <PercentageInput
                    type="number"
                    step="0.01"
                    min="0"
                    max="100"
                    value={dividendTaxRateBps}
                    onChange={(e) => setDividendTaxRateBps(e.target.value)}
                    placeholder="0.00"
                  />
                </div>
              </CardContent>
            </Card>
          )}

          {/* Section 4: VAT Payer Status */}
          <Card>
            <CardHeader>
              <CardTitle className="text-base">{t('taxModels.vatStatus')}</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                {(['none', 'all_year', 'from_date'] as const).map((status) => (
                  <label key={status} className="flex items-center gap-2 cursor-pointer">
                    <input type="radio" name="vatStatus" value={status} checked={vatStatus === status} onChange={() => setVatStatus(status)} className="h-4 w-4" />
                    <span className="text-sm">
                      {status === 'none' && t('taxModels.vatNone')}
                      {status === 'all_year' && t('taxModels.vatAllYear')}
                      {status === 'from_date' && t('taxModels.vatFromDate')}
                    </span>
                  </label>
                ))}
              </div>
              {vatStatus === 'from_date' && (
                <div>
                  <Label className="text-sm mb-1 block">{t('taxModels.vatFromDateLabel')}</Label>
                  <DatePicker value={vatFromDate || undefined} onChange={setVatFromDate} placeholder={t('taxModels.vatFromDateLabel')} />
                </div>
              )}
            </CardContent>
          </Card>

          <div className="flex justify-end">
            <Button onClick={handleSave}>{t('taxModels.saveAndCalculate')}</Button>
          </div>
        </div>
      )}
    </div>
  )
}

export default TaxModelFormPage
