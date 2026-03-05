import { useState, useEffect } from 'react';
import { Input } from './ui/input';
import { Label } from './ui/label';

interface Props {
  selectedDate: string;
  onDateChange: (date: string) => void;
}

export default function Header({ selectedDate, onDateChange }: Props) {
  const [localDate, setLocalDate] = useState(selectedDate);

  // Sync local state if parent changes selectedDate externally
  useEffect(() => {
    setLocalDate(selectedDate);
  }, [selectedDate]);

  const commitDate = () => {
    if (localDate && localDate !== selectedDate) {
      onDateChange(localDate);
    }
  };

  return (
    <header className="flex items-center justify-between rounded-t-xl border-b bg-card px-6 py-3">
      <h1 className="text-xl font-bold tracking-tight">Our Finances</h1>
      <div className="flex items-center gap-2">
        <Label htmlFor="date-picker" className="text-sm font-medium text-muted-foreground">
          Date
        </Label>
        <Input
          id="date-picker"
          type="date"
          value={localDate}
          onChange={(e) => setLocalDate(e.target.value)}
          onBlur={commitDate}
          onKeyDown={(e) => {
            if (e.key === 'Enter') commitDate();
          }}
          className="w-40"
        />
      </div>
    </header>
  );
}
