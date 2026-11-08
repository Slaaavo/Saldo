import { describe, it, expect, vi, beforeEach, afterEach, type Mock } from 'vitest'
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { ReactElement, ReactNode } from 'react'
import { getAccountsSnapshot, getConsolidationCurrency, getIncomeBudget, listPersons, previewIncomeBudget, saveIncomeBudget } from '@/shared/api'
import type { AsOf, IncomeBudget, IncomeBudgetPreview, MonthLine, SnapshotRow } from '@/shared/types'
import { SelectedPersonProvider } from '@/app/SelectedPersonContext'
import { LIVE } from '@/shared/utils/snapshot'
import { makeIncomeBudgetRow, makeRow, makeSnapshot, sameCurrency } from '@/shared/testing/snapshotFixtures'
import { monthKey, monthsOfYears } from '../month'
import IncomeBudgetDialog from '.'

vi.mock('@/shared/api', () => ({
  getAccountsSnapshot: vi.fn(),
  getIncomeBudget: vi.fn(),
  previewIncomeBudget: vi.fn(),
  saveIncomeBudget: vi.fn(),
  listPersons: vi.fn(),
  getConsolidationCurrency: vi.fn(),
}))

vi.mock('react-i18next', async () => {
  const { i18nMock } = await import('@/shared/testing/i18nMock')
  const Trans = ({ i18nKey, components }: { i18nKey: string; components: { amount: ReactElement } }) => (
    <>
      {i18nKey} {components.amount}
    </>
  )
  return { ...i18nMock, Trans }
})

vi.mock('sonner', () => ({ toast: { error: vi.fn() } }))

const timeMachine: { asOf: AsOf } = { asOf: LIVE }
vi.mock('@/app/useTimeMachine', () => ({ useTimeMachine: () => timeMachine }))

const SEP = monthKey(2026, 9)
const OCT = monthKey(2026, 10)
const NOV = monthKey(2026, 11)
const DEC = monthKey(2026, 12)

const BUCKET = makeIncomeBudgetRow({ accountId: 30, accountName: 'Consulting income', personId: 2 })

const makeBudget = (overrides?: Partial<IncomeBudget>): IncomeBudget => ({
  day: '2026-11-08',
  momentMonth: NOV,
  months: [
    { month: OCT, budgetMinor: 564000, vatBps: 2300 },
    { month: NOV, budgetMinor: 612000, vatBps: 2300 },
  ],
  countedAccounts: [{ id: 5, name: 'TB Sumbal consulting' }],
  ...overrides,
})

const LINES: Partial<Record<string, Partial<MonthLine>>> = {
  [SEP]: { budgeted: false, receivedMinor: 548100, differenceMinor: null },
  [OCT]: { receivedMinor: 585700, differenceMinor: 21700 },
  [NOV]: { receivedMinor: 0, differenceMinor: -612000 },
}

const makePreview = (fromYear: number, toYear: number): IncomeBudgetPreview => ({
  valueMinor: -590300,
  valuation: sameCurrency(-590300),
  lines: monthsOfYears(fromYear, toYear).map((month) => {
    const started = month <= NOV
    return { month, started, budgeted: started && month >= OCT, receivedMinor: started ? 0 : null, differenceMinor: null, ...LINES[month] }
  }),
})

const setupMocks = (overrides?: { rows?: SnapshotRow[]; budget?: IncomeBudget }) => {
  ;(getAccountsSnapshot as Mock).mockResolvedValue(makeSnapshot({ rows: overrides?.rows ?? [makeRow(), BUCKET] }))
  ;(getIncomeBudget as Mock).mockResolvedValue(overrides?.budget ?? makeBudget())
  ;(previewIncomeBudget as Mock).mockImplementation(async ({ fromYear, toYear }: { fromYear: number; toYear: number }) => makePreview(fromYear, toYear))
  ;(saveIncomeBudget as Mock).mockResolvedValue(undefined)
  ;(listPersons as Mock).mockResolvedValue([
    { id: 1, name: 'Personal', personType: 'physical', isDefault: true, vatPayer: false },
    { id: 2, name: 'Sumbal consulting', personType: 'legal', isDefault: false, vatPayer: true },
  ])
  ;(getConsolidationCurrency as Mock).mockResolvedValue({ id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false })
}

const newQueryClient = () => new QueryClient({ defaultOptions: { queries: { retry: 0 } } })

const renderDialog = (queryClient = newQueryClient()) => {
  const onClose = vi.fn()
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <SelectedPersonProvider>{children}</SelectedPersonProvider>
    </QueryClientProvider>
  )
  const { unmount } = render(<IncomeBudgetDialog bucketId={30} onClose={onClose} />, { wrapper })
  return { onClose, unmount }
}

const rowCells = (label: string) => within(screen.getByRole('row', { name: new RegExp(`^${label}`) })).getAllByRole('cell')

const NOV_INDEX = 10

describe('IncomeBudgetDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    timeMachine.asOf = LIVE
    setupMocks()
  })

  it('names the bucket and its person and opens on the saved year with month headers', async () => {
    renderDialog()

    expect(await screen.findByRole('dialog', { name: /Consulting income/ })).toHaveTextContent('Sumbal consulting')
    expect(screen.getByRole('columnheader', { name: /^Jan 2026/ })).toBeInTheDocument()
    expect(screen.getByRole('columnheader', { name: /^Dec 2026/ })).toBeInTheDocument()
    expect(screen.queryByRole('columnheader', { name: /2025|2027/ })).not.toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: 'incomeBudget.table.budgetCell (month=October 2026)' })).toHaveValue('5640')
    expect(screen.getByRole('textbox', { name: 'incomeBudget.table.vatCell (month=November 2026)' })).toHaveValue('23')
  })

  it('marks the moment month Today when live', async () => {
    renderDialog()

    expect((await screen.findByText('incomeBudget.table.today')).closest('th')).toHaveTextContent('Nov 2026')
    expect(getIncomeBudget).toHaveBeenCalledWith(30, LIVE)
  })

  it('marks the moment month with its date in the time machine', async () => {
    timeMachine.asOf = { kind: 'at', until: '2026-10-15' }
    setupMocks({ budget: makeBudget({ day: '2026-10-15', momentMonth: OCT }) })
    renderDialog()

    expect((await screen.findByText('incomeBudget.table.asOf (date=15 Oct)')).closest('th')).toHaveTextContent('Oct 2026')
    expect(screen.queryByText('incomeBudget.table.today')).not.toBeInTheDocument()
    expect(getIncomeBudget).toHaveBeenCalledWith(30, { kind: 'at', until: '2026-10-15' })
  })

  it('greys Received where the month has no budget', async () => {
    renderDialog()

    await waitFor(() => expect(rowCells('incomeBudget.table.received')[8]).toHaveTextContent('5 481.00'))
    const received = rowCells('incomeBudget.table.received')
    expect(received[8]).toHaveClass('text-muted-foreground')
    expect(received[9]).toHaveTextContent('5 857.00')
    expect(received[9]).not.toHaveClass('text-muted-foreground')
    expect(received[11]).toHaveTextContent('')
  })

  it('says the moment month is so far, in Received and Difference', async () => {
    renderDialog()

    await waitFor(() => expect(rowCells('incomeBudget.table.difference')[NOV_INDEX]).toHaveTextContent('-6 120.00'))
    expect(rowCells('incomeBudget.table.difference')[NOV_INDEX]).toHaveTextContent('incomeBudget.table.soFar')
    expect(rowCells('incomeBudget.table.received')[NOV_INDEX]).toHaveTextContent('incomeBudget.table.soFar')
    expect(rowCells('incomeBudget.table.difference')[9]).toHaveTextContent(/^\+217\.00$/)
  })

  it("states the value and what it does to Left to Spend, from Rust's preview", async () => {
    renderDialog()

    expect(await screen.findByText('-5 903.00 EUR')).toBeInTheDocument()
    expect(screen.getByText(/incomeBudget\.effect\.aboveCash/)).toHaveTextContent(
      'incomeBudget.effect.aboveCash 5 903.00 EUR incomeBudget.effect.currentMonthPending (month=November)',
    )
    expect(screen.getByText('incomeBudget.summary.on (date=8 November 2026)')).toBeInTheDocument()
  })

  it('names the counted accounts under the table', async () => {
    renderDialog()

    expect(await screen.findByText('incomeBudget.counted.some (accounts=TB Sumbal consulting)')).toBeInTheDocument()
  })

  it('says when no account counts yet', async () => {
    setupMocks({ budget: makeBudget({ countedAccounts: [] }) })
    renderDialog()

    expect(await screen.findByText('incomeBudget.counted.none (person=Sumbal consulting, currency=EUR)')).toBeInTheDocument()
  })

  it('adds a year at either end without changing the values', async () => {
    const user = userEvent.setup()
    renderDialog()

    await user.click(await screen.findByRole('button', { name: 'incomeBudget.table.addYear (year=2027)' }))
    await user.click(screen.getByRole('button', { name: 'incomeBudget.table.addYear (year=2025)' }))

    expect(screen.getByRole('columnheader', { name: /^Jan 2025/ })).toBeInTheDocument()
    expect(screen.getByRole('columnheader', { name: /^Dec 2027/ })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'incomeBudget.table.addYear (year=2024)' })).toBeInTheDocument()
    expect(screen.getByText('incomeBudget.announce.yearAdded (year=2025)')).toBeInTheDocument()
    await waitFor(() => expect(previewIncomeBudget).toHaveBeenLastCalledWith(expect.objectContaining({ fromYear: 2025, toYear: 2027 })))
    expect(screen.getByRole('textbox', { name: 'incomeBudget.table.budgetCell (month=October 2026)' })).toHaveValue('5640')
  })

  it('previews what is typed and marks an invalid cell without sending it', async () => {
    const user = userEvent.setup()
    renderDialog()

    const december = await screen.findByRole('textbox', { name: 'incomeBudget.table.budgetCell (month=December 2026)' })
    await user.type(december, '6120')
    await waitFor(() =>
      expect(previewIncomeBudget).toHaveBeenLastCalledWith(expect.objectContaining({ months: expect.arrayContaining([{ month: DEC, budgetMinor: 612000, vatBps: null }]) })),
    )

    await user.clear(december)
    await user.type(december, '61,20.0')
    expect(december).toHaveAttribute('aria-invalid', 'true')
    await waitFor(() =>
      expect(previewIncomeBudget).toHaveBeenLastCalledWith(expect.objectContaining({ months: expect.not.arrayContaining([expect.objectContaining({ month: DEC })]) })),
    )
  })

  it('saves the typed months and closes', async () => {
    const user = userEvent.setup()
    const { onClose } = renderDialog()

    await user.type(await screen.findByRole('textbox', { name: 'incomeBudget.table.vatCell (month=December 2026)' }), '23')
    await user.click(screen.getByRole('button', { name: 'incomeBudget.save' }))

    await waitFor(() => expect(onClose).toHaveBeenCalledOnce())
    expect(saveIncomeBudget).toHaveBeenCalledWith(30, [
      { month: OCT, budgetMinor: 564000, vatBps: 2300 },
      { month: NOV, budgetMinor: 612000, vatBps: 2300 },
      { month: DEC, budgetMinor: null, vatBps: 2300 },
    ])
  })

  it('reopens on the months saved since it was last open, wherever the time machine was', async () => {
    const user = userEvent.setup()
    let stored = makeBudget().months
    ;(getIncomeBudget as Mock).mockImplementation(async () => makeBudget({ months: stored }))
    ;(saveIncomeBudget as Mock).mockImplementation(async (_bucketId: number, months: IncomeBudget['months']) => {
      stored = months
    })
    const queryClient = newQueryClient()
    const december = () => screen.findByRole('textbox', { name: 'incomeBudget.table.budgetCell (month=December 2026)' })

    const firstLive = renderDialog(queryClient)
    expect(await december()).toHaveValue('')
    firstLive.unmount()

    timeMachine.asOf = { kind: 'at', until: '2026-10-15' }
    const inThePast = renderDialog(queryClient)
    await user.type(await december(), '6120')
    await user.click(screen.getByRole('button', { name: 'incomeBudget.save' }))
    await waitFor(() => expect(inThePast.onClose).toHaveBeenCalledOnce())
    inThePast.unmount()

    timeMachine.asOf = LIVE
    renderDialog(queryClient)
    expect(await december()).toHaveValue('6120')
  })

  it('sends nothing on Cancel', async () => {
    const user = userEvent.setup()
    const { onClose } = renderDialog()

    await user.type(await screen.findByRole('textbox', { name: 'incomeBudget.table.budgetCell (month=December 2026)' }), '6120')
    await user.click(screen.getByRole('button', { name: 'incomeBudget.cancel' }))

    expect(onClose).toHaveBeenCalledOnce()
    expect(saveIncomeBudget).not.toHaveBeenCalled()
  })

  it('closes when its bucket leaves the snapshot', async () => {
    setupMocks({ rows: [makeRow()] })
    const { onClose } = renderDialog()

    await waitFor(() => expect(onClose).toHaveBeenCalled())
  })

  describe('during a fill drag', () => {
    const capture = vi.fn()
    beforeEach(() => {
      HTMLElement.prototype.setPointerCapture = capture
    })
    afterEach(() => {
      Reflect.deleteProperty(HTMLElement.prototype, 'setPointerCapture')
    })

    it('lets Escape cancel only the drag, and the next Escape close the dialog', async () => {
      const { onClose } = renderDialog()
      const square = await screen.findByRole('button', { name: 'incomeBudget.table.fillBudget (month=October 2026)' })

      fireEvent.pointerDown(square, { button: 0, pointerId: 7 })
      expect(capture).toHaveBeenCalledWith(7)
      fireEvent.keyDown(document.body, { key: 'Escape' })
      expect(onClose).not.toHaveBeenCalled()

      fireEvent.keyDown(document.body, { key: 'Escape' })
      expect(onClose).toHaveBeenCalledOnce()
    })
  })
})
