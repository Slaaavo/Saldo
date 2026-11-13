import { useTranslation } from 'react-i18next'
import { Loader2 } from 'lucide-react'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogBody, DialogFooter } from './dialog'
import { Button } from './button'

const ACTIONS = {
  delete: { titleKey: 'modals.confirm.deleteTitle', submitKey: 'common.delete', variant: 'destructive' },
  confirm: { titleKey: 'modals.confirm.title', submitKey: 'modals.confirm.submit', variant: 'default' },
} as const

interface Props {
  action: keyof typeof ACTIONS
  message: string
  onConfirm: () => void
  onCancel: () => void
  loading?: boolean
}

const ConfirmDialog = ({ action, message, onConfirm, onCancel, loading = false }: Props) => {
  const { t } = useTranslation()
  const { titleKey, submitKey, variant } = ACTIONS[action]

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open && !loading) onCancel()
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t(titleKey)}</DialogTitle>
        </DialogHeader>
        <DialogBody>
          <DialogDescription>{message}</DialogDescription>
        </DialogBody>
        <DialogFooter>
          <Button variant="outline" onClick={onCancel} disabled={loading}>
            {t('modals.confirm.cancel')}
          </Button>
          <Button variant={variant} onClick={onConfirm} disabled={loading}>
            {loading && <Loader2 className="h-4 w-4 animate-spin" />}
            {t(submitKey)}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export default ConfirmDialog
