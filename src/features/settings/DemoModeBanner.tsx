import { useTranslation } from 'react-i18next';
import { Button } from '../../shared/ui/button';

interface Props {
  onExit: () => void;
}

export default function DemoModeBanner({ onExit }: Props) {
  const { t } = useTranslation();

  return (
    <div className="flex items-center justify-between border-b border-amber-300 dark:border-amber-700/50 bg-amber-100 dark:bg-amber-900/40 px-4 py-2">
      <div className="flex items-center gap-3">
        <span className="text-xs font-bold uppercase tracking-wider text-amber-800 dark:text-amber-300">
          {t('demo.bannerLabel')}
        </span>
        <span className="text-xs text-amber-700 dark:text-amber-400">
          {t('demo.bannerMessage')}
        </span>
      </div>
      <Button
        variant="outline"
        size="sm"
        onClick={onExit}
        className="border-amber-400 text-amber-800 hover:bg-amber-200 dark:border-amber-600 dark:text-amber-300 dark:hover:bg-amber-800/40"
      >
        {t('demo.exitButton')}
      </Button>
    </div>
  );
}
