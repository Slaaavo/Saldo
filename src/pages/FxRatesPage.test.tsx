import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import FxRatesPage from './FxRatesPage';
import type { FxRateRow, Currency } from '../types';

// ── Mock the API layer ──────────────────────────────────────────────────────
vi.mock('../api', () => ({
  listFxRates: vi.fn(),
  fetchFxRates: vi.fn(),
  listCurrencies: vi.fn(),
  getConsolidationCurrency: vi.fn(),
  setFxRateManual: vi.fn(),
  getMissingRateDates: vi.fn(),
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

// ── Mock DatePicker to avoid Popover/Calendar complexity in tests ────────────
vi.mock('../components/ui/date-picker', () => ({
  DatePicker: ({ value, onChange }: { value: string; onChange: (v: string) => void }) => (
    <input data-testid="date-picker" value={value} onChange={(e) => onChange(e.target.value)} />
  ),
}));

import {
  listFxRates,
  fetchFxRates,
  listCurrencies,
  getConsolidationCurrency,
  setFxRateManual,
  getMissingRateDates,
} from '../api';

// ── Helpers ─────────────────────────────────────────────────────────────────
const makeCurrency = (id: number, code: string): Currency => ({
  id,
  code,
  name: code,
  minorUnits: 2,
  isCustom: false,
});

const makeRate = (
  id: number,
  date: string,
  toCurrencyCode: string,
  mantissa: number,
  exponent: number,
  isManual = false,
): FxRateRow => ({
  id,
  date,
  fromCurrencyCode: 'EUR',
  toCurrencyCode,
  rateMantissa: mantissa,
  rateExponent: exponent,
  isManual,
  fetchedAt: `${date}T12:00:00`,
});

const EUR = makeCurrency(1, 'EUR');
const USD = makeCurrency(2, 'USD');
const GBP = makeCurrency(3, 'GBP');

const sampleRates: FxRateRow[] = [
  makeRate(1, '2026-01-02', 'USD', 10900, -4),
  makeRate(2, '2026-01-02', 'GBP', 8500, -4),
  makeRate(3, '2026-01-01', 'USD', 10842, -4),
  makeRate(4, '2026-01-01', 'GBP', 8456, -4),
];

function setupMocks(overrides?: {
  rates?: FxRateRow[];
  currencies?: Currency[];
  consolidationCode?: string;
  missingDates?: string[];
}) {
  const {
    rates = sampleRates,
    currencies = [EUR, USD, GBP],
    consolidationCode = 'EUR',
    missingDates = [],
  } = overrides ?? {};

  (listFxRates as Mock).mockResolvedValue(rates);
  (listCurrencies as Mock).mockResolvedValue(currencies);
  (getConsolidationCurrency as Mock).mockResolvedValue({ code: consolidationCode });
  (getMissingRateDates as Mock).mockResolvedValue(missingDates);
  (fetchFxRates as Mock).mockResolvedValue(undefined);
  (setFxRateManual as Mock).mockResolvedValue(undefined);
}

// ── Tests ───────────────────────────────────────────────────────────────────
describe('FxRatesPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ── Initial rendering ──

  it('calls all API endpoints on mount', async () => {
    setupMocks();
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(listFxRates).toHaveBeenCalledOnce();
      expect(listCurrencies).toHaveBeenCalledWith(false);
      expect(getConsolidationCurrency).toHaveBeenCalledOnce();
      expect(getMissingRateDates).toHaveBeenCalledOnce();
    });
  });

  it('renders the title', async () => {
    setupMocks();
    render(<FxRatesPage />);

    expect(screen.getByText('fxRates.title')).toBeInTheDocument();
  });

  it('shows subtitle with consolidation currency', async () => {
    setupMocks();
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(screen.getByText('fxRates.subtitle')).toBeInTheDocument();
    });
  });

  it('renders empty state when no rates exist', async () => {
    setupMocks({ rates: [] });
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(screen.getByText('fxRates.noRates')).toBeInTheDocument();
    });
  });

  // ── Table rendering ──

  it('renders rate table with dates and currencies', async () => {
    setupMocks();
    render(<FxRatesPage />);

    await waitFor(() => {
      // Column headers
      expect(screen.getByText('USD')).toBeInTheDocument();
      expect(screen.getByText('GBP')).toBeInTheDocument();
      // Date rows
      expect(screen.getByText('2026-01-02')).toBeInTheDocument();
      expect(screen.getByText('2026-01-01')).toBeInTheDocument();
    });
  });

  it('renders rate values in the table', async () => {
    setupMocks();
    render(<FxRatesPage />);

    await waitFor(() => {
      // 10900e-4 = 1.09, 8500e-4 = 0.85
      expect(screen.getByText('1.09')).toBeInTheDocument();
      expect(screen.getByText('0.85')).toBeInTheDocument();
    });
  });

  it('shows manual badge for manual rates', async () => {
    const manualRate = makeRate(10, '2026-01-01', 'USD', 10842, -4, true);
    setupMocks({ rates: [manualRate] });
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(screen.getByText('(fxRates.isManual)')).toBeInTheDocument();
    });
  });

  it('shows dash for missing rate in a cell', async () => {
    // USD and GBP columns exist, but only USD has a rate for this date
    const rates = [
      makeRate(1, '2026-01-01', 'USD', 10842, -4),
      makeRate(2, '2026-01-02', 'GBP', 8500, -4),
    ];
    setupMocks({ rates });
    render(<FxRatesPage />);

    await waitFor(() => {
      // The GBP cell on 2026-01-01 and USD cell on 2026-01-02 should show dashes
      const dashes = screen.getAllByText('—');
      expect(dashes.length).toBeGreaterThanOrEqual(1);
    });
  });

  // ── Error handling ──

  it('shows error when listFxRates fails', async () => {
    setupMocks();
    (listFxRates as Mock).mockRejectedValue(new Error('Network error'));
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(screen.getByText('Network error')).toBeInTheDocument();
    });
  });

  // ── Refresh flow ──

  it('calls fetchFxRates on refresh button click', async () => {
    setupMocks();
    const user = userEvent.setup();
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(screen.getByText('fxRates.refreshRates')).toBeInTheDocument();
    });

    await user.click(screen.getByText('fxRates.refreshRates'));

    await waitFor(() => {
      expect(fetchFxRates).toHaveBeenCalledWith(expect.any(String), true);
      // Should reload rates after refresh
      expect(listFxRates).toHaveBeenCalledTimes(2);
    });
  });

  it('shows error when refresh fails', async () => {
    setupMocks();
    (fetchFxRates as Mock).mockRejectedValue(new Error('API limit'));
    const user = userEvent.setup();
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(screen.getByText('fxRates.refreshRates')).toBeInTheDocument();
    });

    await user.click(screen.getByText('fxRates.refreshRates'));

    await waitFor(() => {
      expect(screen.getByText('API limit')).toBeInTheDocument();
    });
  });

  // ── Backfill flow ──

  it('shows backfill button when missing dates exist', async () => {
    setupMocks({ missingDates: ['2026-01-03', '2026-01-04'] });
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(screen.getByText('fxRates.backfill')).toBeInTheDocument();
    });
  });

  it('does not show backfill button when no missing dates', async () => {
    setupMocks({ missingDates: [] });
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(screen.getByText('fxRates.refreshRates')).toBeInTheDocument();
    });
    expect(screen.queryByText('fxRates.backfill')).not.toBeInTheDocument();
  });

  it('calls fetchFxRates for each missing date on backfill', async () => {
    const missingDates = ['2026-01-03', '2026-01-04'];
    setupMocks({ missingDates });
    const user = userEvent.setup();
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(screen.getByText('fxRates.backfill')).toBeInTheDocument();
    });

    await user.click(screen.getByText('fxRates.backfill'));

    await waitFor(() => {
      expect(fetchFxRates).toHaveBeenCalledWith('2026-01-03');
      expect(fetchFxRates).toHaveBeenCalledWith('2026-01-04');
      expect(fetchFxRates).toHaveBeenCalledTimes(2);
    });
  });

  // ── Inline editing ──

  it('opens edit input when clicking a rate cell', async () => {
    setupMocks();
    const user = userEvent.setup();
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(screen.getByText('1.09')).toBeInTheDocument();
    });

    await user.click(screen.getByText('1.09'));

    const input = screen.getByDisplayValue('1.09');
    expect(input).toBeInTheDocument();
    expect(input.tagName).toBe('INPUT');
  });

  it('saves on Enter and calls setFxRateManual', async () => {
    setupMocks();
    const user = userEvent.setup();
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(screen.getByText('1.09')).toBeInTheDocument();
    });

    await user.click(screen.getByText('1.09'));
    const input = screen.getByDisplayValue('1.09');

    await user.clear(input);
    await user.type(input, '1.15');
    await user.keyboard('{Enter}');

    await waitFor(() => {
      // setFxRateManual(fromId=1, toId=2, date, mantissa=115, exponent=-2)
      expect(setFxRateManual).toHaveBeenCalledWith(1, 2, '2026-01-02', 115, -2);
    });
  });

  it('cancels edit on Escape', async () => {
    setupMocks();
    const user = userEvent.setup();
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(screen.getByText('1.09')).toBeInTheDocument();
    });

    await user.click(screen.getByText('1.09'));
    expect(screen.getByDisplayValue('1.09')).toBeInTheDocument();

    await user.keyboard('{Escape}');

    // Input should be gone, original value displayed
    expect(screen.queryByDisplayValue('1.09')).not.toBeInTheDocument();
    expect(screen.getByText('1.09')).toBeInTheDocument();
  });

  it('shows error for invalid rate input', async () => {
    setupMocks();
    const user = userEvent.setup();
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(screen.getByText('1.09')).toBeInTheDocument();
    });

    await user.click(screen.getByText('1.09'));
    const input = screen.getByDisplayValue('1.09');

    await user.clear(input);
    await user.type(input, '-5');
    await user.keyboard('{Enter}');

    await waitFor(() => {
      expect(screen.getByText('fxRates.invalidRate')).toBeInTheDocument();
    });
    expect(setFxRateManual).not.toHaveBeenCalled();
  });

  it('does not save when input is empty', async () => {
    setupMocks();
    const user = userEvent.setup();
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(screen.getByText('1.09')).toBeInTheDocument();
    });

    await user.click(screen.getByText('1.09'));
    const input = screen.getByDisplayValue('1.09');

    await user.clear(input);
    await user.keyboard('{Enter}');

    expect(setFxRateManual).not.toHaveBeenCalled();
  });

  it('shows error when setFxRateManual fails', async () => {
    setupMocks();
    (setFxRateManual as Mock).mockRejectedValue(new Error('DB write failed'));
    const user = userEvent.setup();
    render(<FxRatesPage />);

    await waitFor(() => {
      expect(screen.getByText('1.09')).toBeInTheDocument();
    });

    await user.click(screen.getByText('1.09'));
    const input = screen.getByDisplayValue('1.09');

    await user.clear(input);
    await user.type(input, '1.2');
    await user.keyboard('{Enter}');

    await waitFor(() => {
      expect(screen.getByText('DB write failed')).toBeInTheDocument();
    });
  });
});
