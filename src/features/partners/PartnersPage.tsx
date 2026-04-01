import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Button } from '../../shared/ui/button'
import { Input } from '../../shared/ui/input'
import ConfirmDialog from '../../shared/ui/ConfirmDialog'
import { extractErrorMessage } from '../../shared/utils/errors'
import { usePartners } from './usePartners'
import CreatePartnerModal from './CreatePartnerModal'
import EditPartnerModal from './EditPartnerModal'

export default function PartnersPage() {
  const { t } = useTranslation()
  const {
    filteredPartners,
    loading,
    searchQuery,
    setSearchQuery,
    consolidationCurrencyId,
    createModalOpen,
    setCreateModalOpen,
    editingPartner,
    setEditingPartner,
    deletingPartner,
    setDeletingPartner,
    handleCreate,
    handleUpdate,
    handleDelete,
  } = usePartners()

  const handleDeleteWithError = async (accountId: number) => {
    try {
      await handleDelete(accountId)
    } catch (err) {
      toast.error(t('errors.deletePartner', { error: extractErrorMessage(err) }))
    }
  }

  return (
    <div className="px-4 md:px-10 py-8">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-2xl font-bold">{t('partners.title')}</h2>
        <Button onClick={() => setCreateModalOpen(true)}>{t('partners.addPartner')}</Button>
      </div>

      {/* Search */}
      <div className="mb-4">
        <Input type="text" placeholder={t('partners.search')} value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} className="max-w-sm" />
      </div>

      {/* Table */}
      {loading ? null : filteredPartners.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('partners.empty')}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm border-collapse">
            <thead>
              <tr className="border-b border-border">
                <th className="text-left py-2 pr-4 font-semibold text-muted-foreground">{t('partners.name')}</th>
                <th className="text-left py-2 pr-4 font-semibold text-muted-foreground">{t('partners.iban')}</th>
                <th className="text-right py-2 font-semibold text-muted-foreground" />
              </tr>
            </thead>
            <tbody>
              {filteredPartners.map((partner) => (
                <tr key={partner.id} className="border-b border-border last:border-0 even:bg-muted/50">
                  <td className="py-2 pr-4 font-medium">{partner.name}</td>
                  <td className="py-2 pr-4 font-mono text-muted-foreground">{partner.iban ?? '—'}</td>
                  <td className="py-2 text-right">
                    <div className="flex gap-2 justify-end">
                      <Button variant="ghost" size="sm" onClick={() => setEditingPartner(partner)}>
                        {t('accounts.rename')}
                      </Button>
                      <Button variant="ghost" size="sm" className="text-destructive hover:text-destructive" onClick={() => setDeletingPartner(partner)}>
                        {t('accounts.delete')}
                      </Button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Modals */}
      {createModalOpen && consolidationCurrencyId !== null && (
        <CreatePartnerModal currencyId={consolidationCurrencyId} onSubmit={handleCreate} onClose={() => setCreateModalOpen(false)} />
      )}
      {editingPartner && <EditPartnerModal partner={editingPartner} onSubmit={handleUpdate} onClose={() => setEditingPartner(null)} />}
      {deletingPartner && (
        <ConfirmDialog
          message={t('partners.deleteConfirm', { name: deletingPartner.name })}
          onConfirm={() => handleDeleteWithError(deletingPartner.id)}
          onCancel={() => setDeletingPartner(null)}
        />
      )}
    </div>
  )
}
