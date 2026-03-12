import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import SettingsPage from './SettingsPage';
import type { Currency } from '../../shared/types';
import type { ThemePreference } from './useTheme';

// ── Mock useSettings hook ───────────────────────────────────────────────────
const mockHandleCurrencySelect = vi.fn();
const mockHandleSaveApiKey = vi.fn();
const mockSetApiKey = vi.fn();

const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false };
const USD: Currency = { id: 2, code: 'USD', name: 'US Dollar', minorUnits: 2, isCustom: false };

let hookReturn = {
  currencies: [EUR, USD],
  selectedCurrency: EUR as Currency | null,
  apiKey: '',
  setApiKey: mockSetApiKey,
  apiKeySaved: false,
  currencySaved: false,
  handleCurrencySelect: mockHandleCurrencySelect,
  handleSaveApiKey: mockHandleSaveApiKey,
};

vi.mock('./useSettings', () => ({
  useSettings: () => hookReturn,
}));

// ── Mock i18n — pass-through that returns the key (with interpolations) ─────
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (opts) {
        let result = key;
        for (const [k, v] of Object.entries(opts)) {
          result = result.replace(`{{${k}}}`, String(v));
        }
        return result;
      }
      return key;
    },
    i18n: { language: 'en' },
  }),
}));

// ── Mock child components to isolate SettingsPage logic ─────────────────────
vi.mock('../currency/CurrencySelect', () => ({
  default: ({
    value,
    onChange,
    currencies,
  }: {
    value: Currency | null;
    onChange: (c: Currency) => void;
    currencies: Currency[];
  }) => (
    <div data-testid="currency-select">
      <span data-testid="currency-select-value">{value?.code ?? 'none'}</span>
      {currencies.map((c) => (
        <button key={c.id} data-testid={`currency-option-${c.code}`} onClick={() => onChange(c)}>
          {c.code}
        </button>
      ))}
    </div>
  ),
}));

vi.mock('./LanguageSelector', () => ({
  default: () => <div data-testid="language-selector">LanguageSelector</div>,
}));

// ── Default props ───────────────────────────────────────────────────────────
function defaultProps(overrides?: Partial<Parameters<typeof SettingsPage>[0]>) {
  return {
    onConsolidationCurrencyChange: vi.fn(),
    themePreference: 'system' as ThemePreference,
    onThemeChange: vi.fn(),
    isDemoMode: false,
    onEnterDemoMode: vi.fn(),
    onExitDemoMode: vi.fn(),
    dbLocationPath: '/home/user/.local/share/our-finances/db.sqlite',
    dbLocationIsDefault: true,
    onChangeDbLocation: vi.fn(),
    onResetDbLocation: vi.fn(),
    ...overrides,
  };
}

// ── Tests ───────────────────────────────────────────────────────────────────

describe('SettingsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    hookReturn = {
      currencies: [EUR, USD],
      selectedCurrency: EUR,
      apiKey: '',
      setApiKey: mockSetApiKey,
      apiKeySaved: false,
      currencySaved: false,
      handleCurrencySelect: mockHandleCurrencySelect,
      handleSaveApiKey: mockHandleSaveApiKey,
    };
  });

  // ── Section rendering ───────────────────────────────────────────────────

  it('renders all four sections', () => {
    render(<SettingsPage {...defaultProps()} />);

    expect(screen.getByText('settings.sectionDisplay')).toBeInTheDocument();
    expect(screen.getByText('settings.sectionIntegrations')).toBeInTheDocument();
    expect(screen.getByText('dataStorage.sectionTitle')).toBeInTheDocument();
    expect(screen.getByText('demo.settingsTitle')).toBeInTheDocument();
  });

  it('renders section descriptions', () => {
    render(<SettingsPage {...defaultProps()} />);

    expect(screen.getByText('settings.sectionDisplayDesc')).toBeInTheDocument();
    expect(screen.getByText('settings.sectionIntegrationsDesc')).toBeInTheDocument();
    expect(screen.getByText('dataStorage.sectionDesc')).toBeInTheDocument();
    expect(screen.getByText('demo.settingsDesc')).toBeInTheDocument();
  });

  // ── Display section ───────────────────────────────────────────────────

  it('renders language selector', () => {
    render(<SettingsPage {...defaultProps()} />);
    expect(screen.getByTestId('language-selector')).toBeInTheDocument();
  });

  it('renders theme select with current preference', () => {
    render(<SettingsPage {...defaultProps({ themePreference: 'dark' })} />);
    expect(screen.getByText('settings.theme.label')).toBeInTheDocument();
  });

  it('renders currency select with loaded currencies', () => {
    render(<SettingsPage {...defaultProps()} />);

    expect(screen.getByTestId('currency-select-value')).toHaveTextContent('EUR');
    expect(screen.getByTestId('currency-option-EUR')).toBeInTheDocument();
    expect(screen.getByTestId('currency-option-USD')).toBeInTheDocument();
  });

  it('shows currency saved badge when currencySaved is true', () => {
    hookReturn = { ...hookReturn, currencySaved: true };
    render(<SettingsPage {...defaultProps()} />);

    const savedElements = screen.getAllByText('settings.saved');
    const currencySaved = savedElements.find(
      (el) =>
        el.closest('[data-testid="currency-select"]')?.parentElement != null ||
        el.tagName === 'SPAN',
    );
    expect(currencySaved).toBeDefined();
  });

  it('calls handleCurrencySelect when a currency is clicked', async () => {
    const user = userEvent.setup();
    render(<SettingsPage {...defaultProps()} />);

    await user.click(screen.getByTestId('currency-option-USD'));

    expect(mockHandleCurrencySelect).toHaveBeenCalledWith(USD);
  });

  // ── OXR API key section ───────────────────────────────────────────────

  it('renders API key input with stored value', () => {
    hookReturn = { ...hookReturn, apiKey: 'my-secret-key' };
    render(<SettingsPage {...defaultProps()} />);

    expect(screen.getByLabelText('settings.oxrApiKey')).toHaveValue('my-secret-key');
  });

  it('calls setApiKey when typing in API key input', async () => {
    const user = userEvent.setup();
    render(<SettingsPage {...defaultProps()} />);

    const input = screen.getByLabelText('settings.oxrApiKey');
    await user.type(input, 'k');

    expect(mockSetApiKey).toHaveBeenCalled();
  });

  it('calls handleSaveApiKey when save button is clicked', async () => {
    const user = userEvent.setup();
    render(<SettingsPage {...defaultProps()} />);

    await user.click(screen.getByText('settings.saveApiKey'));

    expect(mockHandleSaveApiKey).toHaveBeenCalledOnce();
  });

  it('shows saved state on API key button when apiKeySaved is true', () => {
    hookReturn = { ...hookReturn, apiKeySaved: true };
    render(<SettingsPage {...defaultProps()} />);

    const buttons = screen.getAllByRole('button');
    const saveBtn = buttons.find(
      (b) => b.textContent === 'settings.saved' && !b.closest('[data-testid="currency-select"]'),
    );
    expect(saveBtn).toBeDefined();
  });

  // ── Data Storage section ──────────────────────────────────────────────

  it('displays the database location path', () => {
    render(
      <SettingsPage
        {...defaultProps({ dbLocationPath: '/custom/path/db.sqlite', dbLocationIsDefault: false })}
      />,
    );
    expect(screen.getByText('/custom/path/db.sqlite')).toBeInTheDocument();
  });

  it('shows default badge when using default location', () => {
    render(<SettingsPage {...defaultProps({ dbLocationIsDefault: true })} />);
    expect(screen.getByText('dataStorage.isDefault')).toBeInTheDocument();
  });

  it('does not show default badge for custom location', () => {
    render(<SettingsPage {...defaultProps({ dbLocationIsDefault: false })} />);
    expect(screen.queryByText('dataStorage.isDefault')).not.toBeInTheDocument();
  });

  it('calls onChangeDbLocation when change button is clicked', async () => {
    const props = defaultProps();
    const user = userEvent.setup();
    render(<SettingsPage {...props} />);

    await user.click(screen.getByText('dataStorage.changeButton'));
    expect(props.onChangeDbLocation).toHaveBeenCalledOnce();
  });

  it('shows reset button only when not using default location', () => {
    const { rerender } = render(<SettingsPage {...defaultProps({ dbLocationIsDefault: true })} />);
    expect(screen.queryByText('dataStorage.resetButton')).not.toBeInTheDocument();

    rerender(<SettingsPage {...defaultProps({ dbLocationIsDefault: false })} />);
    expect(screen.getByText('dataStorage.resetButton')).toBeInTheDocument();
  });

  it('calls onResetDbLocation when reset button is clicked', async () => {
    const props = defaultProps({ dbLocationIsDefault: false });
    const user = userEvent.setup();
    render(<SettingsPage {...props} />);

    await user.click(screen.getByText('dataStorage.resetButton'));
    expect(props.onResetDbLocation).toHaveBeenCalledOnce();
  });

  it('disables db buttons in demo mode', () => {
    render(<SettingsPage {...defaultProps({ isDemoMode: true, dbLocationIsDefault: false })} />);

    expect(screen.getByText('dataStorage.changeButton')).toBeDisabled();
    expect(screen.getByText('dataStorage.resetButton')).toBeDisabled();
    expect(screen.getByText('dataStorage.disabledInDemoMode')).toBeInTheDocument();
  });

  // ── Demo Mode section ─────────────────────────────────────────────────

  it('shows start button when not in demo mode', () => {
    render(<SettingsPage {...defaultProps({ isDemoMode: false })} />);

    expect(screen.getByText('demo.startButton')).toBeInTheDocument();
    expect(screen.getByText('demo.settingsNote')).toBeInTheDocument();
    expect(screen.queryByText('demo.stopButton')).not.toBeInTheDocument();
  });

  it('shows stop button when in demo mode', () => {
    render(<SettingsPage {...defaultProps({ isDemoMode: true })} />);

    expect(screen.getByText('demo.stopButton')).toBeInTheDocument();
    expect(screen.queryByText('demo.startButton')).not.toBeInTheDocument();
  });

  it('calls onEnterDemoMode when start button is clicked', async () => {
    const props = defaultProps({ isDemoMode: false });
    const user = userEvent.setup();
    render(<SettingsPage {...props} />);

    await user.click(screen.getByText('demo.startButton'));
    expect(props.onEnterDemoMode).toHaveBeenCalledOnce();
  });

  it('calls onExitDemoMode when stop button is clicked', async () => {
    const props = defaultProps({ isDemoMode: true });
    const user = userEvent.setup();
    render(<SettingsPage {...props} />);

    await user.click(screen.getByText('demo.stopButton'));
    expect(props.onExitDemoMode).toHaveBeenCalledOnce();
  });
});
