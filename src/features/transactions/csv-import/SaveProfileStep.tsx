import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { DialogHeader, DialogTitle } from '../../../shared/ui/dialog';
import { Button } from '../../../shared/ui/button';
import { Input } from '../../../shared/ui/input';

interface SaveProfileStepProps {
  importedCount: number;
  loadedProfileName: string | null;
  onSaveNew: (name: string) => Promise<void>;
  onUpdate: () => Promise<void>;
  onClose: () => void;
}

export default function SaveProfileStep({
  importedCount,
  loadedProfileName,
  onSaveNew,
  onUpdate,
  onClose,
}: SaveProfileStepProps) {
  const { t } = useTranslation();
  const [profileName, setProfileName] = useState('');
  const [saving, setSaving] = useState(false);

  const handleSaveNew = async () => {
    if (!profileName.trim()) return;
    setSaving(true);
    try {
      await onSaveNew(profileName.trim());
    } finally {
      setSaving(false);
    }
  };

  const handleUpdate = async () => {
    setSaving(true);
    try {
      await onUpdate();
    } finally {
      setSaving(false);
    }
  };

  return (
    <>
      <DialogHeader>
        <DialogTitle>{t('import.saveStep.title')}</DialogTitle>
      </DialogHeader>

      <div className="flex flex-col gap-4">
        <p className="text-sm">{t('import.saveStep.success', { count: importedCount })}</p>

        {loadedProfileName === null ? (
          // Variant A: no profile was loaded
          <div className="flex flex-col gap-3">
            <p className="text-sm text-muted-foreground">{t('import.saveStep.savePrompt')}</p>
            <div className="flex gap-2">
              <Input
                value={profileName}
                onChange={(e) => setProfileName(e.target.value)}
                placeholder={t('import.saveStep.profileName')}
                className="flex-1"
                onKeyDown={(e) => {
                  if (e.key === 'Enter') void handleSaveNew();
                }}
              />
              <Button
                type="button"
                onClick={() => void handleSaveNew()}
                disabled={!profileName.trim() || saving}
              >
                {t('import.saveStep.save')}
              </Button>
            </div>
          </div>
        ) : (
          // Variant B: a profile was loaded and changed
          <div className="flex flex-col gap-4">
            <p className="text-sm text-muted-foreground">
              {t('import.saveStep.changedPrompt', { name: loadedProfileName })}
            </p>

            <Button
              type="button"
              onClick={() => void handleUpdate()}
              disabled={saving}
              variant="default"
            >
              {t('import.saveStep.update', { name: loadedProfileName })}
            </Button>

            <div className="relative flex items-center gap-2">
              <div className="h-px flex-1 bg-border" />
              <span className="text-xs text-muted-foreground">
                {t('import.saveStep.orSaveNew')}
              </span>
              <div className="h-px flex-1 bg-border" />
            </div>

            <div className="flex gap-2">
              <Input
                value={profileName}
                onChange={(e) => setProfileName(e.target.value)}
                placeholder={t('import.saveStep.profileName')}
                className="flex-1"
                onKeyDown={(e) => {
                  if (e.key === 'Enter') void handleSaveNew();
                }}
              />
              <Button
                type="button"
                onClick={() => void handleSaveNew()}
                disabled={!profileName.trim() || saving}
                variant="outline"
              >
                {t('import.saveStep.saveAsNew')}
              </Button>
            </div>
          </div>
        )}

        <div className="flex justify-end">
          <Button type="button" variant="outline" onClick={onClose} disabled={saving}>
            {t('import.saveStep.close')}
          </Button>
        </div>
      </div>
    </>
  );
}
