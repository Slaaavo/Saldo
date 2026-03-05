import { DayPicker, type DayPickerProps } from 'react-day-picker';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { cn } from '@/lib/utils';

// The Calendar component props are the DayPicker props
type CalendarProps = DayPickerProps;

function Calendar({ className, classNames, ...props }: CalendarProps) {
  return (
    <DayPicker
      weekStartsOn={1}
      className={cn('p-3 relative', className)}
      classNames={{
        months: 'flex flex-col sm:flex-row gap-2',
        month: 'flex flex-col gap-4',
        month_caption: 'flex items-center h-7',
        caption_label: 'text-sm font-bold',
        nav: 'absolute top-3 right-3 flex items-center gap-1 h-7',
        button_previous:
          'size-7 bg-transparent hover:bg-accent rounded-[var(--radius)] inline-flex items-center justify-center cursor-pointer',
        button_next:
          'size-7 bg-transparent hover:bg-accent rounded-[var(--radius)] inline-flex items-center justify-center cursor-pointer',
        month_grid: 'w-full border-collapse space-y-1',
        weekdays: 'flex',
        weekday: 'text-muted-foreground rounded-[var(--radius)] w-9 font-normal text-[0.8rem]',
        week: 'flex w-full mt-2',
        day: 'h-9 w-9 text-center text-sm p-0 relative',
        day_button:
          'h-9 w-9 p-0 font-normal rounded-[var(--radius)] hover:bg-accent hover:text-accent-foreground cursor-pointer inline-flex items-center justify-center',
        selected:
          'bg-primary text-primary-foreground hover:bg-primary hover:text-primary-foreground focus:bg-primary focus:text-primary-foreground rounded-[var(--radius)]',
        today: 'bg-accent text-accent-foreground rounded-[var(--radius)]',
        outside: 'text-muted-foreground/50 aria-selected:text-muted-foreground/50',
        disabled: 'text-muted-foreground/50',
        hidden: 'invisible',
        ...classNames,
      }}
      components={{
        Chevron: ({ orientation, ...chevronProps }) => {
          const Icon = orientation === 'left' ? ChevronLeft : ChevronRight;
          return <Icon className="size-4" {...chevronProps} />;
        },
      }}
      {...props}
    />
  );
}
Calendar.displayName = 'Calendar';

export { Calendar };
export type { CalendarProps };
