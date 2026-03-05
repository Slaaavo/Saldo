import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from './ui/dialog';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { Label } from './ui/label';

interface Props {
  accountType?: 'account' | 'bucket';
  onSubmit: (name: string, initialBalanceMinor?: number, accountType?: string) => void;
  onClose: () => void;
}

export default function CreateAccountModal({ accountType, onSubmit, onClose }: Props) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [balance, setBalance] = useState('');

  const isBucket = accountType === 'bucket';

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      window.alert(
        t('validation.nameRequired', { entity: t(isBucket ? 'common.bucket' : 'common.account') }),
      );
      return;
    }
    let initialBalanceMinor: number | undefined;
    if (balance.trim()) {
      const parsed = parseFloat(balance);
      if (isNaN(parsed)) {
        window.alert(t('validation.invalidBalance'));
        return;
      }
      initialBalanceMinor = Math.round(parsed * 100);
    }
    onSubmit(name.trim(), initialBalanceMinor, accountType);
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
          <DialogTitle>
            {t(isBucket ? 'modals.createBucket.title' : 'modals.createAccount.title')}
          </DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <div className="flex flex-col gap-2">
            <Label htmlFor="create-account-name">
              {t(isBucket ? 'modals.createBucket.nameLabel' : 'modals.createAccount.nameLabel')}
            </Label>
            <Input
              id="create-account-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t(
                isBucket
                  ? 'modals.createBucket.namePlaceholder'
                  : 'modals.createAccount.namePlaceholder',
              )}
              required
              autoFocus
            />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="create-account-balance">
              {t('modals.createAccount.initialBalance')}
            </Label>
            <Input
              id="create-account-balance"
              type="number"
              step="0.01"
              value={balance}
              onChange={(e) => setBalance(e.target.value)}
              placeholder="0.00"
            />
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.createAccount.cancel')}
            </Button>
            <Button type="submit">{t('modals.createAccount.submit')}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
