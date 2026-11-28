import { useState } from 'react'
import { Trans, useTranslation } from 'react-i18next'
import type { SnapshotRow } from '@/shared/types'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '@/shared/ui/dialog'
import { Button } from '@/shared/ui/button'
import { CurrencyInput } from '@/shared/ui/CurrencyInput'
import { Label } from '@/shared/ui/label'
import NumberValue from '@/shared/ui/NumberValue'
import { getMinorUnitsStep, toMinorUnits } from '@/shared/utils/format'
import DifferenceCauses from './DifferenceCauses'
import { useBookUnexplainedDifference, useConfirmBankMatch } from './useBankCheckMutations'
import { useDifferenceCauses } from './useDifferenceCauses'

interface Props {
  row: SnapshotRow
  onClose: () => void
}

const DifferenceSentence = ({ row, differenceMinor }: { row: SnapshotRow; differenceMinor: number }) => {
  const { t } = useTranslation()
  if (differenceMinor === 0) {
    return <>{t('bankCheck.dialog.same')}</>
  }
  const amount = <NumberValue value={Math.abs(differenceMinor)} currencyCode={row.currencyCode} minorUnits={row.currencyMinorUnits} className="font-semibold" />
  return <Trans i18nKey={differenceMinor > 0 ? 'bankCheck.dialog.lower' : 'bankCheck.dialog.higher'} components={{ amount }} />
}

const FindBankDifferenceModal = ({ row, onClose }: Props) => {
  const { t } = useTranslation()
  const [bankBalance, setBankBalance] = useState('')
  const confirmMatch = useConfirmBankMatch()
  const book = useBookUnexplainedDifference()

  const parsedMinor = toMinorUnits(bankBalance, row.currencyMinorUnits)
  const bankBalanceMinor = Number.isFinite(parsedMinor) ? parsedMinor : null
  const differenceMinor = bankBalanceMinor === null ? null : bankBalanceMinor - row.balanceMinor
  const causes = useDifferenceCauses(row.accountId, bankBalanceMinor, row.balanceMinor)
  const pending = confirmMatch.isPending || book.isPending

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (bankBalanceMinor === null || pending) return
    const seen = { accountId: row.accountId, seenBalanceMinor: row.balanceMinor }
    if (differenceMinor === 0) {
      confirmMatch.mutate(seen, { onSuccess: onClose })
    } else {
      book.mutate({ ...seen, bankBalanceMinor }, { onSuccess: onClose })
    }
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
          <DialogTitle>{t('bankCheck.dialog.title', { name: row.accountName })}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            <div className="flex items-center justify-between py-2 px-3 rounded-md bg-muted text-sm">
              <span className="text-muted-foreground">{t('bankCheck.dialog.saldoBalance')}</span>
              <NumberValue value={row.balanceMinor} currencyCode={row.currencyCode} minorUnits={row.currencyMinorUnits} className="font-semibold" />
            </div>

            <div className="flex flex-col gap-2">
              <Label htmlFor="bank-balance">{t('bankCheck.dialog.bankBalance')}</Label>
              <CurrencyInput
                id="bank-balance"
                type="number"
                step={getMinorUnitsStep(row.currencyMinorUnits)}
                value={bankBalance}
                onChange={(e) => setBankBalance(e.target.value)}
                currencyCode={row.currencyCode}
                autoFocus
              />
            </div>

            <p className="min-h-5 text-sm text-muted-foreground" aria-live="polite">
              {differenceMinor !== null && <DifferenceSentence row={row} differenceMinor={differenceMinor} />}
            </p>

            {differenceMinor !== null && differenceMinor !== 0 && causes && <DifferenceCauses row={row} causes={causes} onChecked={onClose} />}
          </DialogBody>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('bankCheck.dialog.cancel')}
            </Button>
            <Button type="submit" disabled={differenceMinor === null || pending}>
              {t(differenceMinor === 0 ? 'bankCheck.dialog.markAsMatching' : 'bankCheck.dialog.book')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default FindBankDifferenceModal
