import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { render, screen, waitFor, act } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ThemeProvider } from './ThemeContext'
import { useTheme } from './useTheme'
import { getAppSetting, setAppSetting } from '../../shared/api'

vi.mock('../../shared/api', () => ({
  getAppSetting: vi.fn(),
  setAppSetting: vi.fn(),
}))

type MediaListener = (e: { matches: boolean }) => void
let mediaListeners: MediaListener[] = []

const ThemeReader = ({ label }: { label: string }) => {
  const { theme, themePreference } = useTheme()
  return (
    <span data-testid={`reader-${label}`}>
      {themePreference}/{theme}
    </span>
  )
}

const ThemeSetter = () => {
  const { setThemePreference } = useTheme()
  return <button onClick={() => setThemePreference('light')}>choose light</button>
}

const renderProvider = (ui: React.ReactNode) => render(<ThemeProvider>{ui}</ThemeProvider>)

describe('ThemeProvider', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()
    document.documentElement.classList.remove('dark')
    mediaListeners = []
    ;(getAppSetting as Mock).mockResolvedValue(null)
    ;(setAppSetting as Mock).mockResolvedValue(undefined)
    Object.defineProperty(window, 'matchMedia', {
      writable: true,
      value: (query: string) => ({
        matches: false,
        media: query,
        addEventListener: (_: string, l: MediaListener) => mediaListeners.push(l),
        removeEventListener: (_: string, l: MediaListener) => {
          mediaListeners = mediaListeners.filter((x) => x !== l)
        },
      }),
    })
  })

  it('throws when useTheme is used outside a provider', () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    expect(() => render(<ThemeReader label="a" />)).toThrow('useTheme must be used within a ThemeProvider')
    consoleError.mockRestore()
  })

  it('loads the stored preference from the backend and caches it in localStorage', async () => {
    ;(getAppSetting as Mock).mockResolvedValue('dark')
    renderProvider(<ThemeReader label="a" />)

    await waitFor(() => {
      expect(screen.getByTestId('reader-a')).toHaveTextContent('dark/dark')
      expect(localStorage.getItem('theme')).toBe('dark')
      expect(document.documentElement.classList.contains('dark')).toBe(true)
    })
  })

  it('shares one preference between all consumers', async () => {
    const user = userEvent.setup()
    renderProvider(
      <>
        <ThemeReader label="shell" />
        <ThemeSetter />
        <ThemeReader label="settings" />
      </>,
    )
    await waitFor(() => expect(getAppSetting).toHaveBeenCalledWith('theme'))

    await user.click(screen.getByText('choose light'))

    expect(screen.getByTestId('reader-shell')).toHaveTextContent('light/light')
    expect(screen.getByTestId('reader-settings')).toHaveTextContent('light/light')
    expect(setAppSetting).toHaveBeenCalledWith('theme', 'light')
    expect(localStorage.getItem('theme')).toBe('light')
  })

  it('keeps an explicit choice when the OS theme changes afterwards', async () => {
    const user = userEvent.setup()
    renderProvider(
      <>
        <ThemeReader label="shell" />
        <ThemeSetter />
      </>,
    )
    await waitFor(() => expect(getAppSetting).toHaveBeenCalledWith('theme'))
    await user.click(screen.getByText('choose light'))

    act(() => mediaListeners.forEach((l) => l({ matches: true })))

    expect(screen.getByTestId('reader-shell')).toHaveTextContent('light/light')
    expect(document.documentElement.classList.contains('dark')).toBe(false)
  })

  it('follows the OS theme while the preference is system', async () => {
    renderProvider(<ThemeReader label="shell" />)
    await waitFor(() => expect(getAppSetting).toHaveBeenCalledWith('theme'))

    act(() => mediaListeners.forEach((l) => l({ matches: true })))

    expect(screen.getByTestId('reader-shell')).toHaveTextContent('system/dark')
    expect(document.documentElement.classList.contains('dark')).toBe(true)
  })
})
