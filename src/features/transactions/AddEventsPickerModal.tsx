import { useTranslation } from 'react-i18next'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody } from '../../shared/ui/dialog'
import { RefreshCw, TrendingUp, TrendingDown } from 'lucide-react'

interface Props {
  onBulkUpdate: () => void
  onRevenue: () => void
  onExpense: () => void
  onClose: () => void
}

const AddEventsPickerModal = ({ onBulkUpdate, onRevenue, onExpense, onClose }: Props) => {
  const { t } = useTranslation()

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('modals.addEvents.title')}</DialogTitle>
        </DialogHeader>
        <DialogBody>
          <div className="flex flex-col gap-3">
            <button type="button" onClick={onBulkUpdate} className="flex items-start gap-4 rounded-lg border border-border p-4 text-left hover:bg-muted transition-colors">
              <RefreshCw className="h-6 w-6 mt-0.5 text-muted-foreground shrink-0" />
              <div>
                <p className="font-medium text-sm">{t('modals.addEvents.bulkBalanceUpdate')}</p>
                <p className="text-xs text-muted-foreground mt-0.5">{t('modals.addEvents.bulkBalanceUpdateDesc')}</p>
              </div>
            </button>
            <button type="button" onClick={onRevenue} className="flex items-start gap-4 rounded-lg border border-border p-4 text-left hover:bg-muted transition-colors">
              <TrendingUp className="h-6 w-6 mt-0.5 text-amber-600 dark:text-amber-400 shrink-0" />
              <div>
                <p className="font-medium text-sm">{t('modals.addEvents.revenue')}</p>
                <p className="text-xs text-muted-foreground mt-0.5">{t('modals.addEvents.revenueDesc')}</p>
              </div>
            </button>
            <button type="button" onClick={onExpense} className="flex items-start gap-4 rounded-lg border border-border p-4 text-left hover:bg-muted transition-colors">
              <TrendingDown className="h-6 w-6 mt-0.5 text-rose-600 dark:text-rose-400 shrink-0" />
              <div>
                <p className="font-medium text-sm">{t('modals.addEvents.expense')}</p>
                <p className="text-xs text-muted-foreground mt-0.5">{t('modals.addEvents.expenseDesc')}</p>
              </div>
            </button>
          </div>
        </DialogBody>
      </DialogContent>
    </Dialog>
  )
}

export default AddEventsPickerModal
