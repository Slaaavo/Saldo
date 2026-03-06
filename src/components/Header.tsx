import { useTranslation } from 'react-i18next';
import { Settings } from 'lucide-react';
import { DatePicker } from './ui/date-picker';
import { Button } from './ui/button';
import { Label } from './ui/label';
import LanguageSelector from './LanguageSelector';

interface Props {
  selectedDate: string;
  onDateChange: (date: string) => void;
  currentView: 'dashboard' | 'fx-rates';
  onNavigate: (view: 'dashboard' | 'fx-rates') => void;
  onOpenSettings: () => void;
}

export default function Header({
  selectedDate,
  onDateChange,
  currentView,
  onNavigate,
  onOpenSettings,
}: Props) {
  const { t } = useTranslation();

  return (
    <header className="flex items-center justify-between rounded-t-xl border-b bg-card px-6 py-3">
      <div className="flex items-center gap-3">
        <h1 className="text-xl font-bold tracking-tight">{t('header.title')}</h1>
        <LanguageSelector />
      </div>
      <div className="flex items-center gap-2">
        <Button
          variant={currentView === 'fx-rates' ? 'default' : 'ghost'}
          size="sm"
          onClick={() => onNavigate(currentView === 'fx-rates' ? 'dashboard' : 'fx-rates')}
        >
          {t('header.fxRates')}
        </Button>
        <Button
          variant="ghost"
          size="icon"
          onClick={onOpenSettings}
          title={t('header.settings')}
          aria-label={t('header.settings')}
        >
          <Settings className="h-4 w-4" />
        </Button>
        {currentView === 'dashboard' && (
          <>
            <Label htmlFor="date-picker" className="text-sm font-medium text-muted-foreground">
              {t('header.date')}
            </Label>
            <DatePicker
              id="date-picker"
              value={selectedDate}
              onChange={onDateChange}
              className="w-48"
            />
          </>
        )}
      </div>
    </header>
  );
}
