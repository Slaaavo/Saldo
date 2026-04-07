import * as React from 'react'
import { cn } from '../lib/utils'
import { Input } from './input'

const PercentageInput = React.forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(({ className, onWheel, ...props }, ref) => {
  const handleWheel = (e: React.WheelEvent<HTMLInputElement>) => {
    e.currentTarget.blur()
    onWheel?.(e)
  }
  return (
    <div className="relative">
      <Input className={cn('pr-12', className)} ref={ref} onWheel={handleWheel} {...props} />
      <span className="absolute right-3 top-1/2 -translate-y-1/2 text-sm text-muted-foreground pointer-events-none select-none">%</span>
    </div>
  )
})
PercentageInput.displayName = 'PercentageInput'

export { PercentageInput }
