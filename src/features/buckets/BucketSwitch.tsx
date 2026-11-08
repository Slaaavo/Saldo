import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { setBucketEnabled } from '@/shared/api'
import { Switch } from '@/shared/ui/switch'
import { extractErrorMessage } from '@/shared/utils/errors'

interface Props {
  bucketId: number
  bucketName: string
  enabled: boolean
}

const BucketSwitch = ({ bucketId, bucketName, enabled }: Props) => {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [requested, setRequested] = useState<boolean | null>(null)

  const toggle = async (next: boolean) => {
    setRequested(next)
    try {
      await setBucketEnabled(bucketId, next)
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
    } catch (err) {
      toast.error(t('errors.switchBucket', { error: extractErrorMessage(err) }))
    }
    setRequested(null)
  }

  return <Switch size="sm" checked={requested ?? enabled} onCheckedChange={(next) => void toggle(next)} aria-label={t('buckets.countInLeftToSpend', { name: bucketName })} />
}

export default BucketSwitch
