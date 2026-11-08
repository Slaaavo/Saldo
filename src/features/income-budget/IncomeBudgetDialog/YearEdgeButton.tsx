import { useTranslation } from 'react-i18next'

interface Props {
  year: number
  disabled: boolean
  onClick: () => void
}

export const YearEdgeButton = ({ year, disabled, onClick }: Props) => {
  const { t } = useTranslation()
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={t('incomeBudget.table.addYear', { year })}
      className="flex w-12 flex-col items-center justify-center gap-1 rounded-md border border-dashed border-border py-2 font-semibold text-muted-foreground transition-colors hover:border-primary hover:bg-accent hover:text-primary disabled:pointer-events-none disabled:opacity-40"
    >
      <span aria-hidden="true" className="text-xl leading-none">
        +
      </span>
      <span className="text-[11px] tabular-nums">{year}</span>
    </button>
  )
}
