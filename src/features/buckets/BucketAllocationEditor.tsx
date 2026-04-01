import { useTranslation } from 'react-i18next'
import type { SnapshotRow } from '../../shared/types'
import type { LinkRow } from './useBucketLinks'
import { Button } from '../../shared/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../shared/ui/select'

interface Props {
  visibleLinks: LinkRow[]
  availableToLink: SnapshotRow[]
  allAccounts: SnapshotRow[]
  loadingLinks: boolean
  constraintError: string | null
  handleSourceAccountSelect: (tempId: string, sourceAccountId: number) => void
  handleAddLink: () => void
  handleRemoveNew: (tempId: string) => void
  handleUnlink: (tempId: string) => void
}

export default function BucketAllocationEditor({
  visibleLinks,
  availableToLink,
  allAccounts,
  loadingLinks,
  constraintError,
  handleSourceAccountSelect,
  handleAddLink,
  handleRemoveNew,
  handleUnlink,
}: Props) {
  const { t } = useTranslation()

  const hasAccounts = allAccounts.some((a) => a.accountType === 'account')

  return (
    <>
      <hr className="border-border" />
      <div className="flex flex-col gap-3">
        <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">{t('modals.createBalanceUpdate.linkedAccounts')}</p>

        {loadingLinks ? (
          <p className="text-sm text-muted-foreground">…</p>
        ) : (
          <>
            {visibleLinks.map((link) => {
              const sourceAccount = allAccounts.find((a) => a.accountId === link.sourceAccountId)
              return (
                <div key={link.tempId} className="flex flex-col gap-2 rounded-md border border-border bg-muted/30 p-3">
                  {link.isNew && link.sourceAccountId === null ? (
                    <>
                      <Select value="" onValueChange={(val) => handleSourceAccountSelect(link.tempId, Number(val))}>
                        <SelectTrigger>
                          <SelectValue placeholder={t('modals.createBalanceUpdate.selectSourceAccount')} />
                        </SelectTrigger>
                        <SelectContent>
                          {availableToLink.map((a) => (
                            <SelectItem key={a.accountId} value={String(a.accountId)}>
                              {a.accountName}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                      <div className="flex justify-end">
                        <Button type="button" variant="ghost" size="sm" onClick={() => handleRemoveNew(link.tempId)}>
                          ×
                        </Button>
                      </div>
                    </>
                  ) : (
                    <>
                      <p className="text-sm font-medium">{sourceAccount?.accountName ?? String(link.sourceAccountId)}</p>
                      <p className="text-xs text-muted-foreground">{t('modals.allocation.linkDescription')}</p>
                      <div className="flex justify-end">
                        <Button type="button" variant="outline" size="sm" onClick={() => handleUnlink(link.tempId)}>
                          {t('modals.createBalanceUpdate.unlinkAccount')}
                        </Button>
                      </div>
                    </>
                  )}
                </div>
              )
            })}

            {availableToLink.length > 0 ? (
              <Button type="button" variant="outline" size="sm" onClick={handleAddLink} className="self-start">
                {t('modals.createBalanceUpdate.linkAccount')}
              </Button>
            ) : (
              !hasAccounts && <p className="text-sm text-muted-foreground">{t('modals.createBalanceUpdate.noAccountsToLink')}</p>
            )}

            {constraintError !== null && <p className="text-xs text-destructive mt-2">{constraintError}</p>}
          </>
        )}
      </div>
    </>
  )
}
