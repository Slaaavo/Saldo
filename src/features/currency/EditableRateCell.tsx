import type { FxRateRow } from '../../shared/types';
import { cn } from '@/shared/lib/utils';
import { formatRate } from './fxRate';

interface EditableRateCellProps {
  date: string;
  code: string;
  row: FxRateRow | undefined;
  isEditing: boolean;
  editValue: string;
  onCellClick: (date: string, code: string, row: FxRateRow | undefined) => void;
  onEditValueChange: (value: string) => void;
  onSave: (date: string, code: string, value: string) => void;
  onCancel: () => void;
  manualLabel: string;
}

export function EditableRateCell({
  date,
  code,
  row,
  isEditing,
  editValue,
  onCellClick,
  onEditValueChange,
  onSave,
  onCancel,
  manualLabel,
}: EditableRateCellProps) {
  return (
    <td
      className={cn(
        'text-right py-1 px-3 font-mono text-sm cursor-pointer',
        row?.isManual && 'font-bold bg-amber-50 dark:bg-amber-900/20',
      )}
      onClick={() => {
        if (!isEditing) onCellClick(date, code, row);
      }}
    >
      {isEditing ? (
        <input
          autoFocus
          type="text"
          value={editValue}
          onChange={(e) => onEditValueChange(e.target.value)}
          onBlur={(e) => onSave(date, code, e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              onSave(date, code, editValue);
            } else if (e.key === 'Escape') {
              onCancel();
            }
          }}
          className="w-24 text-right font-mono text-sm border border-border rounded px-1 bg-background"
        />
      ) : (
        <>
          {row ? formatRate(row) : '—'}
          {row?.isManual && <span className="ml-1 text-amber-600 text-xs">({manualLabel})</span>}
        </>
      )}
    </td>
  );
}
