import { useState } from 'react';
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
  const [name, setName] = useState('');
  const [balance, setBalance] = useState('');

  const isBucket = accountType === 'bucket';
  const entityLabel = isBucket ? 'Bucket' : 'Account';

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      window.alert(`${entityLabel} name is required.`);
      return;
    }
    let initialBalanceMinor: number | undefined;
    if (balance.trim()) {
      const parsed = parseFloat(balance);
      if (isNaN(parsed)) {
        window.alert('Please enter a valid balance amount.');
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
          <DialogTitle>Create {entityLabel}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <div className="flex flex-col gap-2">
            <Label htmlFor="create-account-name">{entityLabel} Name</Label>
            <Input
              id="create-account-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={isBucket ? 'e.g. Emergency Fund' : 'e.g. Checking Account'}
              required
              autoFocus
            />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="create-account-balance">Initial Balance (€, optional)</Label>
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
              Cancel
            </Button>
            <Button type="submit">Create</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
