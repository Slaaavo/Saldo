import { useTranslation } from 'react-i18next';
import { Button } from '../../shared/ui/button';
import { DatePicker } from '../../shared/ui/date-picker';
import { Label } from '../../shared/ui/label';
import { EditableRateCell } from './EditableRateCell';
import { useFxRates } from './useFxRates';

export default function FxRatesPage() {
  const { t } = useTranslation();
  const {
    dates,
    targetCurrencies,
    rateMap,
    consolidationCode,
    missingDates,
    error,
    fetchDate,
    isRefreshing,
    isBackfilling,
    editingCell,
    editValue,
    setFetchDate,
    setEditValue,
    handleRefresh,
    handleBackfill,
    handleCellClick,
    handleCellSave,
    cancelEdit,
  } = useFxRates();

  return (
    <div className="px-4 md:px-10 py-8">
      <div className="mb-6">
        {/* Row 1: Summary */}
        <div className="mb-4">
          <h2 className="text-2xl font-bold">{t('fxRates.title')}</h2>
          {consolidationCode && (
            <p className="text-sm text-muted-foreground">
              {t('fxRates.subtitle', { currency: consolidationCode })}
            </p>
          )}
        </div>
        {/* Row 2: Actions */}
        <div className="flex items-center gap-3">
          {missingDates.length > 0 && (
            <Button variant="outline" onClick={handleBackfill} disabled={isBackfilling}>
              {isBackfilling
                ? t('fxRates.backfilling')
                : t('fxRates.backfill', { count: missingDates.length })}
            </Button>
          )}
          <Label className="text-sm font-medium text-muted-foreground">
            {t('fxRates.fetchHistorical')}
          </Label>
          <DatePicker value={fetchDate} onChange={setFetchDate} className="w-44" />
          <Button onClick={handleRefresh} disabled={isRefreshing}>
            {isRefreshing ? '…' : t('fxRates.refreshRates')}
          </Button>
        </div>
      </div>

      {error && (
        <div className="mb-4 rounded-md border border-destructive bg-destructive/10 px-4 py-2 text-sm text-destructive">
          {error}
        </div>
      )}

      {dates.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('fxRates.noRates')}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm border-collapse">
            <thead>
              <tr className="border-b border-border">
                <th className="sticky left-0 z-[1] bg-card text-left py-2 pr-4 font-semibold text-muted-foreground">
                  <div>{t('fxRates.date')}</div>
                  {consolidationCode && (
                    <div className="text-xs font-normal">
                      {t('fxRates.baseCurrencyPrefix', { currency: consolidationCode })}
                    </div>
                  )}
                </th>
                {targetCurrencies.map((code) => (
                  <th key={code} className="text-right py-2 px-3 font-semibold">
                    {code}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {dates.map((date) => (
                <tr
                  key={date}
                  className="group border-b border-border last:border-0 even:bg-muted/50 hover:bg-muted/30"
                >
                  <td className="sticky left-0 z-[1] bg-card group-even:bg-muted/50 py-2 pr-4 text-muted-foreground font-mono text-sm">
                    {date}
                  </td>
                  {targetCurrencies.map((code) => (
                    <EditableRateCell
                      key={code}
                      date={date}
                      code={code}
                      row={rateMap.get(`${date}:${code}`)}
                      isEditing={editingCell?.date === date && editingCell?.code === code}
                      editValue={editValue}
                      onCellClick={handleCellClick}
                      onEditValueChange={setEditValue}
                      onSave={handleCellSave}
                      onCancel={cancelEdit}
                      manualLabel={t('fxRates.isManual')}
                    />
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
