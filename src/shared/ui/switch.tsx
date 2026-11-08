import type { ButtonHTMLAttributes } from 'react'
import { cn } from '@/shared/lib/utils'

const SIZES = {
  sm: { track: 'h-4 w-7', thumb: 'h-3 w-3', on: 'translate-x-3' },
  default: { track: 'h-5 w-9', thumb: 'h-4 w-4', on: 'translate-x-4' },
} as const

interface SwitchProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'type' | 'role' | 'onClick' | 'onChange'> {
  checked: boolean
  onCheckedChange: (checked: boolean) => void
  size?: keyof typeof SIZES
}

export const Switch = ({ checked, onCheckedChange, size = 'default', className, ...props }: SwitchProps) => {
  const dimensions = SIZES[size]
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onCheckedChange(!checked)}
      className={cn(
        'inline-flex shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50',
        checked ? 'bg-primary' : 'bg-input',
        dimensions.track,
        className,
      )}
      {...props}
    >
      <span className={cn('pointer-events-none block rounded-full bg-background shadow transition-transform', dimensions.thumb, checked ? dimensions.on : 'translate-x-0')} />
    </button>
  )
}
