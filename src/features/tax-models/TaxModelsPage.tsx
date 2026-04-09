import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from '@tanstack/react-router'
import { toast } from 'sonner'
import { Button } from '../../shared/ui/button'
import ConfirmDialog from '../../shared/ui/ConfirmDialog'
import { extractErrorMessage } from '../../shared/utils/errors'
import { useTaxModels } from './useTaxModels'
import type { TaxModelRow } from '../../shared/types'

const TaxModelsPage = () => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { taxModels, isLoading, handleDeleteTaxModel } = useTaxModels()
  const [deletingModel, setDeletingModel] = useState<TaxModelRow | null>(null)

  const handleDeleteWithError = async (modelId: number) => {
    try {
      await handleDeleteTaxModel(modelId)
      setDeletingModel(null)
    } catch (err) {
      toast.error(extractErrorMessage(err))
    }
  }

  return (
    <div className="px-4 md:px-10 py-8">
      <div className="flex justify-end mb-6">
        <Button onClick={() => navigate({ to: '/tax-models/new' })}>{t('taxModels.createModel')}</Button>
      </div>

      {isLoading ? null : taxModels.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('taxModels.empty')}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm border-collapse">
            <thead>
              <tr className="border-b border-border">
                <th className="text-left py-2 pr-4 font-semibold text-muted-foreground">{t('taxModels.name')}</th>
                <th className="text-left py-2 pr-4 font-semibold text-muted-foreground">{t('taxModels.year')}</th>
                <th className="text-left py-2 pr-4 font-semibold text-muted-foreground">{t('taxModels.person')}</th>
                <th className="text-right py-2 font-semibold text-muted-foreground" />
              </tr>
            </thead>
            <tbody>
              {taxModels.map((row) => (
                <tr key={row.id} className="border-b border-border last:border-0 even:bg-muted/50">
                  <td className="py-2 pr-4">
                    <button className="font-bold hover:underline text-left" onClick={() => navigate({ to: '/tax-models/$modelId/results', params: { modelId: String(row.id) } })}>
                      {row.name}
                    </button>
                  </td>
                  <td className="py-2 pr-4 text-muted-foreground">{row.calendarYear}</td>
                  <td className="py-2 pr-4">
                    <span>{row.personName}</span>
                    <span className="ml-2 text-xs font-normal text-muted-foreground border border-border rounded px-1">
                      {row.personType === 'physical' ? t('taxModels.personTypeBadgePhysical') : t('taxModels.personTypeBadgeLegal')}
                    </span>
                  </td>
                  <td className="py-2 text-right">
                    <Button variant="ghost" size="sm" className="text-destructive hover:text-destructive" onClick={() => setDeletingModel(row)}>
                      {t('accounts.delete')}
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {deletingModel && <ConfirmDialog message={t('taxModels.deleteConfirm')} onConfirm={() => handleDeleteWithError(deletingModel.id)} onCancel={() => setDeletingModel(null)} />}
    </div>
  )
}

export default TaxModelsPage
