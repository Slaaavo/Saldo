import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { SnapshotRow } from '../../shared/types'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { Checkbox } from '../../shared/ui/checkbox'

interface Props {
  accounts: SnapshotRow[]
  currentExclusions: number[]
  onSave: (excludedIds: number[]) => void
  onClose: () => void
}

type CheckedState = boolean | 'indeterminate'

const computeSectionState = (sectionIds: number[], excluded: Set<number>): CheckedState => {
  if (sectionIds.length === 0) return true
  const includedCount = sectionIds.filter((id) => !excluded.has(id)).length
  if (includedCount === sectionIds.length) return true
  if (includedCount === 0) return false
  return 'indeterminate'
}

const BulkUpdateVisibilityModal = ({ accounts, currentExclusions, onSave, onClose }: Props) => {
  const { t } = useTranslation()
  const [excluded, setExcluded] = useState<Set<number>>(() => new Set(currentExclusions))

  const realAccounts = accounts.filter((r) => r.accountType === 'account')
  const bucketAccounts = accounts.filter((r) => r.accountType === 'bucket')
  const assetAccounts = accounts.filter((r) => r.accountType === 'asset')

  const allIds = accounts.map((r) => r.accountId)

  const selectAllState: CheckedState = (() => {
    const includedCount = allIds.filter((id) => !excluded.has(id)).length
    if (includedCount === allIds.length) return true
    if (includedCount === 0) return false
    return 'indeterminate'
  })()

  const toggleItem = (id: number) => {
    setExcluded((prev) => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })
  }

  const toggleSection = (sectionIds: number[], state: CheckedState) => {
    setExcluded((prev) => {
      const next = new Set(prev)
      if (state === true) {
        // All included → exclude all
        sectionIds.forEach((id) => next.add(id))
      } else {
        // Some or none included → include all
        sectionIds.forEach((id) => next.delete(id))
      }
      return next
    })
  }

  const toggleSelectAll = (state: CheckedState) => {
    setExcluded(() => {
      if (state === true) {
        // All included → exclude all
        return new Set(allIds)
      } else {
        // Some or none included → include all
        return new Set()
      }
    })
  }

  const handleSave = () => {
    onSave(Array.from(excluded))
  }

  const renderSection = (sectionAccounts: SnapshotRow[], label: string) => {
    if (sectionAccounts.length === 0) return null
    const sectionIds = sectionAccounts.map((r) => r.accountId)
    const sectionState = computeSectionState(sectionIds, excluded)

    return (
      <div className="flex flex-col gap-2">
        <div className="flex items-center gap-2">
          <Checkbox checked={sectionState} onCheckedChange={() => toggleSection(sectionIds, sectionState)} />
          <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">{label}</span>
        </div>
        {sectionAccounts.map((row) => (
          <div key={row.accountId} className="flex items-center gap-2 pl-6">
            <Checkbox checked={!excluded.has(row.accountId)} onCheckedChange={() => toggleItem(row.accountId)} />
            <span className="text-sm">{row.accountName}</span>
            <span className="text-xs text-muted-foreground">{row.currencyCode}</span>
          </div>
        ))}
      </div>
    )
  }

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('modals.bulkUpdateVisibility.title')}</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            <div className="flex items-center gap-2">
              <Checkbox checked={selectAllState} onCheckedChange={() => toggleSelectAll(selectAllState)} />
              <span className="text-sm font-medium">{t('modals.bulkUpdateVisibility.selectAll')}</span>
            </div>
            <hr className="border-border" />
            {renderSection(realAccounts, t('modals.bulkUpdateVisibility.accountsSection'))}
            {renderSection(bucketAccounts, t('modals.bulkUpdateVisibility.bucketsSection'))}
            {renderSection(assetAccounts, t('modals.bulkUpdateVisibility.assetsSection'))}
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.bulkUpdateVisibility.cancel')}
            </Button>
            <Button type="button" onClick={handleSave}>
              {t('modals.bulkUpdateVisibility.save')}
            </Button>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  )
}

export default BulkUpdateVisibilityModal
