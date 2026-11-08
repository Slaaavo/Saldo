import { useEffect } from 'react'
import { useQuery } from '@tanstack/react-query'
import { listPersons } from '@/shared/api'
import { useTimeMachine } from '@/app/useTimeMachine'
import { useSnapshotQuery } from '@/shared/hooks/useSnapshotQuery'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/shared/ui/dialog'
import { useIncomeBudget } from '../useIncomeBudgetQueries'
import { BudgetEditor } from './BudgetEditor'

interface Props {
  bucketId: number
  onClose: () => void
}

const IncomeBudgetDialog = ({ bucketId, onClose }: Props) => {
  const { asOf } = useTimeMachine()
  const snapshotQuery = useSnapshotQuery(asOf)
  const row = snapshotQuery.data?.rows.find((r) => r.accountId === bucketId)
  const rowGone = snapshotQuery.data !== undefined && row === undefined
  const { data: budget } = useIncomeBudget(bucketId, asOf)
  const { data: persons } = useQuery({ queryKey: ['persons'], queryFn: listPersons })
  const personName = persons?.find((person) => person.id === row?.personId)?.name ?? ''

  useEffect(() => {
    if (rowGone) onClose()
  }, [rowGone, onClose])

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <DialogContent className="max-w-5xl" aria-describedby={undefined}>
        <DialogHeader>
          <DialogTitle className="flex flex-wrap items-center gap-2">
            {row?.accountName}
            {personName && <span className="rounded-full bg-accent px-2 py-0.5 text-xs font-semibold text-accent-foreground">{personName}</span>}
          </DialogTitle>
        </DialogHeader>
        {row && budget ? <BudgetEditor row={row} budget={budget} asOf={asOf} personName={personName} onClose={onClose} /> : <div className="h-96" />}
      </DialogContent>
    </Dialog>
  )
}

export default IncomeBudgetDialog
