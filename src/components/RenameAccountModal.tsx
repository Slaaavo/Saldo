import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from './ui/dialog';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { Label } from './ui/label';

interface Props {
  accountId: number;
  currentName: string;
  onSubmit: (accountId: number, name: string) => void;
  onClose: () => void;
}

export default function RenameAccountModal({ accountId, currentName, onSubmit, onClose }: Props) {
  const { t } = useTranslation();
  const [name, setName] = useState(currentName);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      window.alert(t('validation.nameRequired', { entity: t('common.account') }));
      return;
    }
    onSubmit(accountId, name.trim());
  };

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('modals.renameAccount.title')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <div className="flex flex-col gap-2">
            <Label htmlFor="rename-name">{t('modals.renameAccount.nameLabel')}</Label>
            <Input
              id="rename-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              required
              autoFocus
            />
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.renameAccount.cancel')}
            </Button>
            <Button type="submit">{t('modals.renameAccount.submit')}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
