import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { act, fireEvent, renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { toast } from 'sonner'
import { saveIncomeBudget } from '@/shared/api'
import type { BudgetCell, IncomeBudget } from '@/shared/types'
import { cellText } from './budgetDraft'
import { monthKey } from './month'
import type { DragSurface, FillPointerDown } from './useFillDrag'
import { useIncomeBudgetEditor } from './useIncomeBudgetEditor'

vi.mock('@/shared/api', () => ({ saveIncomeBudget: vi.fn() }))
vi.mock('sonner', () => ({ toast: { error: vi.fn() } }))
vi.mock('react-i18next', async () => (await import('@/shared/testing/i18nMock')).i18nMock)

const OCT = monthKey(2026, 10)
const NOV = monthKey(2026, 11)
const DEC = monthKey(2026, 12)

const makeBudget = (months: BudgetCell[] = []): IncomeBudget => ({ day: '2026-11-08', momentMonth: NOV, months, countedAccounts: [] })

const COLUMN = 100
const STICKY = 148
const columnCentre = (index: number) => STICKY + index * COLUMN + COLUMN / 2

const surface: DragSurface = {
  edges: Array.from({ length: 13 }, (_, i) => STICKY + i * COLUMN),
  stickyWidth: STICKY,
  scroller: { scrollLeft: 0, clientWidth: 2000, getBoundingClientRect: () => ({ left: 0 }) },
}

const pointerDown = (): FillPointerDown => ({ button: 0, pointerId: 1, currentTarget: { setPointerCapture: vi.fn() }, preventDefault: vi.fn() })

const renderEditor = (budget = makeBudget()) => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
  const onSaved = vi.fn()
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  const hook = renderHook(() => useIncomeBudgetEditor({ bucketId: 30, budget, minorUnits: 2, measure: () => surface, onSaved }), { wrapper })
  return { ...hook, invalidate, onSaved }
}

describe('useIncomeBudgetEditor', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(saveIncomeBudget as Mock).mockResolvedValue(undefined)
  })

  it('opens on the saved years with the saved values as text', () => {
    const { result } = renderEditor(makeBudget([{ month: monthKey(2025, 12), budgetMinor: 564050, vatBps: 2300 }]))

    expect(result.current.span).toEqual({ fromYear: 2025, toYear: 2025 })
    expect(result.current.months).toHaveLength(12)
    expect(cellText(result.current.draft, monthKey(2025, 12), 'budget')).toBe('5640.50')
  })

  it('saves only the typed months, then closes and refreshes the snapshot', async () => {
    const { result, invalidate, onSaved } = renderEditor()

    act(() => result.current.changeCell(NOV, 'budget', '6 120'))
    act(() => result.current.changeCell(DEC, 'vat', '23 %'))
    act(() => result.current.changeCell(OCT, 'budget', '5640'))
    act(() => result.current.changeCell(OCT, 'budget', ''))
    await act(() => result.current.save())

    expect(saveIncomeBudget).toHaveBeenCalledWith(30, [
      { month: NOV, budgetMinor: 612000, vatBps: null },
      { month: DEC, budgetMinor: null, vatBps: 2300 },
    ])
    expect(onSaved).toHaveBeenCalledOnce()
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['snapshot'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['incomeBudget', 30] })
  })

  it('blocks Save on an invalid cell and names it', async () => {
    const { result, onSaved } = renderEditor()

    act(() => result.current.changeCell(NOV, 'budget', '1,234.50'))
    await act(() => result.current.save())

    expect(saveIncomeBudget).not.toHaveBeenCalled()
    expect(onSaved).not.toHaveBeenCalled()
    expect(toast.error).toHaveBeenCalledWith('incomeBudget.invalid.budget (cell=incomeBudget.table.budgetCell (month=November 2026))')
  })

  it('keeps the dialog open and says why when the save is refused', async () => {
    ;(saveIncomeBudget as Mock).mockRejectedValue({ code: 'VALIDATION', message: 'This bucket is not an income budget.' })
    const { result, onSaved } = renderEditor()

    act(() => result.current.changeCell(NOV, 'budget', '6120'))
    await act(() => result.current.save())

    expect(onSaved).not.toHaveBeenCalled()
    expect(toast.error).toHaveBeenCalledWith('errors.saveIncomeBudget (error=This bucket is not an income budget.)')
    expect(result.current.saving).toBe(false)
  })

  it('shows an added year without saving it, so it is gone after Save and reopen', async () => {
    const { result } = renderEditor()

    act(() => result.current.addYear('before'))
    act(() => result.current.addYear('after'))
    expect(result.current.span).toEqual({ fromYear: 2025, toYear: 2027 })
    expect(result.current.months).toHaveLength(36)
    act(() => result.current.changeCell(NOV, 'budget', '6120'))
    await act(() => result.current.save())

    const saved: BudgetCell[] = (saveIncomeBudget as Mock).mock.calls[0][1]
    expect(saved).toEqual([{ month: NOV, budgetMinor: 612000, vatBps: null }])
    expect(renderEditor(makeBudget(saved)).result.current.span).toEqual({ fromYear: 2026, toYear: 2026 })
  })

  it('copies the origin into the months a finished drag passed and says so', () => {
    const { result } = renderEditor()
    act(() => result.current.changeCell(OCT, 'budget', '5640'))

    act(() => result.current.startFill('budget', 9, pointerDown()))
    act(() => void fireEvent.pointerMove(window, { clientX: columnCentre(10) }))
    act(() => void fireEvent.pointerMove(window, { clientX: columnCentre(11) }))
    expect(result.current.drag).toEqual({ row: 'budget', originIdx: 9, targetIdx: 11 })
    act(() => void fireEvent.pointerUp(window))

    expect([OCT, NOV, DEC].map((month) => cellText(result.current.draft, month, 'budget'))).toEqual(['5640', '5640', '5640'])
    expect(cellText(result.current.draft, monthKey(2026, 9), 'budget')).toBe('')
    expect(result.current.drag).toBeNull()
    expect(result.current.announcement).toBe('incomeBudget.announce.filled (count=2, value=5640)')
  })

  it('cancels a drag on Escape without writing anything', () => {
    const { result } = renderEditor()
    act(() => result.current.changeCell(OCT, 'budget', '5640'))

    act(() => result.current.startFill('budget', 9, pointerDown()))
    act(() => void fireEvent.pointerMove(window, { clientX: columnCentre(11) }))
    const escape = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
    act(() => void document.body.dispatchEvent(escape))
    act(() => void fireEvent.pointerUp(window))

    expect(escape.defaultPrevented).toBe(true)
    expect(result.current.drag).toBeNull()
    expect([NOV, DEC].map((month) => cellText(result.current.draft, month, 'budget'))).toEqual(['', ''])
  })
})
