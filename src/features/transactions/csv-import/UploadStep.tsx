import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import type { SnapshotRow, ImportProfileRow } from '../../../shared/types';
import { cn } from '../../../shared/lib/utils';
import { Button } from '../../../shared/ui/button';
import { DialogFooter } from '../../../shared/ui/dialog';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '../../../shared/ui/select';

interface UploadStepProps {
  fileName: string | null;
  selectedAccountId: number | null;
  accounts: SnapshotRow[];
  profiles: ImportProfileRow[];
  selectedProfileId: number | null;
  onProfileSelect: (profileId: number | null) => void;
  onFileSelect: (file: File) => Promise<void>;
  onAccountSelect: (accountId: number) => Promise<void>;
  onNext: () => void;
  onCancel: () => void;
  canProceed: boolean;
}

export default function UploadStep({
  fileName,
  selectedAccountId,
  accounts,
  profiles,
  selectedProfileId,
  onProfileSelect,
  onFileSelect,
  onAccountSelect,
  onNext,
  onCancel,
  canProceed,
}: UploadStepProps) {
  const { t } = useTranslation();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [isDragOver, setIsDragOver] = useState(false);

  const handleFile = async (file: File) => {
    if (!file.name.toLowerCase().endsWith('.csv')) {
      toast.error(t('import.uploadStep.invalidFile'));
      return;
    }
    await onFileSelect(file);
  };

  const handleDragOver = (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    setIsDragOver(true);
  };

  const handleDragLeave = () => {
    setIsDragOver(false);
  };

  const handleDrop = async (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    setIsDragOver(false);
    const file = e.dataTransfer.files[0];
    if (file) {
      await handleFile(file);
    }
  };

  const handleInputChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      await handleFile(file);
    }
    // Reset input so same file can be re-selected
    e.target.value = '';
  };

  const handleAccountChange = async (value: string) => {
    await onAccountSelect(Number(value));
  };

  return (
    <>
      <div className="flex flex-col gap-4">
        <div
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
          className={cn(
            'flex flex-col items-center justify-center rounded-[var(--radius)] border-2 border-dashed p-8 text-center transition-colors',
            isDragOver
              ? 'border-primary bg-primary/5'
              : 'border-border bg-muted/20 hover:border-primary/50',
          )}
        >
          {fileName ? (
            <p className="text-sm font-medium">{fileName}</p>
          ) : (
            <p className="text-sm text-muted-foreground">{t('import.uploadStep.dragDrop')}</p>
          )}
          <input
            ref={fileInputRef}
            type="file"
            accept=".csv"
            className="hidden"
            onChange={handleInputChange}
          />
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="mt-3"
            onClick={() => fileInputRef.current?.click()}
          >
            {t('import.uploadStep.browse')}
          </Button>
        </div>

        <div className="flex flex-col gap-2">
          <span className="text-sm font-medium">{t('import.uploadStep.selectAccount')}</span>
          <Select
            value={selectedAccountId !== null ? String(selectedAccountId) : ''}
            onValueChange={handleAccountChange}
          >
            <SelectTrigger>
              <SelectValue placeholder={t('import.uploadStep.selectAccount')} />
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
          <span className="text-sm font-medium">{t('import.uploadStep.profile')}</span>
          <Select
            value={selectedProfileId !== null ? String(selectedProfileId) : '__none__'}
            onValueChange={(value) => {
              onProfileSelect(value === '__none__' ? null : Number(value));
            }}
            disabled={selectedAccountId === null}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="__none__">{t('import.uploadStep.profileNone')}</SelectItem>
              {profiles.map((p) => (
                <SelectItem key={p.id} value={String(p.id)}>
                  {p.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      <DialogFooter>
        <Button type="button" variant="outline" onClick={onCancel}>
          {t('modals.confirm.cancel')}
        </Button>
        <Button type="button" onClick={onNext} disabled={!canProceed}>
          {t('import.next')}
        </Button>
      </DialogFooter>
    </>
  );
}
