import { useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import {
  getDbLocation,
  pickDbFolder,
  changeDbLocation,
  resetDbLocation,
  checkDefaultDb,
} from '../api';
import type { ModalState } from '../types';
import { extractErrorMessage } from '../utils/errors';

interface UseDbLocationOptions {
  setModalState: (state: ModalState) => void;
  closeModal: () => void;
  onAfterDbChange: () => Promise<void>;
}

export function useDbLocation({
  setModalState,
  closeModal,
  onAfterDbChange,
}: UseDbLocationOptions) {
  const { t } = useTranslation();
  const [path, setPath] = useState('');
  const [isDefault, setIsDefault] = useState(true);
  const [actionLoading, setActionLoading] = useState(false);

  const load = useCallback(async () => {
    try {
      const info = await getDbLocation();
      setPath(info.currentPath);
      setIsDefault(info.isDefault);
      if (info.fallbackWarning) {
        toast.warning(t('dataStorage.toasts.fallbackWarning'));
      }
    } catch (err) {
      console.error('Failed to load DB location:', err);
    }
  }, [t]);

  useEffect(() => {
    load();
  }, [load]);

  const handleChange = async () => {
    try {
      const result = await pickDbFolder();
      if (!result) return;
      if (result.dbExists) {
        setModalState({ type: 'confirmSwitchDb', folder: result.folder });
      } else {
        setModalState({ type: 'dbLocationChoice', folder: result.folder, isReset: false });
      }
    } catch (err) {
      toast.error(t('dataStorage.errors.change', { error: extractErrorMessage(err) }));
    }
  };

  const handleReset = async () => {
    try {
      const dbExists = await checkDefaultDb();
      if (dbExists) {
        setModalState({ type: 'confirmResetDbLocation' });
      } else {
        setModalState({ type: 'dbLocationChoice', folder: '', isReset: true });
      }
    } catch (err) {
      toast.error(t('dataStorage.errors.reset', { error: extractErrorMessage(err) }));
    }
  };

  const handleConfirmSwitch = async (folder: string) => {
    setActionLoading(true);
    try {
      await changeDbLocation(folder, 'switch');
      await onAfterDbChange();
      await load();
      toast.success(t('dataStorage.toasts.switched', { path: folder }));
      closeModal();
    } catch (err) {
      toast.error(t('dataStorage.errors.change', { error: extractErrorMessage(err) }));
    } finally {
      setActionLoading(false);
    }
  };

  const handleLocationChoiceAction = async (action: string, folder: string, isReset: boolean) => {
    setActionLoading(true);
    try {
      if (isReset) {
        await resetDbLocation(action);
      } else {
        await changeDbLocation(folder, action);
      }
      await onAfterDbChange();
      await load();
      if (isReset) {
        toast.success(t('dataStorage.toasts.reset'));
      } else if (action === 'move') {
        toast.success(t('dataStorage.toasts.moved', { path: folder }));
      } else {
        toast.success(t('dataStorage.toasts.fresh', { path: folder }));
      }
      closeModal();
    } catch (err) {
      toast.error(
        isReset
          ? t('dataStorage.errors.reset', { error: extractErrorMessage(err) })
          : t('dataStorage.errors.change', { error: extractErrorMessage(err) }),
      );
    } finally {
      setActionLoading(false);
    }
  };

  const handleConfirmReset = async () => {
    setActionLoading(true);
    try {
      await resetDbLocation('switch');
      await onAfterDbChange();
      await load();
      toast.success(t('dataStorage.toasts.reset'));
      closeModal();
    } catch (err) {
      toast.error(t('dataStorage.errors.reset', { error: extractErrorMessage(err) }));
    } finally {
      setActionLoading(false);
    }
  };

  return {
    path,
    isDefault,
    actionLoading,
    load,
    handleChange,
    handleReset,
    handleConfirmSwitch,
    handleLocationChoiceAction,
    handleConfirmReset,
  };
}
