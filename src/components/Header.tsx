import { useTranslation } from 'react-i18next';
import { DatePicker } from './ui/date-picker';
import { Label } from './ui/label';
import LanguageSelector from './LanguageSelector';

interface Props {
  selectedDate: string;
  onDateChange: (date: string) => void;
}

export default function Header({ selectedDate, onDateChange }: Props) {
  const { t } = useTranslation();

  return (
    <header className="flex items-center justify-between rounded-t-xl border-b bg-card px-6 py-3">
      <div className="flex items-center gap-3">
        <h1 className="text-xl font-bold tracking-tight">{t('header.title')}</h1>
        <LanguageSelector />
      </div>
      <div className="flex items-center gap-2">
        <Label htmlFor="date-picker" className="text-sm font-medium text-muted-foreground">
          {t('header.date')}
        </Label>
        <DatePicker
          id="date-picker"
          value={selectedDate}
          onChange={onDateChange}
          className="w-48"
        />
      </div>
    </header>
  );
}
