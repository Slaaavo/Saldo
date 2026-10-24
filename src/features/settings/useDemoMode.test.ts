import { describe, it, expect, vi, beforeEach, afterEach, type Mock } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { useDemoMode } from './useDemoMode'
import { enterDemoMode, exitDemoMode, getAppSetting, isDemoMode } from '../../shared/api'

vi.mock('../../shared/api', () => ({
  enterDemoMode: vi.fn(),
  exitDemoMode: vi.fn(),
  getAppSetting: vi.fn(),
  isDemoMode: vi.fn(),
}))

const mockToastError = vi.fn()
vi.mock('sonner', () => ({
  toast: { error: (...args: unknown[]) => mockToastError(...args) },
}))

const mockReload = vi.fn()

describe('useDemoMode', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()
    ;(isDemoMode as Mock).mockResolvedValue(false)
    ;(enterDemoMode as Mock).mockResolvedValue(undefined)
    ;(exitDemoMode as Mock).mockResolvedValue(undefined)
    ;(getAppSetting as Mock).mockResolvedValue('dark')
    vi.stubGlobal('location', { ...window.location, reload: mockReload })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('reads the demo flag from the backend on mount', async () => {
    ;(isDemoMode as Mock).mockResolvedValue(true)
    const { result } = renderHook(() => useDemoMode())
    await waitFor(() => expect(result.current.isDemoMode).toBe(true))
  })

  it('reloads the window after the backend entered demo mode', async () => {
    const { result } = renderHook(() => useDemoMode())

    await act(() => result.current.handleEnter())

    expect(enterDemoMode).toHaveBeenCalledOnce()
    expect(mockReload).toHaveBeenCalledOnce()
    expect(mockToastError).not.toHaveBeenCalled()
  })

  it('does not reload and toasts the error when entering demo mode fails', async () => {
    ;(enterDemoMode as Mock).mockRejectedValue(new Error('enter failed'))
    const { result } = renderHook(() => useDemoMode())

    await act(() => result.current.handleEnter())

    expect(mockReload).not.toHaveBeenCalled()
    expect(mockToastError).toHaveBeenCalledWith('enter failed')
  })

  it('writes the real theme to localStorage and then reloads after exiting demo mode', async () => {
    localStorage.setItem('theme', 'light')
    const { result } = renderHook(() => useDemoMode())

    await act(() => result.current.handleExit())

    expect(exitDemoMode).toHaveBeenCalledOnce()
    expect(getAppSetting).toHaveBeenCalledWith('theme')
    expect(localStorage.getItem('theme')).toBe('dark')
    expect(mockReload).toHaveBeenCalledOnce()
    expect((getAppSetting as Mock).mock.invocationCallOrder[0]).toBeLessThan(mockReload.mock.invocationCallOrder[0])
  })

  it('falls back to the system theme when the real database has no theme setting', async () => {
    ;(getAppSetting as Mock).mockResolvedValue(null)
    localStorage.setItem('theme', 'light')
    const { result } = renderHook(() => useDemoMode())

    await act(() => result.current.handleExit())

    expect(localStorage.getItem('theme')).toBe('system')
    expect(mockReload).toHaveBeenCalledOnce()
  })

  it('still reloads when the real theme cannot be read after exiting demo mode', async () => {
    ;(getAppSetting as Mock).mockRejectedValue(new Error('settings unavailable'))
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    const { result } = renderHook(() => useDemoMode())

    await act(() => result.current.handleExit())

    expect(mockReload).toHaveBeenCalledOnce()
    expect(mockToastError).not.toHaveBeenCalled()
    consoleError.mockRestore()
  })

  it('does not reload or touch the theme and toasts the error when exiting demo mode fails', async () => {
    ;(exitDemoMode as Mock).mockRejectedValue(new Error('exit failed'))
    localStorage.setItem('theme', 'light')
    const { result } = renderHook(() => useDemoMode())

    await act(() => result.current.handleExit())

    expect(mockReload).not.toHaveBeenCalled()
    expect(getAppSetting).not.toHaveBeenCalled()
    expect(localStorage.getItem('theme')).toBe('light')
    expect(mockToastError).toHaveBeenCalledWith('exit failed')
  })
})
