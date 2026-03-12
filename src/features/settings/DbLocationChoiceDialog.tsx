import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Loader2 } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogBody,
  DialogFooter,
} from '../../shared/ui/dialog';
import { Button } from '../../shared/ui/button';

type DbLocationAction = 'move' | 'fresh';

interface Props {
  onAction: (action: DbLocationAction) => void;
  onCancel: () => void;
  loading?: boolean;
}

export default function DbLocationChoiceDialog({ onAction, onCancel, loading = false }: Props) {
  const { t } = useTranslation();
  const [pendingAction, setPendingAction] = useState<DbLocationAction | null>(null);

  const handleAction = (action: DbLocationAction) => {
    setPendingAction(action);
    onAction(action);
  };

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open && !loading) onCancel();
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('dataStorage.noDbFoundTitle')}</DialogTitle>
        </DialogHeader>
        <DialogBody>
          <DialogDescription>{t('dataStorage.noDbFoundMessage')}</DialogDescription>
        </DialogBody>
        <DialogFooter>
          <Button variant="outline" onClick={onCancel} disabled={loading}>
            {t('dataStorage.noDbFoundCancel')}
          </Button>
          <Button variant="outline" onClick={() => handleAction('move')} disabled={loading}>
            {loading && pendingAction === 'move' && (
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            )}
            {t('dataStorage.moveButton')}
          </Button>
          <Button onClick={() => handleAction('fresh')} disabled={loading}>
            {loading && pendingAction === 'fresh' && (
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            )}
            {t('dataStorage.freshButton')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
