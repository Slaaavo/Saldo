import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CalendarIcon, X } from 'lucide-react';

import { cn } from '@/shared/lib/utils';
import { formatDisplayDate, todayIso } from '@/shared/utils/format';
import { Button } from './button';
import { Calendar } from './calendar';
import { Label } from './label';
import { Popover, PopoverContent, PopoverTrigger } from './popover';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './select';

interface DatePickerProps {
  value: string | undefined;
  onChange: (date: string) => void;
  placeholder?: string;
  id?: string;
  className?: string;
  clearable?: boolean;
  withTime?: boolean;
  defaultTime?: string;
}

function parseDateOnly(value: string): Date {
  const datePart = value.substring(0, 10);
  const [y, m, d] = datePart.split('-').map(Number);
  return new Date(y, m - 1, d, 12, 0, 0);
}

function toIsoString(date: Date): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, '0');
  const d = String(date.getDate()).padStart(2, '0');
  return `${y}-${m}-${d}`;
}

function combineDateTime(date: Date, h: string, m: string): string {
  const y = date.getFullYear();
  const mo = String(date.getMonth() + 1).padStart(2, '0');
  const d = String(date.getDate()).padStart(2, '0');
  return `${y}-${mo}-${d}T${h}:${m}:00`;
}

function parseTimePart(value: string | undefined, defaultTime: string): { h: string; m: string } {
  if (value && value.length >= 16) {
    const [h, min] = value.substring(11, 16).split(':');
    return { h: h.padStart(2, '0'), m: min.padStart(2, '0') };
  }
  const [h, min] = defaultTime.split(':');
  return { h: h.padStart(2, '0'), m: min.padStart(2, '0') };
}

const HOURS = Array.from({ length: 24 }, (_, i) => String(i).padStart(2, '0'));
const MINUTES = Array.from({ length: 60 }, (_, i) => String(i).padStart(2, '0'));

export function DatePicker({
  value,
  onChange,
  placeholder = 'Pick a date',
  id,
  className,
  clearable = false,
  withTime = false,
  defaultTime = '23:59',
}: DatePickerProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  const selected = value ? parseDateOnly(value) : undefined;

  const { h: initH, m: initM } = parseTimePart(value, defaultTime);
  const [hour, setHour] = useState(initH);
  const [minute, setMinute] = useState(initM);

  const handleDateSelect = (date: Date | undefined) => {
    if (!date) return;
    if (withTime) {
      onChange(combineDateTime(date, hour, minute));
      // Don't auto-close when withTime is true — user closes manually
    } else {
      onChange(toIsoString(date));
      setOpen(false);
    }
  };

  const handleHourChange = (h: string) => {
    setHour(h);
    if (selected) {
      onChange(combineDateTime(selected, h, minute));
    }
  };

  const handleMinuteChange = (m: string) => {
    setMinute(m);
    if (selected) {
      onChange(combineDateTime(selected, hour, m));
    }
  };

  const handleTodayClick = () => {
    const today = parseDateOnly(todayIso());
    if (withTime) {
      onChange(combineDateTime(today, hour, minute));
      // Don't auto-close
    } else {
      onChange(todayIso());
      setOpen(false);
    }
  };

  const displayDate = value ? formatDisplayDate(value.substring(0, 10)) : undefined;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          id={id}
          variant="outline"
          className={cn(
            'justify-start text-left font-normal',
            !value && 'text-muted-foreground',
            className,
          )}
        >
          {displayDate ? displayDate : <span>{placeholder}</span>}
          {clearable && value ? (
            <span
              role="button"
              aria-label="Clear date"
              tabIndex={0}
              className="ml-auto opacity-50 hover:opacity-100"
              onClick={(e) => {
                e.stopPropagation();
                onChange('');
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.stopPropagation();
                  onChange('');
                }
              }}
            >
              <X className="size-4" />
            </span>
          ) : (
            <CalendarIcon className="ml-auto size-4 opacity-50" />
          )}
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-auto p-0">
        <Calendar
          mode="single"
          selected={selected}
          defaultMonth={selected}
          onSelect={handleDateSelect}
        />
        {withTime && (
          <div className="border-t border-border px-3 py-3 flex gap-3 items-end">
            <div className="flex flex-col gap-1.5 flex-1">
              <Label className="text-xs">{t('datePicker.hour')}</Label>
              <Select value={hour} onValueChange={handleHourChange}>
                <SelectTrigger className="h-8 text-sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {HOURS.map((h) => (
                    <SelectItem key={h} value={h}>
                      {h}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="flex flex-col gap-1.5 flex-1">
              <Label className="text-xs">{t('datePicker.minute')}</Label>
              <Select value={minute} onValueChange={handleMinuteChange}>
                <SelectTrigger className="h-8 text-sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {MINUTES.map((m) => (
                    <SelectItem key={m} value={m}>
                      {m}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
        )}
        <div className="border-t border-border px-3 py-2">
          <Button variant="ghost" size="sm" className="w-full text-sm" onClick={handleTodayClick}>
            {t('datePicker.today')}
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
