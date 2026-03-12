import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { X, Link2 } from 'lucide-react';
import type { SnapshotRow } from '../../shared/types';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogBody,
  DialogFooter,
} from '../../shared/ui/dialog';
import { Button } from '../../shared/ui/button';

interface Props {
  accountId: number;
  accountName: string;
  assets: SnapshotRow[];
  currentLinks: number[];
  onSave: (accountId: number, assetIds: number[]) => void;
  onClose: () => void;
}

export default function ManageLinkedAssetsModal({
  accountId,
  accountName,
  assets,
  currentLinks,
  onSave,
  onClose,
}: Props) {
  const { t } = useTranslation();
  const [linkedAssetIds, setLinkedAssetIds] = useState<number[]>(currentLinks);

  const linkedAssets = assets.filter((a) => linkedAssetIds.includes(a.accountId));
  const availableAssets = assets.filter((a) => !linkedAssetIds.includes(a.accountId));

  const handleUnlink = (assetId: number) => {
    setLinkedAssetIds((prev) => prev.filter((id) => id !== assetId));
  };

  const handleLink = (assetId: number) => {
    setLinkedAssetIds((prev) => [...prev, assetId]);
  };

  const handleSave = () => {
    onSave(accountId, linkedAssetIds);
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
          <DialogTitle>{t('modals.manageLinkedAssets.title', { name: accountName })}</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            <p className="text-sm text-muted-foreground">
              {t('modals.manageLinkedAssets.helperText')}
            </p>

            {assets.length === 0 ? (
              <p className="text-sm text-muted-foreground italic">
                {t('modals.manageLinkedAssets.noAssetsAvailable')}
              </p>
            ) : (
              <>
                {linkedAssets.length === 0 ? (
                  <p className="text-sm text-muted-foreground italic">
                    {t('modals.manageLinkedAssets.noLinks')}
                  </p>
                ) : (
                  <ul className="flex flex-col gap-2">
                    {linkedAssets.map((asset) => (
                      <li
                        key={asset.accountId}
                        className="flex items-center justify-between rounded-md border border-border px-3 py-2 text-sm"
                      >
                        <div className="flex items-center gap-2">
                          <Link2 className="h-3.5 w-3.5 text-muted-foreground" />
                          <span>{asset.accountName}</span>
                        </div>
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon"
                          className="h-6 w-6 text-muted-foreground hover:text-destructive"
                          onClick={() => handleUnlink(asset.accountId)}
                          aria-label={t('modals.manageLinkedAssets.unlink')}
                        >
                          <X className="h-3.5 w-3.5" />
                        </Button>
                      </li>
                    ))}
                  </ul>
                )}

                {availableAssets.length > 0 && (
                  <div className="flex flex-col gap-2">
                    <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      {t('modals.manageLinkedAssets.addLink')}
                    </p>
                    <ul className="flex flex-col gap-1">
                      {availableAssets.map((asset) => (
                        <li key={asset.accountId}>
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            className="w-full justify-start gap-2"
                            onClick={() => handleLink(asset.accountId)}
                          >
                            <Link2 className="h-3.5 w-3.5" />
                            {asset.accountName}
                          </Button>
                        </li>
                      ))}
                    </ul>
                  </div>
                )}
              </>
            )}
          </DialogBody>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.manageLinkedAssets.cancel')}
            </Button>
            <Button type="button" onClick={handleSave}>
              {t('modals.manageLinkedAssets.save')}
            </Button>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  );
}
