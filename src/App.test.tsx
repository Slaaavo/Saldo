import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import App from './App';
import type { SnapshotRow, EventWithData, Currency, ModalState, DbLocationInfo } from './types';
import type { ThemePreference } from './hooks/useTheme';

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

// ── Mock sonner ─────────────────────────────────────────────────────────────
const mockToastSuccess = vi.fn();
const mockToastError = vi.fn();
const mockToastWarning = vi.fn();

vi.mock('sonner', () => ({
  Toaster: () => <div data-testid="toaster" />,
  toast: {
    success: (...args: unknown[]) => mockToastSuccess(...args),
    error: (...args: unknown[]) => mockToastError(...args),
    warning: (...args: unknown[]) => mockToastWarning(...args),
  },
}));

// ── Mock API ────────────────────────────────────────────────────────────────
const mockFetchFxRates = vi.fn();
const mockEnterDemoMode = vi.fn();
const mockExitDemoMode = vi.fn();
const mockIsDemoMode = vi.fn();
const mockGetDbLocation = vi.fn();
const mockPickDbFolder = vi.fn();
const mockChangeDbLocation = vi.fn();
const mockResetDbLocation = vi.fn();
const mockCheckDefaultDb = vi.fn();

vi.mock('./api', () => ({
  fetchFxRates: (...args: unknown[]) => mockFetchFxRates(...args),
  enterDemoMode: (...args: unknown[]) => mockEnterDemoMode(...args),
  exitDemoMode: (...args: unknown[]) => mockExitDemoMode(...args),
  isDemoMode: (...args: unknown[]) => mockIsDemoMode(...args),
  getDbLocation: (...args: unknown[]) => mockGetDbLocation(...args),
  pickDbFolder: (...args: unknown[]) => mockPickDbFolder(...args),
  changeDbLocation: (...args: unknown[]) => mockChangeDbLocation(...args),
  resetDbLocation: (...args: unknown[]) => mockResetDbLocation(...args),
  checkDefaultDb: (...args: unknown[]) => mockCheckDefaultDb(...args),
}));

// ── Mock hooks ──────────────────────────────────────────────────────────────
const mockSetThemePreference = vi.fn();
let themeReturn = {
  theme: 'light' as 'light' | 'dark',
  themePreference: 'system' as ThemePreference,
  setThemePreference: mockSetThemePreference,
};

vi.mock('./hooks/useTheme', () => ({
  useTheme: () => themeReturn,
}));

const mockSetModalState = vi.fn();
const mockCloseModal = vi.fn();
let modalStateValue: ModalState = { type: 'none' };

vi.mock('./hooks/useModalManager', () => ({
  useModalManager: () => ({
    modalState: modalStateValue,
    setModalState: mockSetModalState,
    closeModal: mockCloseModal,
  }),
}));

const mockRefresh = vi.fn().mockResolvedValue(undefined);
const mockHandleCreateBalanceUpdate = vi.fn();
const mockHandleEditBalanceUpdate = vi.fn();
const mockHandleDeleteEvent = vi.fn();
const mockHandleCreateAccount = vi.fn();
const mockHandleRenameAccount = vi.fn();
const mockHandleDeleteAccount = vi.fn();
const mockHandleBulkUpdateSubmit = vi.fn();
const mockHandleSaveOrder = vi.fn();
const mockHandleConsolidationCurrencyChange = vi.fn().mockResolvedValue(undefined);
const mockHandleUpdateAssetValue = vi.fn();
const mockHandleSetAccountAssetLinks = vi.fn();
const mockSetSelectedDate = vi.fn();

const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false };

let financeDataReturn: Record<string, unknown>;

function defaultFinanceData(overrides?: Partial<typeof financeDataReturn>) {
  return {
    selectedDate: '2026-01-15',
    setSelectedDate: mockSetSelectedDate,
    snapshot: [] as SnapshotRow[],
    events: [] as EventWithData[],
    consolidationCurrency: EUR,
    refresh: mockRefresh,
    handleConsolidationCurrencyChange: mockHandleConsolidationCurrencyChange,
    missingFxCurrencies: [] as string[],
    ...overrides,
  };
}

vi.mock('./hooks/useFinanceData', () => ({
  useFinanceData: () => financeDataReturn,
}));

vi.mock('./hooks/useModalActions', () => ({
  useModalActions: () => ({
    handleCreateBalanceUpdate: mockHandleCreateBalanceUpdate,
    handleEditBalanceUpdate: mockHandleEditBalanceUpdate,
    handleDeleteEvent: mockHandleDeleteEvent,
    handleCreateAccount: mockHandleCreateAccount,
    handleRenameAccount: mockHandleRenameAccount,
    handleDeleteAccount: mockHandleDeleteAccount,
    handleBulkUpdateSubmit: mockHandleBulkUpdateSubmit,
    handleSaveOrder: mockHandleSaveOrder,
    handleUpdateAssetValue: mockHandleUpdateAssetValue,
    handleSetAccountAssetLinks: mockHandleSetAccountAssetLinks,
    handleCreateAssetSuccess: vi.fn().mockImplementation(async () => {
      mockCloseModal();
      await mockRefresh();
    }),
  }),
}));

// ── Mock child components ───────────────────────────────────────────────────
let capturedDashboardProps: Record<string, unknown> = {};

vi.mock('./components/Header', () => ({
  default: (props: { pageTitle: string; showDatePicker: boolean }) => (
    <div data-testid="header">
      <span data-testid="page-title">{props.pageTitle}</span>
      {props.showDatePicker && <span data-testid="date-picker-visible" />}
    </div>
  ),
}));

vi.mock('./components/Sidebar', () => ({
  default: (props: {
    currentView: string;
    onNavigate: (view: string) => void;
    collapsed: boolean;
    onToggleCollapse: () => void;
  }) => (
    <div data-testid="sidebar">
      <span data-testid="current-view">{props.currentView}</span>
      <button data-testid="nav-dashboard" onClick={() => props.onNavigate('dashboard')}>
        Dashboard
      </button>
      <button data-testid="nav-settings" onClick={() => props.onNavigate('settings')}>
        Settings
      </button>
      <button data-testid="nav-fx-rates" onClick={() => props.onNavigate('fx-rates')}>
        FxRates
      </button>
      <button data-testid="nav-units" onClick={() => props.onNavigate('units')}>
        Units
      </button>
      <button data-testid="toggle-collapse" onClick={props.onToggleCollapse}>
        Toggle
      </button>
      <span data-testid="sidebar-collapsed">{String(props.collapsed)}</span>
    </div>
  ),
}));

vi.mock('./components/DemoModeBanner', () => ({
  default: (props: { onExit: () => void }) => (
    <div data-testid="demo-banner">
      <button data-testid="exit-demo" onClick={props.onExit}>
        Exit Demo
      </button>
    </div>
  ),
}));

vi.mock('./pages/DashboardView', () => ({
  default: (props: Record<string, unknown>) => {
    capturedDashboardProps = props;
    return <div data-testid="dashboard-view" />;
  },
}));

vi.mock('./pages/SettingsPage', () => ({
  default: (props: {
    isDemoMode: boolean;
    onEnterDemoMode: () => void;
    onExitDemoMode: () => void;
    onChangeDbLocation: () => void;
    onResetDbLocation: () => void;
  }) => (
    <div data-testid="settings-page">
      <button data-testid="enter-demo" onClick={props.onEnterDemoMode}>
        Enter Demo
      </button>
      <button data-testid="exit-demo-settings" onClick={props.onExitDemoMode}>
        Exit Demo
      </button>
      <button data-testid="change-db" onClick={props.onChangeDbLocation}>
        Change DB
      </button>
      <button data-testid="reset-db" onClick={props.onResetDbLocation}>
        Reset DB
      </button>
      <span data-testid="demo-mode-value">{String(props.isDemoMode)}</span>
    </div>
  ),
}));

vi.mock('./pages/FxRatesPage', () => ({
  default: () => <div data-testid="fx-rates-page" />,
}));

vi.mock('./pages/UnitsPage', () => ({
  default: () => <div data-testid="units-page" />,
}));

// Mock all modals
vi.mock('./components/CreateBalanceUpdateModal', () => ({
  default: (props: { onSubmit: () => void; onClose: () => void }) => (
    <div data-testid="create-balance-modal">
      <button data-testid="create-balance-submit" onClick={props.onSubmit}>
        Submit
      </button>
      <button data-testid="create-balance-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}));

vi.mock('./components/EditBalanceUpdateModal', () => ({
  default: (props: { onClose: () => void }) => (
    <div data-testid="edit-balance-modal">
      <button data-testid="edit-balance-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}));

vi.mock('./components/CreateAccountModal', () => ({
  default: (props: { accountType?: string; onClose: () => void }) => (
    <div data-testid="create-account-modal">
      <span data-testid="create-account-type">{props.accountType}</span>
      <button data-testid="create-account-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}));

vi.mock('./components/CreateAssetModal', () => ({
  default: (props: { onSuccess: () => void; onClose: () => void }) => (
    <div data-testid="create-asset-modal">
      <button data-testid="create-asset-success" onClick={props.onSuccess}>
        Success
      </button>
      <button data-testid="create-asset-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}));

vi.mock('./components/UpdateAssetValueModal', () => ({
  default: (props: { onClose: () => void }) => (
    <div data-testid="update-asset-modal">
      <button data-testid="update-asset-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}));

vi.mock('./components/RenameAccountModal', () => ({
  default: (props: { onClose: () => void }) => (
    <div data-testid="rename-account-modal">
      <button data-testid="rename-account-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}));

vi.mock('./components/ConfirmDialog', () => ({
  default: (props: { message: string; onConfirm: () => void; onCancel: () => void }) => (
    <div data-testid="confirm-dialog">
      <span data-testid="confirm-message">{props.message}</span>
      <button data-testid="confirm-ok" onClick={props.onConfirm}>
        Confirm
      </button>
      <button data-testid="confirm-cancel" onClick={props.onCancel}>
        Cancel
      </button>
    </div>
  ),
}));

vi.mock('./components/BulkUpdateBalanceModal', () => ({
  default: (props: { onClose: () => void }) => (
    <div data-testid="bulk-update-modal">
      <button data-testid="bulk-update-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}));

vi.mock('./components/DbLocationChoiceDialog', () => ({
  default: (props: { onAction: (action: string) => void; onCancel: () => void }) => (
    <div data-testid="db-choice-dialog">
      <button data-testid="db-choice-move" onClick={() => props.onAction('move')}>
        Move
      </button>
      <button data-testid="db-choice-fresh" onClick={() => props.onAction('fresh')}>
        Fresh
      </button>
      <button data-testid="db-choice-cancel" onClick={props.onCancel}>
        Cancel
      </button>
    </div>
  ),
}));

vi.mock('./components/ReorderModal', () => ({
  default: (props: { title: string; onSave: () => void; onClose: () => void }) => (
    <div data-testid="reorder-modal">
      <span data-testid="reorder-title">{props.title}</span>
      <button data-testid="reorder-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}));

vi.mock('./components/ManageLinkedAssetsModal', () => ({
  default: (props: { onClose: () => void }) => (
    <div data-testid="manage-linked-assets-modal">
      <button data-testid="manage-links-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}));

// ── Helpers ─────────────────────────────────────────────────────────────────
function makeSnapshot(overrides?: Partial<SnapshotRow>): SnapshotRow {
  return {
    accountId: 1,
    accountName: 'Checking',
    accountType: 'account',
    balanceMinor: 100000,
    currencyCode: 'EUR',
    currencyMinorUnits: 2,
    isCustom: false,
    convertedBalanceMinor: 100000,
    fxRateMissing: false,
    allocatedTotalMinor: 0,
    linkedAllocationsBalanceMinor: 0,
    overAllocationBuckets: [],
    linkedAllocations: [],
    linkedAllocationsFromAssetsMinor: 0,
    isLinkedToAsset: false,
    linkedAssetIds: [],
    ...overrides,
  };
}

function makeBucket(overrides?: Partial<SnapshotRow>): SnapshotRow {
  return makeSnapshot({
    accountId: 10,
    accountName: 'Vacation',
    accountType: 'bucket',
    balanceMinor: 50000,
    convertedBalanceMinor: 50000,
    ...overrides,
  });
}

function makeAsset(overrides?: Partial<SnapshotRow>): SnapshotRow {
  return makeSnapshot({
    accountId: 20,
    accountName: 'House',
    accountType: 'asset',
    balanceMinor: 30000000,
    convertedBalanceMinor: 30000000,
    ...overrides,
  });
}

function makeEvent(overrides?: Partial<EventWithData>): EventWithData {
  return {
    id: 1,
    accountId: 1,
    accountName: 'Checking',
    accountType: 'account',
    eventType: 'balance_update',
    eventDate: '2026-01-15T10:00:00',
    amountMinor: 100000,
    note: null,
    createdAt: '2026-01-15T10:00:00',
    currencyCode: 'EUR',
    currencyMinorUnits: 2,
    ...overrides,
  };
}

const defaultDbLocation: DbLocationInfo = {
  currentPath: '/default/path',
  isDefault: true,
  isDemoMode: false,
  fallbackWarning: false,
};

function setupDefaultMocks(overrides?: { snapshot?: SnapshotRow[]; events?: EventWithData[] }) {
  mockIsDemoMode.mockResolvedValue(false);
  mockGetDbLocation.mockResolvedValue(defaultDbLocation);
  mockEnterDemoMode.mockResolvedValue(undefined);
  mockExitDemoMode.mockResolvedValue(undefined);
  mockFetchFxRates.mockResolvedValue([]);
  mockPickDbFolder.mockResolvedValue(null);
  mockChangeDbLocation.mockResolvedValue(undefined);
  mockResetDbLocation.mockResolvedValue(undefined);
  mockCheckDefaultDb.mockResolvedValue(false);

  financeDataReturn = defaultFinanceData({
    snapshot: overrides?.snapshot ?? [makeSnapshot()],
    events: overrides?.events ?? [makeEvent()],
  });
}

// ── Tests ───────────────────────────────────────────────────────────────────
describe('App', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    modalStateValue = { type: 'none' };
    capturedDashboardProps = {};
    themeReturn = {
      theme: 'light',
      themePreference: 'system',
      setThemePreference: mockSetThemePreference,
    };
    setupDefaultMocks();
  });

  // ── Initial render & mount behavior ──

  describe('initial render', () => {
    it('renders the sidebar and header', async () => {
      render(<App />);
      await waitFor(() => {
        expect(screen.getByTestId('sidebar')).toBeInTheDocument();
        expect(screen.getByTestId('header')).toBeInTheDocument();
      });
    });

    it('renders the toaster', () => {
      render(<App />);
      expect(screen.getByTestId('toaster')).toBeInTheDocument();
    });

    it('checks demo mode on mount', () => {
      render(<App />);
      expect(mockIsDemoMode).toHaveBeenCalledOnce();
    });

    it('loads DB location on mount', () => {
      render(<App />);
      expect(mockGetDbLocation).toHaveBeenCalledOnce();
    });

    it('defaults to dashboard view', () => {
      render(<App />);
      expect(screen.getByTestId('dashboard-view')).toBeInTheDocument();
      expect(screen.getByTestId('current-view')).toHaveTextContent('dashboard');
    });
  });

  // ── Navigation ──

  describe('navigation', () => {
    it('navigates to settings page', async () => {
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-settings'));
      expect(screen.getByTestId('settings-page')).toBeInTheDocument();
      expect(screen.queryByTestId('dashboard-view')).not.toBeInTheDocument();
    });

    it('navigates to fx-rates page', async () => {
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-fx-rates'));
      expect(screen.getByTestId('fx-rates-page')).toBeInTheDocument();
      expect(screen.queryByTestId('dashboard-view')).not.toBeInTheDocument();
    });

    it('navigates to units page', async () => {
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-units'));
      expect(screen.getByTestId('units-page')).toBeInTheDocument();
      expect(screen.queryByTestId('dashboard-view')).not.toBeInTheDocument();
    });

    it('navigates back to dashboard', async () => {
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-settings'));
      expect(screen.getByTestId('settings-page')).toBeInTheDocument();

      await user.click(screen.getByTestId('nav-dashboard'));
      expect(screen.getByTestId('dashboard-view')).toBeInTheDocument();
      expect(screen.queryByTestId('settings-page')).not.toBeInTheDocument();
    });
  });

  // ── Page title ──

  describe('page title', () => {
    it('shows dashboard title by default', () => {
      render(<App />);
      expect(screen.getByTestId('page-title')).toHaveTextContent('sidebar.dashboard');
    });

    it('shows settings title on settings page', async () => {
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-settings'));
      expect(screen.getByTestId('page-title')).toHaveTextContent('sidebar.settings');
    });

    it('shows fx-rates title on fx-rates page', async () => {
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-fx-rates'));
      expect(screen.getByTestId('page-title')).toHaveTextContent('sidebar.fxRates');
    });

    it('shows units title on units page', async () => {
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-units'));
      expect(screen.getByTestId('page-title')).toHaveTextContent('sidebar.units');
    });
  });

  // ── Date picker visibility ──

  describe('date picker visibility', () => {
    it('shows date picker on dashboard view', () => {
      render(<App />);
      expect(screen.getByTestId('date-picker-visible')).toBeInTheDocument();
    });

    it('hides date picker on settings page', async () => {
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-settings'));
      expect(screen.queryByTestId('date-picker-visible')).not.toBeInTheDocument();
    });
  });

  // ── Sidebar collapse ──

  describe('sidebar collapse', () => {
    it('starts expanded (collapsed=false)', () => {
      render(<App />);
      expect(screen.getByTestId('sidebar-collapsed')).toHaveTextContent('false');
    });

    it('toggles collapsed state', async () => {
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('toggle-collapse'));
      expect(screen.getByTestId('sidebar-collapsed')).toHaveTextContent('true');

      await user.click(screen.getByTestId('toggle-collapse'));
      expect(screen.getByTestId('sidebar-collapsed')).toHaveTextContent('false');
    });
  });

  // ── Demo mode banner ──

  describe('demo mode banner', () => {
    it('does not show demo banner when not in demo mode', async () => {
      mockIsDemoMode.mockResolvedValue(false);
      render(<App />);
      await waitFor(() => {
        expect(screen.queryByTestId('demo-banner')).not.toBeInTheDocument();
      });
    });

    it('shows demo banner when in demo mode', async () => {
      mockIsDemoMode.mockResolvedValue(true);
      render(<App />);
      await waitFor(() => {
        expect(screen.getByTestId('demo-banner')).toBeInTheDocument();
      });
    });

    it('exits demo mode when banner exit is clicked', async () => {
      mockIsDemoMode.mockResolvedValue(true);
      mockExitDemoMode.mockResolvedValue(undefined);
      const user = userEvent.setup();
      render(<App />);
      await waitFor(() => {
        expect(screen.getByTestId('demo-banner')).toBeInTheDocument();
      });
      await user.click(screen.getByTestId('exit-demo'));
      expect(mockExitDemoMode).toHaveBeenCalledOnce();
    });
  });

  // ── Demo mode enter/exit from settings ──

  describe('demo mode enter/exit', () => {
    it('enters demo mode and navigates to dashboard', async () => {
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-settings'));

      await user.click(screen.getByTestId('enter-demo'));

      await waitFor(() => {
        expect(mockEnterDemoMode).toHaveBeenCalledOnce();
      });
    });

    it('exits demo mode and shows success toast', async () => {
      mockIsDemoMode.mockResolvedValue(true);
      const user = userEvent.setup();
      render(<App />);

      await waitFor(() => {
        expect(screen.getByTestId('demo-banner')).toBeInTheDocument();
      });

      await user.click(screen.getByTestId('nav-settings'));
      await user.click(screen.getByTestId('exit-demo-settings'));

      await waitFor(() => {
        expect(mockExitDemoMode).toHaveBeenCalledOnce();
        expect(mockToastSuccess).toHaveBeenCalledWith('demo.exitedToast');
      });
    });

    it('shows error toast when enter demo mode fails', async () => {
      mockEnterDemoMode.mockRejectedValue(new Error('Demo error'));
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-settings'));
      await user.click(screen.getByTestId('enter-demo'));

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalledWith('Demo error');
      });
    });

    it('shows error toast when exit demo mode fails', async () => {
      mockIsDemoMode.mockResolvedValue(true);
      mockExitDemoMode.mockRejectedValue(new Error('Exit error'));
      const user = userEvent.setup();
      render(<App />);

      await waitFor(() => {
        expect(screen.getByTestId('demo-banner')).toBeInTheDocument();
      });

      await user.click(screen.getByTestId('exit-demo'));

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalledWith('Exit error');
      });
    });
  });

  // ── DB location ──

  describe('DB location', () => {
    it('shows fallback warning toast when DB location has fallbackWarning', async () => {
      mockGetDbLocation.mockResolvedValue({
        ...defaultDbLocation,
        fallbackWarning: true,
      });
      render(<App />);
      await waitFor(() => {
        expect(mockToastWarning).toHaveBeenCalledWith('dataStorage.toasts.fallbackWarning');
      });
    });

    it('opens confirmSwitchDb modal when picking folder with existing db', async () => {
      mockPickDbFolder.mockResolvedValue({ folder: '/new/path', dbExists: true });
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-settings'));
      await user.click(screen.getByTestId('change-db'));

      await waitFor(() => {
        expect(mockSetModalState).toHaveBeenCalledWith({
          type: 'confirmSwitchDb',
          folder: '/new/path',
        });
      });
    });

    it('opens dbLocationChoice modal when picking folder without existing db', async () => {
      mockPickDbFolder.mockResolvedValue({ folder: '/new/path', dbExists: false });
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-settings'));
      await user.click(screen.getByTestId('change-db'));

      await waitFor(() => {
        expect(mockSetModalState).toHaveBeenCalledWith({
          type: 'dbLocationChoice',
          folder: '/new/path',
          isReset: false,
        });
      });
    });

    it('does nothing when pickDbFolder returns null (cancelled)', async () => {
      mockPickDbFolder.mockResolvedValue(null);
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-settings'));
      await user.click(screen.getByTestId('change-db'));

      await waitFor(() => {
        expect(mockPickDbFolder).toHaveBeenCalledOnce();
      });
      expect(mockSetModalState).not.toHaveBeenCalled();
    });

    it('shows error toast when change DB location fails', async () => {
      mockPickDbFolder.mockRejectedValue(new Error('pick error'));
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-settings'));
      await user.click(screen.getByTestId('change-db'));

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalled();
      });
    });

    it('opens confirmResetDbLocation when default db exists', async () => {
      mockCheckDefaultDb.mockResolvedValue(true);
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-settings'));
      await user.click(screen.getByTestId('reset-db'));

      await waitFor(() => {
        expect(mockSetModalState).toHaveBeenCalledWith({ type: 'confirmResetDbLocation' });
      });
    });

    it('opens dbLocationChoice for reset when default db does not exist', async () => {
      mockCheckDefaultDb.mockResolvedValue(false);
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-settings'));
      await user.click(screen.getByTestId('reset-db'));

      await waitFor(() => {
        expect(mockSetModalState).toHaveBeenCalledWith({
          type: 'dbLocationChoice',
          folder: '',
          isReset: true,
        });
      });
    });

    it('shows error toast when reset DB location fails', async () => {
      mockCheckDefaultDb.mockRejectedValue(new Error('reset check error'));
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('nav-settings'));
      await user.click(screen.getByTestId('reset-db'));

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalled();
      });
    });
  });

  // ── Computed values passed to DashboardView ──

  describe('computed values for DashboardView', () => {
    it('splits snapshot into accounts, buckets, and assets', () => {
      const acct = makeSnapshot({ accountId: 1, accountType: 'account' });
      const bucket = makeBucket({ accountId: 10, accountType: 'bucket' });
      const asset = makeAsset({ accountId: 20, accountType: 'asset' });

      setupDefaultMocks({ snapshot: [acct, bucket, asset] });
      render(<App />);

      expect(capturedDashboardProps.accounts).toEqual([acct]);
      expect(capturedDashboardProps.buckets).toEqual([bucket]);
      expect(capturedDashboardProps.assets).toEqual([asset]);
    });

    it('computes totalMinor as sum of non-asset-linked accounts', () => {
      const acct1 = makeSnapshot({
        accountId: 1,
        convertedBalanceMinor: 100000,
        isLinkedToAsset: false,
      });
      const acct2 = makeSnapshot({
        accountId: 2,
        convertedBalanceMinor: 50000,
        isLinkedToAsset: true,
      });

      setupDefaultMocks({ snapshot: [acct1, acct2] });
      render(<App />);

      // totalMinor = liquidMinor = acct1 (100000) — acct2 is linked to asset
      expect(capturedDashboardProps.totalMinor).toBe(100000);
    });

    it('computes leftToSpendMinor correctly', () => {
      const acct = makeSnapshot({
        accountId: 1,
        convertedBalanceMinor: 200000,
        isLinkedToAsset: false,
      });
      const bucket = makeBucket({
        accountId: 10,
        convertedBalanceMinor: 80000,
        linkedAllocationsFromAssetsMinor: 20000,
      });

      setupDefaultMocks({ snapshot: [acct, bucket] });
      render(<App />);

      // leftToSpend = liquidMinor(200000) - bucketsMinor(80000) + assetAllocationsInBuckets(20000)
      expect(capturedDashboardProps.leftToSpendMinor).toBe(140000);
    });

    it('computes netWorthMinor as accounts + assets', () => {
      const acct = makeSnapshot({
        accountId: 1,
        convertedBalanceMinor: 100000,
      });
      const asset = makeAsset({
        accountId: 20,
        convertedBalanceMinor: 500000,
      });

      setupDefaultMocks({ snapshot: [acct, asset] });
      render(<App />);

      // netWorth = allAccountsMinor(100000) + assetTotalMinor(500000)
      expect(capturedDashboardProps.netWorthMinor).toBe(600000);
    });

    it('sets hasAssets to true when assets exist', () => {
      setupDefaultMocks({ snapshot: [makeSnapshot(), makeAsset()] });
      render(<App />);
      expect(capturedDashboardProps.hasAssets).toBe(true);
    });

    it('sets hasAssets to false when no assets exist', () => {
      setupDefaultMocks({ snapshot: [makeSnapshot()] });
      render(<App />);
      expect(capturedDashboardProps.hasAssets).toBe(false);
    });
  });

  // ── Modal rendering ──

  describe('modal rendering', () => {
    it('renders CreateBalanceUpdateModal when modalState is createBalanceUpdate', () => {
      modalStateValue = { type: 'createBalanceUpdate' };
      render(<App />);
      expect(screen.getByTestId('create-balance-modal')).toBeInTheDocument();
    });

    it('renders EditBalanceUpdateModal when modalState is editBalanceUpdate', () => {
      modalStateValue = { type: 'editBalanceUpdate', event: makeEvent() };
      render(<App />);
      expect(screen.getByTestId('edit-balance-modal')).toBeInTheDocument();
    });

    it('renders CreateAccountModal when modalState is createAccount', () => {
      modalStateValue = { type: 'createAccount', accountType: 'account' };
      render(<App />);
      expect(screen.getByTestId('create-account-modal')).toBeInTheDocument();
      expect(screen.getByTestId('create-account-type')).toHaveTextContent('account');
    });

    it('renders CreateAssetModal when modalState is createAsset', () => {
      modalStateValue = { type: 'createAsset' };
      render(<App />);
      expect(screen.getByTestId('create-asset-modal')).toBeInTheDocument();
    });

    it('renders RenameAccountModal when modalState is renameAccount', () => {
      modalStateValue = { type: 'renameAccount', accountId: 1, currentName: 'Test' };
      render(<App />);
      expect(screen.getByTestId('rename-account-modal')).toBeInTheDocument();
    });

    it('renders ConfirmDialog for confirmDeleteAccount', () => {
      modalStateValue = {
        type: 'confirmDeleteAccount',
        accountId: 1,
        name: 'Checking',
        accountType: 'account',
      };
      render(<App />);
      expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument();
    });

    it('renders ConfirmDialog for confirmDeleteEvent', () => {
      modalStateValue = { type: 'confirmDeleteEvent', eventId: 1 };
      render(<App />);
      expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument();
    });

    it('renders BulkUpdateBalanceModal when modalState is bulkUpdateBalance', () => {
      modalStateValue = { type: 'bulkUpdateBalance' };
      render(<App />);
      expect(screen.getByTestId('bulk-update-modal')).toBeInTheDocument();
    });

    it('renders ConfirmDialog for fetchFxRatePrompt', () => {
      modalStateValue = { type: 'fetchFxRatePrompt', date: '2026-01-15' };
      render(<App />);
      expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument();
    });

    it('renders UpdateAssetValueModal when modalState is updateAssetValue', () => {
      modalStateValue = {
        type: 'updateAssetValue',
        accountId: 20,
        accountName: 'House',
        currencyCode: 'EUR',
        currencyMinorUnits: 2,
        isCustomUnit: false,
        balanceMinor: 30000000,
      };
      render(<App />);
      expect(screen.getByTestId('update-asset-modal')).toBeInTheDocument();
    });

    it('renders ReorderModal for reorderAccounts', () => {
      setupDefaultMocks({ snapshot: [makeSnapshot()] });
      modalStateValue = { type: 'reorderAccounts' };
      render(<App />);
      expect(screen.getByTestId('reorder-modal')).toBeInTheDocument();
      expect(screen.getByTestId('reorder-title')).toHaveTextContent('reorder.titleAccounts');
    });

    it('renders ReorderModal for reorderBuckets', () => {
      setupDefaultMocks({ snapshot: [makeBucket()] });
      modalStateValue = { type: 'reorderBuckets' };
      render(<App />);
      expect(screen.getByTestId('reorder-modal')).toBeInTheDocument();
      expect(screen.getByTestId('reorder-title')).toHaveTextContent('reorder.titleBuckets');
    });

    it('renders ReorderModal for reorderAssets', () => {
      setupDefaultMocks({ snapshot: [makeAsset()] });
      modalStateValue = { type: 'reorderAssets' };
      render(<App />);
      expect(screen.getByTestId('reorder-modal')).toBeInTheDocument();
      expect(screen.getByTestId('reorder-title')).toHaveTextContent('reorder.titleAssets');
    });

    it('renders ManageLinkedAssetsModal when modalState is manageLinkedAssets', () => {
      modalStateValue = { type: 'manageLinkedAssets', accountId: 1, accountName: 'Checking' };
      render(<App />);
      expect(screen.getByTestId('manage-linked-assets-modal')).toBeInTheDocument();
    });

    it('does not render any modal when modalState is none', () => {
      modalStateValue = { type: 'none' };
      render(<App />);
      expect(screen.queryByTestId('create-balance-modal')).not.toBeInTheDocument();
      expect(screen.queryByTestId('edit-balance-modal')).not.toBeInTheDocument();
      expect(screen.queryByTestId('create-account-modal')).not.toBeInTheDocument();
      expect(screen.queryByTestId('confirm-dialog')).not.toBeInTheDocument();
      expect(screen.queryByTestId('bulk-update-modal')).not.toBeInTheDocument();
      expect(screen.queryByTestId('reorder-modal')).not.toBeInTheDocument();
    });
  });

  // ── ConfirmDialog behavior ──

  describe('confirm dialog behavior', () => {
    it('calls handleDeleteAccount when confirmDeleteAccount is confirmed', async () => {
      modalStateValue = {
        type: 'confirmDeleteAccount',
        accountId: 42,
        name: 'OldAccount',
        accountType: 'account',
      };
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('confirm-ok'));
      expect(mockHandleDeleteAccount).toHaveBeenCalledWith(42);
    });

    it('calls closeModal when confirmDeleteAccount is cancelled', async () => {
      modalStateValue = {
        type: 'confirmDeleteAccount',
        accountId: 42,
        name: 'OldAccount',
        accountType: 'account',
      };
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('confirm-cancel'));
      expect(mockCloseModal).toHaveBeenCalledOnce();
    });

    it('calls handleDeleteEvent when confirmDeleteEvent is confirmed', async () => {
      modalStateValue = { type: 'confirmDeleteEvent', eventId: 99 };
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('confirm-ok'));
      expect(mockHandleDeleteEvent).toHaveBeenCalledWith(99);
    });

    it('shows asset deletion warning for linked assets', () => {
      const acctLinkedToAsset = makeSnapshot({
        accountId: 1,
        accountName: 'Investment Account',
        linkedAssetIds: [20],
      });
      setupDefaultMocks({ snapshot: [acctLinkedToAsset] });
      modalStateValue = {
        type: 'confirmDeleteAccount',
        accountId: 20,
        name: 'House',
        accountType: 'asset',
      };
      render(<App />);
      expect(screen.getByTestId('confirm-message')).toHaveTextContent(
        'modals.confirm.deleteAssetWithLinks',
      );
    });
  });

  // ── CreateAssetModal onSuccess ──

  describe('CreateAssetModal onSuccess', () => {
    it('calls closeModal and refresh when asset is created', async () => {
      modalStateValue = { type: 'createAsset' };
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('create-asset-success'));

      expect(mockCloseModal).toHaveBeenCalledOnce();
      expect(mockRefresh).toHaveBeenCalledOnce();
    });
  });

  // ── FxRate prompt modal ──

  describe('fetchFxRatePrompt modal', () => {
    it('fetches FX rates and refreshes on confirm', async () => {
      modalStateValue = { type: 'fetchFxRatePrompt', date: '2026-01-15' };
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('confirm-ok'));

      await waitFor(() => {
        expect(mockFetchFxRates).toHaveBeenCalledWith('2026-01-15');
        expect(mockRefresh).toHaveBeenCalled();
        expect(mockCloseModal).toHaveBeenCalled();
      });
    });

    it('closes modal even when fetch fails', async () => {
      mockFetchFxRates.mockRejectedValue(new Error('fetch fail'));
      modalStateValue = { type: 'fetchFxRatePrompt', date: '2026-01-15' };
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('confirm-ok'));

      await waitFor(() => {
        expect(mockCloseModal).toHaveBeenCalled();
      });
    });
  });

  // ── ConfirmSwitchDb modal ──

  describe('confirmSwitchDb modal', () => {
    it('calls changeDbLocation with switch action on confirm', async () => {
      modalStateValue = { type: 'confirmSwitchDb', folder: '/existing/db' };
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('confirm-ok'));

      await waitFor(() => {
        expect(mockChangeDbLocation).toHaveBeenCalledWith('/existing/db', 'switch');
        expect(mockHandleConsolidationCurrencyChange).toHaveBeenCalled();
        expect(mockToastSuccess).toHaveBeenCalled();
        expect(mockCloseModal).toHaveBeenCalled();
      });
    });

    it('shows error toast when changeDbLocation fails', async () => {
      mockChangeDbLocation.mockRejectedValue(new Error('switch fail'));
      modalStateValue = { type: 'confirmSwitchDb', folder: '/existing/db' };
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('confirm-ok'));

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalled();
      });
    });
  });

  // ── DbLocationChoice modal ──

  describe('dbLocationChoice modal', () => {
    it('calls changeDbLocation with move action', async () => {
      modalStateValue = { type: 'dbLocationChoice', folder: '/new/path', isReset: false };
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('db-choice-move'));

      await waitFor(() => {
        expect(mockChangeDbLocation).toHaveBeenCalledWith('/new/path', 'move');
        expect(mockToastSuccess).toHaveBeenCalled();
        expect(mockCloseModal).toHaveBeenCalled();
      });
    });

    it('calls changeDbLocation with fresh action', async () => {
      modalStateValue = { type: 'dbLocationChoice', folder: '/new/path', isReset: false };
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('db-choice-fresh'));

      await waitFor(() => {
        expect(mockChangeDbLocation).toHaveBeenCalledWith('/new/path', 'fresh');
        expect(mockToastSuccess).toHaveBeenCalled();
      });
    });

    it('calls resetDbLocation when isReset is true', async () => {
      modalStateValue = { type: 'dbLocationChoice', folder: '', isReset: true };
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('db-choice-move'));

      await waitFor(() => {
        expect(mockResetDbLocation).toHaveBeenCalledWith('move');
        expect(mockToastSuccess).toHaveBeenCalled();
      });
    });

    it('shows error toast when dbLocationChoice action fails (non-reset)', async () => {
      mockChangeDbLocation.mockRejectedValue(new Error('change fail'));
      modalStateValue = { type: 'dbLocationChoice', folder: '/new/path', isReset: false };
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('db-choice-move'));

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalled();
      });
    });

    it('shows error toast when dbLocationChoice reset action fails', async () => {
      mockResetDbLocation.mockRejectedValue(new Error('reset fail'));
      modalStateValue = { type: 'dbLocationChoice', folder: '', isReset: true };
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('db-choice-move'));

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalled();
      });
    });
  });

  // ── ConfirmResetDbLocation modal ──

  describe('confirmResetDbLocation modal', () => {
    it('calls resetDbLocation with switch on confirm', async () => {
      modalStateValue = { type: 'confirmResetDbLocation' };
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('confirm-ok'));

      await waitFor(() => {
        expect(mockResetDbLocation).toHaveBeenCalledWith('switch');
        expect(mockHandleConsolidationCurrencyChange).toHaveBeenCalled();
        expect(mockToastSuccess).toHaveBeenCalled();
        expect(mockCloseModal).toHaveBeenCalled();
      });
    });

    it('shows error toast when confirmResetDbLocation fails', async () => {
      mockResetDbLocation.mockRejectedValue(new Error('reset fail'));
      modalStateValue = { type: 'confirmResetDbLocation' };
      const user = userEvent.setup();
      render(<App />);
      await user.click(screen.getByTestId('confirm-ok'));

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalled();
      });
    });
  });
});
