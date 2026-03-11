import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import type { EventWithData, SnapshotRow } from '../types';
import { formatDate, fromMinorUnits, toMinorUnits, getMinorUnitsStep } from '../utils/format';
import { extractErrorMessage } from '../utils/errors';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogBody,
  DialogFooter,
} from './ui/dialog';
import { Button } from './ui/button';
import { CurrencyInput } from './CurrencyInput';
import { Input } from './ui/input';
import { Label } from './ui/label';
import { DatePicker } from './ui/date-picker';
import { useBucketAllocations } from '../hooks/useBucketAllocations';
import BucketAllocationEditor from './BucketAllocationEditor';

interface Props {
  event: EventWithData;
  accounts: SnapshotRow[];
  onSubmit: (eventId: number, amountMinor: number, eventDate: string, note: string) => void;
  onClose: () => void;
}

export default function EditBalanceUpdateModal({ event, accounts, onSubmit, onClose }: Props) {
  const { t } = useTranslation();
  const minorUnits = event.currencyMinorUnits;
  const isBucket = event.accountType === 'bucket';
  const [amount, setAmount] = useState(fromMinorUnits(event.amountMinor, minorUnits));
  const [date, setDate] = useState(formatDate(event.eventDate));
  const [note, setNote] = useState(event.note ?? '');

  const allocationSources = accounts.filter(
    (a) => a.accountType === 'account' || a.accountType === 'asset',
  );

  const {
    loadingAllocations,
    visibleAllocations,
    availableToLink,
    displayErrors,
    hasErrors,
    getAvailableBalance,
    handleSourceAccountSelect,
    handleAddAllocation,
    handleUnlink,
    handleRemoveNew,
    handleAllocationAmountChange,
    saveAllocations,
  } = useBucketAllocations({
    bucketId: isBucket ? event.accountId : null,
    date,
    allocationSources,
  });
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const parsed = parseFloat(amount);
    if (isNaN(parsed)) {
      toast.error(t('validation.invalidAmount'));
      return;
    }
    setSubmitting(true);
    try {
      if (isBucket) {
        const saved = await saveAllocations();
        if (!saved) {
          setSubmitting(false);
          return;
        }
      }
      const amountMinor = toMinorUnits(amount, minorUnits);
      onSubmit(event.id, amountMinor, date, note);
    } catch (err) {
      toast.error(extractErrorMessage(err));
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
          <DialogTitle>{t('modals.editBalanceUpdate.title')}</DialogTitle>
        </DialogHeader>
        <form
          onSubmit={handleSubmit}
          className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4"
        >
          <DialogBody className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label>{t('modals.editBalanceUpdate.account')}</Label>
              <Input type="text" value={event.accountName} disabled className="bg-muted" />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="ebu-amount">
                {isBucket
                  ? t('modals.createBalanceUpdate.extraBalance')
                  : t('modals.editBalanceUpdate.amount')}
              </Label>
              <CurrencyInput
                id="ebu-amount"
                type="number"
                step={getMinorUnitsStep(minorUnits)}
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
                currencyCode={event.currencyCode}
                required
                autoFocus
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="ebu-date">{t('modals.editBalanceUpdate.date')}</Label>
              <DatePicker id="ebu-date" value={date} onChange={setDate} />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="ebu-note">{t('modals.editBalanceUpdate.note')}</Label>
              <Input
                id="ebu-note"
                type="text"
                value={note}
                onChange={(e) => setNote(e.target.value)}
                placeholder={t('modals.editBalanceUpdate.notePlaceholder')}
              />
            </div>

            {isBucket && (
              <BucketAllocationEditor
                visibleAllocations={visibleAllocations}
                availableToLink={availableToLink}
                allocationSources={allocationSources}
                loadingAllocations={loadingAllocations}
                displayErrors={displayErrors}
                getAvailableBalance={getAvailableBalance}
                handleSourceAccountSelect={handleSourceAccountSelect}
                handleAllocationAmountChange={handleAllocationAmountChange}
                handleAddAllocation={handleAddAllocation}
                handleRemoveNew={handleRemoveNew}
                handleUnlink={handleUnlink}
              />
            )}
          </DialogBody>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.editBalanceUpdate.cancel')}
            </Button>
            <Button type="submit" disabled={submitting || hasErrors}>
              {t('modals.editBalanceUpdate.submit')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
