import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import type { SnapshotRow } from '../../shared/types';
import { todayIso, toMinorUnits, getMinorUnitsStep } from '../../shared/utils/format';
import { extractErrorMessage } from '../../shared/utils/errors';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogBody,
  DialogFooter,
} from '../../shared/ui/dialog';
import { Button } from '../../shared/ui/button';
import { CurrencyInput } from '../../shared/ui/CurrencyInput';
import { Input } from '../../shared/ui/input';
import { Label } from '../../shared/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '../../shared/ui/select';
import { DatePicker } from '../../shared/ui/date-picker';
import { useBucketAllocations } from '../buckets/useBucketAllocations';
import BucketAllocationEditor from '../buckets/BucketAllocationEditor';

interface Props {
  accounts: SnapshotRow[];
  preselectedAccountId?: number;
  onSubmit: (accountId: number, amountMinor: number, eventDate: string, note: string) => void;
  onClose: () => void;
}

export default function CreateBalanceUpdateModal({
  accounts,
  preselectedAccountId,
  onSubmit,
  onClose,
}: Props) {
  const { t } = useTranslation();

  const initialAccountId = preselectedAccountId ?? accounts[0]?.accountId ?? 0;
  const [accountId, setAccountId] = useState<number>(initialAccountId);
  const [amount, setAmount] = useState(() => {
    const a = accounts.find((acc) => acc.accountId === initialAccountId);
    return a?.accountType === 'bucket' ? '0' : '';
  });
  const [date, setDate] = useState(todayIso());
  const [note, setNote] = useState('');

  const selectedAccount = accounts.find((a) => a.accountId === accountId);
  const minorUnits = selectedAccount?.currencyMinorUnits ?? 2;
  const currencyCode = selectedAccount?.currencyCode;
  const isBucket = selectedAccount?.accountType === 'bucket';

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
    bucketId: isBucket ? accountId : null,
    date,
    allocationSources,
  });
  const [submitting, setSubmitting] = useState(false);

  const handleAccountChange = (val: string) => {
    const newId = Number(val);
    setAccountId(newId);
    const newAccount = accounts.find((a) => a.accountId === newId);
    setAmount(newAccount?.accountType === 'bucket' ? '0' : '');
  };

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
      onSubmit(accountId, amountMinor, date, note);
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
          <DialogTitle>{t('modals.createBalanceUpdate.title')}</DialogTitle>
        </DialogHeader>
        <form
          onSubmit={handleSubmit}
          className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4"
        >
          <DialogBody className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label>{t('modals.createBalanceUpdate.account')}</Label>
              <Select value={String(accountId)} onValueChange={handleAccountChange}>
                <SelectTrigger>
                  <SelectValue placeholder={t('modals.createBalanceUpdate.selectAccount')} />
                </SelectTrigger>
                <SelectContent>
                  {accounts.map((a) => (
                    <SelectItem key={a.accountId} value={String(a.accountId)}>
                      {a.accountName}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="cbu-amount">
                {isBucket
                  ? t('modals.createBalanceUpdate.extraBalance')
                  : t('modals.createBalanceUpdate.amount')}
              </Label>
              <CurrencyInput
                id="cbu-amount"
                type="number"
                step={getMinorUnitsStep(minorUnits)}
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
                currencyCode={currencyCode}
                required
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="cbu-date">{t('modals.createBalanceUpdate.date')}</Label>
              <DatePicker id="cbu-date" value={date} onChange={setDate} />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="cbu-note">{t('modals.createBalanceUpdate.note')}</Label>
              <Input
                id="cbu-note"
                type="text"
                value={note}
                onChange={(e) => setNote(e.target.value)}
                placeholder={t('modals.createBalanceUpdate.notePlaceholder')}
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
              {t('modals.createBalanceUpdate.cancel')}
            </Button>
            <Button type="submit" disabled={submitting || hasErrors}>
              {t('modals.createBalanceUpdate.submit')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
