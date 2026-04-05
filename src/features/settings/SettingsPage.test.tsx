import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import SettingsPage from './SettingsPage'
import type { Currency } from '../../shared/types'
import type { ThemePreference } from './useTheme'

// ── Mock useSettings hook ───────────────────────────────────────────────────
const mockHandleCurrencySelect = vi.fn()
const mockHandleSaveApiKey = vi.fn()
const mockSetApiKey = vi.fn()

const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }
const USD: Currency = { id: 2, code: 'USD', name: 'US Dollar', minorUnits: 2, isCustom: false }

let hookReturn = {
  currencies: [EUR, USD],
  selectedCurrency: EUR as Currency | null,
  apiKey: '',
  setApiKey: mockSetApiKey,
  apiKeySaved: false,
  currencySaved: false,
  handleCurrencySelect: mockHandleCurrencySelect,
  handleSaveApiKey: mockHandleSaveApiKey,
}

vi.mock('./useSettings', () => ({
  useSettings: () => hookReturn,
}))

// ── Mock useDemo hook ──────────────────────────────────────────────────────
const mockOnEnterDemoMode = vi.fn()
const mockOnExitDemoMode = vi.fn()

let demoModeReturn = {
  isDemoMode: false,
  onEnterDemoMode: mockOnEnterDemoMode,
  onExitDemoMode: mockOnExitDemoMode,
}

vi.mock('../../app/DemoContext', () => ({
  useDemo: () => demoModeReturn,
}))

// ── Mock useTheme hook ─────────────────────────────────────────────────────
let themeHookReturn = {
  theme: 'light' as 'light' | 'dark',
  themePreference: 'system' as ThemePreference,
  setThemePreference: vi.fn(),
}

vi.mock('./useTheme', () => ({
  useTheme: () => themeHookReturn,
}))

// ── Mock useModal hook ─────────────────────────────────────────────────────
vi.mock('../../app/ModalContext', () => ({
  useModal: () => ({ modalState: { type: 'none' }, setModalState: vi.fn(), closeModal: vi.fn() }),
}))

// ── Mock useDbLocation hook ────────────────────────────────────────────────
const mockHandleChange = vi.fn()
const mockHandleReset = vi.fn()

let dbLocationReturn = {
  path: '/home/user/.local/share/our-finances/db.sqlite',
  isDefault: true,
  actionLoading: false,
  handleChange: mockHandleChange,
  handleReset: mockHandleReset,
  load: vi.fn(),
  handleConfirmSwitch: vi.fn(),
  handleLocationChoiceAction: vi.fn(),
  handleConfirmReset: vi.fn(),
}

vi.mock('./useDbLocation', () => ({
  useDbLocation: () => dbLocationReturn,
}))

// ── Mock i18n — pass-through that returns the key (with interpolations) ─────
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (opts) {
        let result = key
        for (const [k, v] of Object.entries(opts)) {
          result = result.replace(`{{${k}}}`, String(v))
        }
        return result
      }
      return key
    },
    i18n: { language: 'en' },
  }),
}))

// ── Mock child components to isolate SettingsPage logic ─────────────────────
vi.mock('../currency/CurrencySelect', () => ({
  default: ({ value, onChange, currencies }: { value: Currency | null; onChange: (c: Currency) => void; currencies: Currency[] }) => (
    <div data-testid="currency-select">
      <span data-testid="currency-select-value">{value?.code ?? 'none'}</span>
      {currencies.map((c) => (
        <button key={c.id} data-testid={`currency-option-${c.code}`} onClick={() => onChange(c)}>
          {c.code}
        </button>
      ))}
    </div>
  ),
}))

vi.mock('./LanguageSelector', () => ({
  default: () => <div data-testid="language-selector">LanguageSelector</div>,
}))

// ── Mock API functions used internally by query hooks ────────────────────────
vi.mock('../../shared/api', () => ({
  getAccountsSnapshot: vi.fn(),
  getBulkUpdateExclusions: vi.fn(),
  setBulkUpdateExclusions: vi.fn(),
}))
import { getAccountsSnapshot, getBulkUpdateExclusions } from '../../shared/api'

const makeWrapper = () => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const Wrapper = ({ children }: { children: React.ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  return Wrapper
}

// ── Tests ───────────────────────────────────────────────────────────────────

describe('SettingsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(getAccountsSnapshot as Mock).mockResolvedValue([])
    ;(getBulkUpdateExclusions as Mock).mockResolvedValue([])
    hookReturn = {
      currencies: [EUR, USD],
      selectedCurrency: EUR,
      apiKey: '',
      setApiKey: mockSetApiKey,
      apiKeySaved: false,
      currencySaved: false,
      handleCurrencySelect: mockHandleCurrencySelect,
      handleSaveApiKey: mockHandleSaveApiKey,
    }
    demoModeReturn = {
      isDemoMode: false,
      onEnterDemoMode: mockOnEnterDemoMode,
      onExitDemoMode: mockOnExitDemoMode,
    }
    themeHookReturn = {
      theme: 'light',
      themePreference: 'system',
      setThemePreference: vi.fn(),
    }
    dbLocationReturn = {
      path: '/home/user/.local/share/our-finances/db.sqlite',
      isDefault: true,
      actionLoading: false,
      handleChange: mockHandleChange,
      handleReset: mockHandleReset,
      load: vi.fn(),
      handleConfirmSwitch: vi.fn(),
      handleLocationChoiceAction: vi.fn(),
      handleConfirmReset: vi.fn(),
    }
  })

  // ── Section rendering ───────────────────────────────────────────────────

  it('renders all four sections', () => {
    render(<SettingsPage />, { wrapper: makeWrapper() })

    expect(screen.getByText('settings.sectionDisplay')).toBeInTheDocument()
    expect(screen.getByText('settings.sectionIntegrations')).toBeInTheDocument()
    expect(screen.getByText('dataStorage.sectionTitle')).toBeInTheDocument()
    expect(screen.getByText('demo.settingsTitle')).toBeInTheDocument()
  })

  it('renders section descriptions', () => {
    render(<SettingsPage />, { wrapper: makeWrapper() })

    expect(screen.getByText('settings.sectionDisplayDesc')).toBeInTheDocument()
    expect(screen.getByText('settings.sectionIntegrationsDesc')).toBeInTheDocument()
    expect(screen.getByText('dataStorage.sectionDesc')).toBeInTheDocument()
    expect(screen.getByText('demo.settingsDesc')).toBeInTheDocument()
  })

  // ── Display section ───────────────────────────────────────────────────

  it('renders language selector', () => {
    render(<SettingsPage />, { wrapper: makeWrapper() })
    expect(screen.getByTestId('language-selector')).toBeInTheDocument()
  })

  it('renders theme select with current preference', () => {
    themeHookReturn = { ...themeHookReturn, themePreference: 'dark' }
    render(<SettingsPage />, { wrapper: makeWrapper() })
    expect(screen.getByText('settings.theme.label')).toBeInTheDocument()
  })

  it('renders currency select with loaded currencies', () => {
    render(<SettingsPage />, { wrapper: makeWrapper() })

    expect(screen.getByTestId('currency-select-value')).toHaveTextContent('EUR')
    expect(screen.getByTestId('currency-option-EUR')).toBeInTheDocument()
    expect(screen.getByTestId('currency-option-USD')).toBeInTheDocument()
  })

  it('shows currency saved badge when currencySaved is true', () => {
    hookReturn = { ...hookReturn, currencySaved: true }
    render(<SettingsPage />, { wrapper: makeWrapper() })

    const savedElements = screen.getAllByText('settings.saved')
    const currencySaved = savedElements.find((el) => el.closest('[data-testid="currency-select"]')?.parentElement != null || el.tagName === 'SPAN')
    expect(currencySaved).toBeDefined()
  })

  it('calls handleCurrencySelect when a currency is clicked', async () => {
    const user = userEvent.setup()
    render(<SettingsPage />, { wrapper: makeWrapper() })

    await user.click(screen.getByTestId('currency-option-USD'))

    expect(mockHandleCurrencySelect).toHaveBeenCalledWith(USD)
  })

  // ── OXR API key section ───────────────────────────────────────────────

  it('renders API key input with stored value', () => {
    hookReturn = { ...hookReturn, apiKey: 'my-secret-key' }
    render(<SettingsPage />, { wrapper: makeWrapper() })

    expect(screen.getByLabelText('settings.oxrApiKey')).toHaveValue('my-secret-key')
  })

  it('calls setApiKey when typing in API key input', async () => {
    const user = userEvent.setup()
    render(<SettingsPage />, { wrapper: makeWrapper() })

    const input = screen.getByLabelText('settings.oxrApiKey')
    await user.type(input, 'k')

    expect(mockSetApiKey).toHaveBeenCalled()
  })

  it('calls handleSaveApiKey when save button is clicked', async () => {
    const user = userEvent.setup()
    render(<SettingsPage />, { wrapper: makeWrapper() })

    await user.click(screen.getByText('settings.saveApiKey'))

    expect(mockHandleSaveApiKey).toHaveBeenCalledOnce()
  })

  it('shows saved state on API key button when apiKeySaved is true', () => {
    hookReturn = { ...hookReturn, apiKeySaved: true }
    render(<SettingsPage />, { wrapper: makeWrapper() })

    const buttons = screen.getAllByRole('button')
    const saveBtn = buttons.find((b) => b.textContent === 'settings.saved' && !b.closest('[data-testid="currency-select"]'))
    expect(saveBtn).toBeDefined()
  })

  // ── Data Storage section ──────────────────────────────────────────────

  it('displays the database location path', () => {
    dbLocationReturn = { ...dbLocationReturn, path: '/custom/path/db.sqlite', isDefault: false }
    render(<SettingsPage />, { wrapper: makeWrapper() })
    expect(screen.getByText('/custom/path/db.sqlite')).toBeInTheDocument()
  })

  it('shows default badge when using default location', () => {
    render(<SettingsPage />, { wrapper: makeWrapper() })
    expect(screen.getByText('dataStorage.isDefault')).toBeInTheDocument()
  })

  it('does not show default badge for custom location', () => {
    dbLocationReturn = { ...dbLocationReturn, isDefault: false }
    render(<SettingsPage />, { wrapper: makeWrapper() })
    expect(screen.queryByText('dataStorage.isDefault')).not.toBeInTheDocument()
  })

  it('calls handleChange when change button is clicked', async () => {
    const user = userEvent.setup()
    render(<SettingsPage />, { wrapper: makeWrapper() })

    await user.click(screen.getByText('dataStorage.changeButton'))
    expect(mockHandleChange).toHaveBeenCalledOnce()
  })

  it('shows reset button only when not using default location', () => {
    const { rerender } = render(<SettingsPage />, { wrapper: makeWrapper() })
    expect(screen.queryByText('dataStorage.resetButton')).not.toBeInTheDocument()

    dbLocationReturn = { ...dbLocationReturn, isDefault: false }
    rerender(<SettingsPage />)
    expect(screen.getByText('dataStorage.resetButton')).toBeInTheDocument()
  })

  it('calls handleReset when reset button is clicked', async () => {
    dbLocationReturn = { ...dbLocationReturn, isDefault: false }
    const user = userEvent.setup()
    render(<SettingsPage />, { wrapper: makeWrapper() })

    await user.click(screen.getByText('dataStorage.resetButton'))
    expect(mockHandleReset).toHaveBeenCalledOnce()
  })

  it('disables db buttons in demo mode', () => {
    demoModeReturn = { ...demoModeReturn, isDemoMode: true }
    dbLocationReturn = { ...dbLocationReturn, isDefault: false }
    render(<SettingsPage />, { wrapper: makeWrapper() })

    expect(screen.getByText('dataStorage.changeButton')).toBeDisabled()
    expect(screen.getByText('dataStorage.resetButton')).toBeDisabled()
    expect(screen.getByText('dataStorage.disabledInDemoMode')).toBeInTheDocument()
  })

  // ── Demo Mode section ─────────────────────────────────────────────────

  it('shows start button when not in demo mode', () => {
    render(<SettingsPage />, { wrapper: makeWrapper() })

    expect(screen.getByText('demo.startButton')).toBeInTheDocument()
    expect(screen.getByText('demo.settingsNote')).toBeInTheDocument()
    expect(screen.queryByText('demo.stopButton')).not.toBeInTheDocument()
  })

  it('shows stop button when in demo mode', () => {
    demoModeReturn = { ...demoModeReturn, isDemoMode: true }
    render(<SettingsPage />, { wrapper: makeWrapper() })

    expect(screen.getByText('demo.stopButton')).toBeInTheDocument()
    expect(screen.queryByText('demo.startButton')).not.toBeInTheDocument()
  })

  it('calls onEnterDemoMode from useDemo when start button is clicked', async () => {
    const user = userEvent.setup()
    render(<SettingsPage />, { wrapper: makeWrapper() })

    await user.click(screen.getByText('demo.startButton'))
    expect(mockOnEnterDemoMode).toHaveBeenCalledOnce()
  })

  it('calls onExitDemoMode from useDemo when stop button is clicked', async () => {
    demoModeReturn = { ...demoModeReturn, isDemoMode: true }
    const user = userEvent.setup()
    render(<SettingsPage />, { wrapper: makeWrapper() })

    await user.click(screen.getByText('demo.stopButton'))
    expect(mockOnExitDemoMode).toHaveBeenCalledOnce()
  })
})
