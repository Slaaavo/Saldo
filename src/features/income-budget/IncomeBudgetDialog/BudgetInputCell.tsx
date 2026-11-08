import type { PointerEvent } from 'react'
import { cn } from '@/shared/lib/utils'

interface Props {
  text: string
  label: string
  fillLabel: string
  invalid: boolean
  placeholder: string | undefined
  className: string | undefined
  inFill: boolean
  isFillOrigin: boolean
  onChange: (text: string) => void
  onFillStart: (event: PointerEvent<HTMLButtonElement>) => void
}

export const BudgetInputCell = ({ text, label, fillLabel, invalid, placeholder, className, inFill, isFillOrigin, onChange, onFillStart }: Props) => (
  <td
    className={cn(
      'group relative border-b border-border p-0',
      invalid ? 'ring-2 ring-inset ring-destructive' : 'focus-within:ring-2 focus-within:ring-inset focus-within:ring-primary',
      className,
      inFill && 'bg-primary/15',
    )}
  >
    <input
      value={text}
      onChange={(event) => onChange(event.target.value)}
      aria-label={label}
      aria-invalid={invalid || undefined}
      inputMode="decimal"
      autoComplete="off"
      spellCheck={false}
      placeholder={placeholder}
      className={cn(
        'block w-full min-w-[92px] bg-transparent px-2.5 py-2.5 text-right tabular-nums outline-none placeholder:text-muted-foreground/60',
        invalid && 'text-destructive',
      )}
    />
    <button
      type="button"
      tabIndex={-1}
      aria-label={fillLabel}
      onPointerDown={onFillStart}
      onMouseDown={(event) => event.preventDefault()}
      className={cn(
        "absolute right-px bottom-px z-10 h-3 w-3 cursor-ew-resize touch-none rounded-[3px] border-2 border-card bg-primary after:absolute after:-inset-[7px] after:content-['']",
        isFillOrigin ? 'opacity-100' : 'pointer-events-none opacity-0 group-focus-within:pointer-events-auto group-focus-within:opacity-100',
      )}
    />
  </td>
)
