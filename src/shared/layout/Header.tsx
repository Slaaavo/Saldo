import { useTranslation } from 'react-i18next';
import { DatePicker } from '../ui/date-picker';
import { Label } from '../ui/label';

interface Props {
  pageTitle: string;
  selectedDate: string;
  onDateChange: (date: string) => void;
  showDatePicker: boolean;
}

export default function Header({ pageTitle, selectedDate, onDateChange, showDatePicker }: Props) {
  const { t } = useTranslation();

  return (
    <header className="flex items-center justify-between border-b bg-card px-4 md:px-10 py-3 min-h-16">
      <div className="flex items-center">
        <h1 className="text-xl font-bold tracking-tight">{pageTitle}</h1>
      </div>
      <div className="flex items-center gap-2">
        {showDatePicker && (
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
