import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { enterDemoMode, exitDemoMode, isDemoMode as checkIsDemoMode } from '../../shared/api';
import { extractErrorMessage } from '../../shared/utils/errors';

interface UseDemoModeOptions {
  refresh: () => Promise<void>;
  loadDbLocation: () => Promise<void>;
  onEntered: () => void;
}

export function useDemoMode({ refresh, loadDbLocation, onEntered }: UseDemoModeOptions) {
  const { t } = useTranslation();
  const [isDemoMode, setIsDemoMode] = useState(false);

  useEffect(() => {
    checkIsDemoMode().then(setIsDemoMode).catch(console.error);
  }, []);

  const handleEnter = async () => {
    try {
      await enterDemoMode();
      setIsDemoMode(true);
      await refresh();
      await loadDbLocation();
      onEntered();
    } catch (err) {
      toast.error(extractErrorMessage(err));
    }
  };

  const handleExit = async () => {
    try {
      await exitDemoMode();
      setIsDemoMode(false);
      await refresh();
      await loadDbLocation();
      toast.success(t('demo.exitedToast'));
    } catch (err) {
      toast.error(extractErrorMessage(err));
    }
  };

  return { isDemoMode, handleEnter, handleExit };
}
