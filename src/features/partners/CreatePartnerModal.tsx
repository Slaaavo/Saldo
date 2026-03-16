import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogBody,
  DialogFooter,
} from '../../shared/ui/dialog';
import { Button } from '../../shared/ui/button';
import { Input } from '../../shared/ui/input';
import { Label } from '../../shared/ui/label';
import { extractErrorMessage } from '../../shared/utils/errors';

interface Props {
  currencyId: number;
  onSubmit: (name: string, iban: string, currencyId: number) => Promise<void>;
  onClose: () => void;
}

export default function CreatePartnerModal({ currencyId, onSubmit, onClose }: Props) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [iban, setIban] = useState('');
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      toast.error(t('validation.nameRequired', { entity: t('common.account') }));
      return;
    }
    if (!iban.trim()) {
      toast.error(t('partners.errors.invalidIban'));
      return;
    }
    setSubmitting(true);
    try {
      await onSubmit(name.trim(), iban.trim(), currencyId);
    } catch (err) {
      const msg = extractErrorMessage(err);
      if (msg.includes('DUPLICATE_IBAN') || msg.toLowerCase().includes('already in use')) {
        toast.error(t('partners.errors.duplicateIban'));
      } else if (msg.includes('15') || msg.toLowerCase().includes('alphanumeric')) {
        toast.error(t('partners.errors.invalidIban'));
      } else {
        toast.error(t('errors.createPartner', { error: msg }));
      }
      setSubmitting(false);
    }
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
          <DialogTitle>{t('modals.createPartner.title')}</DialogTitle>
        </DialogHeader>
        <form
          onSubmit={handleSubmit}
          className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4"
        >
          <DialogBody className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="create-partner-name">{t('modals.createPartner.nameLabel')}</Label>
              <Input
                id="create-partner-name"
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t('modals.createPartner.namePlaceholder')}
                required
                autoFocus
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="create-partner-iban">{t('modals.createPartner.ibanLabel')}</Label>
              <Input
                id="create-partner-iban"
                type="text"
                value={iban}
                onChange={(e) => setIban(e.target.value)}
                placeholder={t('modals.createPartner.ibanPlaceholder')}
                required
              />
            </div>
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.createPartner.cancel')}
            </Button>
            <Button type="submit" disabled={submitting}>
              {t('modals.createPartner.submit')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
