import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { setBucketEnabled } from '@/shared/api'
import BucketSwitch from './BucketSwitch'

vi.mock('@/shared/api', () => ({ setBucketEnabled: vi.fn() }))

vi.mock('react-i18next', async () => (await import('@/shared/testing/i18nMock')).i18nMock)

vi.mock('sonner', () => ({ toast: { error: vi.fn() } }))

const renderSwitch = () => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  render(<BucketSwitch bucketId={30} bucketName="Holiday" enabled={true} />, { wrapper })
  return screen.getByRole('switch', { name: 'buckets.countInLeftToSpend (name=Holiday)' })
}

describe('BucketSwitch', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('sends one request for a double click and shows what it asked for until it returns', async () => {
    const user = userEvent.setup()
    let finish = () => {}
    ;(setBucketEnabled as Mock).mockImplementation(() => new Promise<void>((resolve) => (finish = resolve)))
    const toggle = renderSwitch()

    await user.dblClick(toggle)

    expect(setBucketEnabled).toHaveBeenCalledOnce()
    expect(setBucketEnabled).toHaveBeenCalledWith(30, false)
    expect(toggle).toHaveAttribute('aria-checked', 'false')
    expect(toggle).toBeDisabled()
    finish()
    await waitFor(() => expect(toggle).toBeEnabled())
  })
})
