import { useRef } from 'react'
import type { FxRateRow } from '../../shared/types'
import { cn } from '@/shared/lib/utils'
import { formatRate } from './fxRate'

interface EditableRateCellProps {
  date: string
  code: string
  row: FxRateRow | undefined
  isEditing: boolean
  editValue: string
  onCellClick: (date: string, code: string, row: FxRateRow | undefined) => void
  onEditValueChange: (value: string) => void
  onSave: (date: string, code: string, value: string) => void
  onCancel: () => void
  manualLabel: string
}

interface RateInputProps {
  editValue: string
  onEditValueChange: (value: string) => void
  onSave: (value: string) => void
  onCancel: () => void
}

const RateInput = ({ editValue, onEditValueChange, onSave, onCancel }: RateInputProps) => {
  // Enter and Escape unmount the input, and Chromium (WebView2) then fires blur on it, which must not save a second time or save after Escape.
  const finished = useRef(false)

  return (
    <input
      autoFocus
      type="text"
      value={editValue}
      onChange={(e) => onEditValueChange(e.target.value)}
      onBlur={(e) => {
        if (finished.current) return
        finished.current = true
        onSave(e.target.value)
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault()
          finished.current = true
          onSave(editValue)
        } else if (e.key === 'Escape') {
          finished.current = true
          onCancel()
        }
      }}
      className="w-24 text-right font-mono text-sm border border-border rounded px-1 bg-background"
    />
  )
}

export const EditableRateCell = ({ date, code, row, isEditing, editValue, onCellClick, onEditValueChange, onSave, onCancel, manualLabel }: EditableRateCellProps) => {
  return (
    <td
      className={cn('text-right py-1 px-3 font-mono text-sm cursor-pointer', row?.isManual && 'font-bold bg-amber-50 dark:bg-amber-900/20')}
      onClick={() => {
        if (!isEditing) onCellClick(date, code, row)
      }}
    >
      {isEditing ? (
        <RateInput editValue={editValue} onEditValueChange={onEditValueChange} onSave={(value) => onSave(date, code, value)} onCancel={onCancel} />
      ) : (
        <>
          {row ? formatRate(row) : '—'}
          {row?.isManual && <span className="ml-1 text-amber-600 text-xs">({manualLabel})</span>}
        </>
      )}
    </td>
  )
}
