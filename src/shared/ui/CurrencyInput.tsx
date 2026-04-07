import * as React from 'react'
import { cn } from '../lib/utils'
import { Input } from './input'

interface CurrencyInputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  currencyCode?: string
}

const CurrencyInput = React.forwardRef<HTMLInputElement, CurrencyInputProps>(({ currencyCode, className, onWheel, ...props }, ref) => {
  const handleWheel = (e: React.WheelEvent<HTMLInputElement>) => {
    e.currentTarget.blur()
    onWheel?.(e)
  }
  if (!currencyCode) {
    return <Input className={className} ref={ref} onWheel={handleWheel} {...props} />
  }
  return (
    <div className="relative">
      <Input className={cn('pr-12', className)} ref={ref} onWheel={handleWheel} {...props} />
      <span className="absolute right-3 top-1/2 -translate-y-1/2 text-sm text-muted-foreground pointer-events-none select-none">{currencyCode}</span>
    </div>
  )
})
CurrencyInput.displayName = 'CurrencyInput'

export { CurrencyInput }
