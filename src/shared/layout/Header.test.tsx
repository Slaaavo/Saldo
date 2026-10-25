import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import Header from './Header'
import { TimeMachineProvider } from '../../app/TimeMachineContext'
import { useTimeMachine } from '../../app/useTimeMachine'

vi.mock('react-i18next', async () => (await import('../testing/i18nMock')).i18nMock)

let pathname = '/dashboard'
vi.mock('@tanstack/react-router', () => ({
  useRouterState: () => ({ location: { pathname } }),
}))

vi.mock('../../features/dashboard/PersonSelector', () => ({
  default: () => <div data-testid="person-selector" />,
}))

const AsOfProbe = () => {
  const { asOf } = useTimeMachine()
  return <output data-testid="as-of">{JSON.stringify(asOf)}</output>
}

const renderHeader = () =>
  render(
    <TimeMachineProvider>
      <Header pageTitle="Dashboard" />
      <AsOfProbe />
    </TimeMachineProvider>,
  )

const dateControl = () => screen.getByLabelText('header.date')

const pickOctober20 = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.click(dateControl())
  await user.click(await screen.findByRole('button', { name: /October 20th, 2026/ }))
}

describe('Header time machine control', () => {
  beforeEach(() => {
    pathname = '/dashboard'
    vi.useFakeTimers({ toFake: ['Date'] })
    vi.setSystemTime(new Date(2026, 9, 25, 10, 0, 0))
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('starts on live', () => {
    renderHeader()

    expect(dateControl()).toHaveTextContent('timeMachine.live')
    expect(screen.getByTestId('as-of')).toHaveTextContent('{"kind":"live"}')
  })

  it('enters the time machine at the picked day', async () => {
    const user = userEvent.setup()
    renderHeader()

    await pickOctober20(user)

    expect(screen.getByTestId('as-of')).toHaveTextContent('{"kind":"at","until":"2026-10-20"}')
    expect(dateControl()).toHaveTextContent('20 October 2026')
  })

  it('does not offer Today, because that would be a day in the time machine and not live', async () => {
    const user = userEvent.setup()
    renderHeader()

    await user.click(dateControl())
    await screen.findByRole('grid')

    expect(screen.queryByText('datePicker.today')).not.toBeInTheDocument()
  })

  it('marks the control amber while in the time machine and plain while live', async () => {
    const user = userEvent.setup()
    renderHeader()
    expect(dateControl()).not.toHaveClass('border-amber-500')

    await pickOctober20(user)

    expect(dateControl()).toHaveClass('border-amber-500')
  })

  it('goes back to live from the clear control', async () => {
    const user = userEvent.setup()
    renderHeader()
    await pickOctober20(user)

    await user.click(screen.getByRole('button', { name: 'Clear date' }))

    expect(screen.getByTestId('as-of')).toHaveTextContent('{"kind":"live"}')
    expect(dateControl()).toHaveTextContent('timeMachine.live')
  })

  it('leaves the control out of pages other than the dashboard', () => {
    pathname = '/ledger'
    renderHeader()

    expect(screen.queryByLabelText('header.date')).not.toBeInTheDocument()
  })
})
