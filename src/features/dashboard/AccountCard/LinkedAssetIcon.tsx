import { useTranslation } from 'react-i18next'
import type { SnapshotRow } from '../../../shared/types'
import { Link2 } from 'lucide-react'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '../../../shared/ui/tooltip'

interface LinkedAssetIconProps {
  linkedAssetIds: number[]
  allAssets: SnapshotRow[]
}

const LinkedAssetIcon = ({ linkedAssetIds, allAssets }: LinkedAssetIconProps) => {
  const { t } = useTranslation()

  if (!linkedAssetIds?.length || !allAssets?.length) {
    return null
  }

  const linkedAssetNames = allAssets.filter((a) => linkedAssetIds.includes(a.accountId)).map((a) => a.accountName)

  return (
    <TooltipProvider delayDuration={300}>
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="shrink-0 cursor-help">
            <Link2 className="h-3.5 w-3.5 text-muted-foreground" />
          </span>
        </TooltipTrigger>
        <TooltipContent>
          <p className="text-xs">
            {t('accounts.linkedAssetTooltip', {
              assets: linkedAssetNames.join(', '),
            })}
          </p>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}

export { LinkedAssetIcon }
