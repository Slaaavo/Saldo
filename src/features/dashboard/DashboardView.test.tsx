import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import DashboardView from './DashboardView';
import type { SnapshotRow, EventWithData, Currency, ModalState } from '../../shared/types';

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

// ── Mock child components to isolate DashboardView logic ────────────────────
vi.mock('../../shared/ui/NumberValue', () => ({
  default: ({ value }: { value: number }) => <span data-testid="number-value">{value}</span>,
}));

vi.mock('./AccountCards', () => ({
  default: (props: {
    sectionTitle?: string;
    addButtonLabel?: string;
    emptyMessage?: string;
    onCreateAccount: () => void;
    onReorder?: () => void;
    snapshot: SnapshotRow[];
  }) => (
    <div data-testid={`account-cards-${props.sectionTitle ?? 'unknown'}`}>
      <span data-testid="section-title">{props.sectionTitle}</span>
      {props.snapshot.length === 0 && props.emptyMessage && (
        <span data-testid="empty-message">{props.emptyMessage}</span>
      )}
      <button data-testid={`add-btn-${props.sectionTitle}`} onClick={props.onCreateAccount}>
        {props.addButtonLabel}
      </button>
      {props.onReorder && (
        <button data-testid={`reorder-btn-${props.sectionTitle}`} onClick={props.onReorder}>
          reorder
        </button>
      )}
    </div>
  ),
}));

vi.mock('./Ledger', () => ({
  default: (props: { onUpdateBalances: () => void; onViewAll?: () => void }) => (
    <div data-testid="ledger">
      <button data-testid="update-balances-btn" onClick={props.onUpdateBalances}>
        Update
      </button>
      {props.onViewAll && (
        <button data-testid="view-all-btn" onClick={props.onViewAll}>
          View all
        </button>
      )}
    </div>
  ),
}));

// ── Helpers ─────────────────────────────────────────────────────────────────
const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false };

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

interface DefaultPropsOptions {
  accounts?: SnapshotRow[];
  buckets?: SnapshotRow[];
  assets?: SnapshotRow[];
  events?: EventWithData[];
  totalMinor?: number;
  leftToSpendMinor?: number;
  netWorthMinor?: number;
  hasAssets?: boolean;
  missingFxCurrencies?: string[];
  isDemoMode?: boolean;
}

function makeProps(overrides?: DefaultPropsOptions) {
  const accounts = overrides?.accounts ?? [makeSnapshot()];
  const buckets = overrides?.buckets ?? [];
  const assets = overrides?.assets ?? [];
  const events = overrides?.events ?? [makeEvent()];
  const snapshot = [...accounts, ...buckets, ...assets];

  return {
    snapshot,
    accounts,
    buckets,
    assets,
    events,
    totalEvents: 0,
    consolidationCurrency: EUR,
    totalMinor: overrides?.totalMinor ?? 100000,
    leftToSpendMinor: overrides?.leftToSpendMinor ?? 50000,
    netWorthMinor: overrides?.netWorthMinor ?? 300000,
    hasAssets: overrides?.hasAssets ?? false,
    missingFxCurrencies: overrides?.missingFxCurrencies ?? [],
    setModalState: vi.fn() as ReturnType<typeof vi.fn> & ((state: ModalState) => void),
    isDemoMode: overrides?.isDemoMode ?? false,
    onEnterDemoMode: vi.fn(),
    onNavigate: vi.fn(),
  };
}

// ── Tests ───────────────────────────────────────────────────────────────────
describe('DashboardView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ── Metric rendering ──

  describe('metrics', () => {
    it('shows totalBalance metric when there are no assets', () => {
      const props = makeProps({ hasAssets: false });
      render(<DashboardView {...props} />);
      expect(screen.getByText('metrics.totalBalance')).toBeInTheDocument();
      expect(screen.queryByText('metrics.netWorth')).not.toBeInTheDocument();
      expect(screen.queryByText('metrics.liquid')).not.toBeInTheDocument();
    });

    it('shows netWorth and liquid metrics when there are assets', () => {
      const props = makeProps({ hasAssets: true, assets: [makeAsset()] });
      render(<DashboardView {...props} />);
      expect(screen.getByText('metrics.netWorth')).toBeInTheDocument();
      expect(screen.getByText('metrics.liquid')).toBeInTheDocument();
      expect(screen.queryByText('metrics.totalBalance')).not.toBeInTheDocument();
    });

    it('shows leftToSpend metric when buckets exist', () => {
      const props = makeProps({ buckets: [makeBucket()] });
      render(<DashboardView {...props} />);
      expect(screen.getByText('metrics.leftToSpend')).toBeInTheDocument();
    });

    it('hides leftToSpend metric when no buckets exist', () => {
      const props = makeProps({ buckets: [] });
      render(<DashboardView {...props} />);
      expect(screen.queryByText('metrics.leftToSpend')).not.toBeInTheDocument();
    });

    it('shows all three metrics when assets and buckets exist', () => {
      const props = makeProps({
        hasAssets: true,
        assets: [makeAsset()],
        buckets: [makeBucket()],
      });
      render(<DashboardView {...props} />);
      expect(screen.getByText('metrics.netWorth')).toBeInTheDocument();
      expect(screen.getByText('metrics.liquid')).toBeInTheDocument();
      expect(screen.getByText('metrics.leftToSpend')).toBeInTheDocument();
    });

    it('renders metric values via NumberValue', () => {
      const props = makeProps({ totalMinor: 123456 });
      render(<DashboardView {...props} />);
      const values = screen.getAllByTestId('number-value');
      expect(values.some((el) => el.textContent === '123456')).toBe(true);
    });
  });

  // ── FX rate missing warning ──

  describe('FX rate missing warning', () => {
    it('shows warning when missingFxCurrencies is non-empty', () => {
      const props = makeProps({ missingFxCurrencies: ['USD', 'GBP'] });
      render(<DashboardView {...props} />);
      expect(screen.getByText('metrics.fxRateMissing', { exact: false })).toBeInTheDocument();
    });

    it('hides warning when missingFxCurrencies is empty', () => {
      const props = makeProps({ missingFxCurrencies: [] });
      render(<DashboardView {...props} />);
      expect(screen.queryByText('metrics.fxRateMissing')).not.toBeInTheDocument();
    });
  });

  // ── Empty state / demo mode ──

  describe('empty state', () => {
    it('shows empty state when no accounts and not in demo mode', () => {
      const props = makeProps({ accounts: [], isDemoMode: false });
      render(<DashboardView {...props} />);
      expect(screen.getByText('demo.emptyTitle')).toBeInTheDocument();
      expect(screen.getByText('demo.emptyDesc')).toBeInTheDocument();
      expect(screen.getByText('demo.emptyCta')).toBeInTheDocument();
      expect(screen.getByText('accounts.addAccount')).toBeInTheDocument();
    });

    it('calls onEnterDemoMode when demo CTA is clicked', async () => {
      const user = userEvent.setup();
      const props = makeProps({ accounts: [], isDemoMode: false });
      render(<DashboardView {...props} />);
      await user.click(screen.getByText('demo.emptyCta'));
      expect(props.onEnterDemoMode).toHaveBeenCalledOnce();
    });

    it('opens createAccount modal when add account button is clicked in empty state', async () => {
      const user = userEvent.setup();
      const props = makeProps({ accounts: [], isDemoMode: false });
      render(<DashboardView {...props} />);
      await user.click(screen.getByText('accounts.addAccount'));
      expect(props.setModalState).toHaveBeenCalledWith({
        type: 'createAccount',
        accountType: 'account',
      });
    });

    it('does not show empty state when in demo mode with no accounts', () => {
      const props = makeProps({ accounts: [], isDemoMode: true });
      render(<DashboardView {...props} />);
      expect(screen.queryByText('demo.emptyTitle')).not.toBeInTheDocument();
    });
  });

  // ── Account cards section ──

  describe('account cards section', () => {
    it('renders AccountCards for accounts when accounts exist', () => {
      const props = makeProps();
      render(<DashboardView {...props} />);
      expect(screen.getByTestId('account-cards-accounts.sectionTitle')).toBeInTheDocument();
    });

    it('renders AccountCards for buckets when accounts exist', () => {
      const props = makeProps({ buckets: [makeBucket()] });
      render(<DashboardView {...props} />);
      expect(screen.getByTestId('account-cards-buckets.sectionTitle')).toBeInTheDocument();
    });

    it('renders AccountCards for assets when accounts exist', () => {
      const props = makeProps({ assets: [makeAsset()] });
      render(<DashboardView {...props} />);
      expect(screen.getByTestId('account-cards-assets.sectionTitle')).toBeInTheDocument();
    });

    it('does not render buckets, assets, or ledger sections when no accounts exist', () => {
      const props = makeProps({ accounts: [], isDemoMode: true });
      render(<DashboardView {...props} />);
      expect(screen.queryByTestId('account-cards-buckets.sectionTitle')).not.toBeInTheDocument();
      expect(screen.queryByTestId('account-cards-assets.sectionTitle')).not.toBeInTheDocument();
      expect(screen.queryByTestId('ledger')).not.toBeInTheDocument();
    });
  });

  // ── Create account/bucket/asset actions ──

  describe('section add/create buttons', () => {
    it('opens createAccount modal for accounts section', async () => {
      const user = userEvent.setup();
      const props = makeProps();
      render(<DashboardView {...props} />);
      await user.click(screen.getByTestId('add-btn-accounts.sectionTitle'));
      expect(props.setModalState).toHaveBeenCalledWith({
        type: 'createAccount',
        accountType: 'account',
      });
    });

    it('opens createAccount modal for buckets section', async () => {
      const user = userEvent.setup();
      const props = makeProps({ buckets: [makeBucket()] });
      render(<DashboardView {...props} />);
      await user.click(screen.getByTestId('add-btn-buckets.sectionTitle'));
      expect(props.setModalState).toHaveBeenCalledWith({
        type: 'createAccount',
        accountType: 'bucket',
      });
    });

    it('opens createAsset modal for assets section', async () => {
      const user = userEvent.setup();
      const props = makeProps({ assets: [makeAsset()] });
      render(<DashboardView {...props} />);
      await user.click(screen.getByTestId('add-btn-assets.sectionTitle'));
      expect(props.setModalState).toHaveBeenCalledWith({ type: 'createAsset' });
    });
  });

  // ── Reorder actions ──

  describe('reorder buttons', () => {
    it('opens reorderAccounts modal', async () => {
      const user = userEvent.setup();
      const props = makeProps();
      render(<DashboardView {...props} />);
      await user.click(screen.getByTestId('reorder-btn-accounts.sectionTitle'));
      expect(props.setModalState).toHaveBeenCalledWith({ type: 'reorderAccounts' });
    });

    it('opens reorderBuckets modal', async () => {
      const user = userEvent.setup();
      const props = makeProps({ buckets: [makeBucket()] });
      render(<DashboardView {...props} />);
      await user.click(screen.getByTestId('reorder-btn-buckets.sectionTitle'));
      expect(props.setModalState).toHaveBeenCalledWith({ type: 'reorderBuckets' });
    });

    it('opens reorderAssets modal', async () => {
      const user = userEvent.setup();
      const props = makeProps({ assets: [makeAsset()] });
      render(<DashboardView {...props} />);
      await user.click(screen.getByTestId('reorder-btn-assets.sectionTitle'));
      expect(props.setModalState).toHaveBeenCalledWith({ type: 'reorderAssets' });
    });
  });

  // ── Ledger section ──

  describe('ledger', () => {
    it('renders Ledger when accounts exist', () => {
      const props = makeProps();
      render(<DashboardView {...props} />);
      expect(screen.getByTestId('ledger')).toBeInTheDocument();
    });

    it('opens bulkUpdateBalance modal from Ledger', async () => {
      const user = userEvent.setup();
      const props = makeProps();
      render(<DashboardView {...props} />);
      await user.click(screen.getByTestId('update-balances-btn'));
      expect(props.setModalState).toHaveBeenCalledWith({ type: 'bulkUpdateBalance' });
    });

    it('calls onNavigate with "ledger" when View all is clicked', async () => {
      const user = userEvent.setup();
      const props = makeProps();
      render(<DashboardView {...props} />);
      await user.click(screen.getByTestId('view-all-btn'));
      expect(props.onNavigate).toHaveBeenCalledWith('ledger');
    });
  });

  // ── MetricCard styling ──

  describe('MetricCard styling', () => {
    it('applies destructive class for negative values', () => {
      const props = makeProps({ totalMinor: -5000, hasAssets: false });
      render(<DashboardView {...props} />);
      const metricEl = screen.getByText('metrics.totalBalance').closest('div');
      const valueEl = metricEl?.querySelector('.text-destructive');
      expect(valueEl).toBeInTheDocument();
    });

    it('does not apply destructive class for positive values', () => {
      const props = makeProps({ totalMinor: 5000, hasAssets: false });
      render(<DashboardView {...props} />);
      const metricEl = screen.getByText('metrics.totalBalance').closest('div');
      const valueEl = metricEl?.querySelector('.text-destructive');
      expect(valueEl).not.toBeInTheDocument();
    });
  });
});
