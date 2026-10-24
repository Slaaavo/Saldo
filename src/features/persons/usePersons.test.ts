import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { renderHook, waitFor, act } from '@testing-library/react'
import { createElement, type ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { usePersons } from './usePersons'
import { SelectedPersonContext } from '../../app/useSelectedPerson'

vi.mock('../../shared/api', () => ({
  listPersons: vi.fn(),
  createPerson: vi.fn(),
  updatePerson: vi.fn(),
  deletePerson: vi.fn(),
}))

import { listPersons, updatePerson, deletePerson } from '../../shared/api'

const setup = (selectedPersonId: number | null) => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
  const setSelectedPersonId = vi.fn()
  const wrapper = ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client: queryClient }, createElement(SelectedPersonContext.Provider, { value: { selectedPersonId, setSelectedPersonId } }, children))
  const hook = renderHook(() => usePersons(), { wrapper })
  const invalidatedKeys = () => invalidate.mock.calls.map(([filters]) => (filters?.queryKey as string[])[0])
  return { hook, setSelectedPersonId, invalidatedKeys }
}

describe('usePersons', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(listPersons as Mock).mockResolvedValue([])
    ;(updatePerson as Mock).mockResolvedValue(undefined)
    ;(deletePerson as Mock).mockResolvedValue(undefined)
  })

  it('invalidates persons, events and snapshot on rename', async () => {
    const { hook, invalidatedKeys } = setup(null)
    await waitFor(() => expect(hook.result.current.isLoading).toBe(false))

    await act(() => hook.result.current.handleUpdatePerson(1, 'New name', 'physical', false))

    expect(invalidatedKeys()).toEqual(expect.arrayContaining(['persons', 'events', 'snapshot']))
  })

  it('invalidates persons, events and snapshot on delete', async () => {
    const { hook, invalidatedKeys } = setup(null)
    await waitFor(() => expect(hook.result.current.isLoading).toBe(false))

    await act(() => hook.result.current.handleDeletePerson(2))

    expect(invalidatedKeys()).toEqual(expect.arrayContaining(['persons', 'events', 'snapshot']))
  })

  it('clears the selected person when the deleted person is selected', async () => {
    const { hook, setSelectedPersonId } = setup(2)
    await waitFor(() => expect(hook.result.current.isLoading).toBe(false))

    await act(() => hook.result.current.handleDeletePerson(2))

    expect(setSelectedPersonId).toHaveBeenCalledWith(null)
  })

  it('keeps the selected person when another person is deleted', async () => {
    const { hook, setSelectedPersonId } = setup(1)
    await waitFor(() => expect(hook.result.current.isLoading).toBe(false))

    await act(() => hook.result.current.handleDeletePerson(2))

    expect(setSelectedPersonId).not.toHaveBeenCalled()
  })
})
