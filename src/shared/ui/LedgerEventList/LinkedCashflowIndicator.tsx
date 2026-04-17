import { CheckCircle2 } from 'lucide-react'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '../tooltip'

interface Props {
  tooltip: string
}

const LinkedCashflowIndicator = ({ tooltip }: Props) => {
  return (
    <TooltipProvider delayDuration={300}>
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="inline-flex items-center cursor-default">
            <CheckCircle2 className="h-4 w-4 text-green-600 dark:text-green-400" />
          </span>
        </TooltipTrigger>
        <TooltipContent>
          <p className="text-xs">{tooltip}</p>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}

export default LinkedCashflowIndicator
