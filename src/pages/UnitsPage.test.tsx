import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import UnitsPage from './UnitsPage';
import { formatPrice, parsePriceAsRate } from '../utils/unitPricing';
import type { FxRateRow, Currency } from '../types';

// ── Mock the API layer ──────────────────────────────────────────────────────
vi.mock('../api', () => ({
  listCustomUnits: vi.fn(),
  listFxRates: vi.fn(),
  getConsolidationCurrency: vi.fn(),
  setFxRateManual: vi.fn(),
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

import { listCustomUnits, listFxRates, getConsolidationCurrency, setFxRateManual } from '../api';

// ── Helpers ─────────────────────────────────────────────────────────────────
const makeCurrency = (id: number, code: string, isCustom = false): Currency => ({
  id,
  code,
  name: code,
  minorUnits: 2,
  isCustom,
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
const GOLD = makeCurrency(10, 'GOLD', true);
const BTC = makeCurrency(11, 'BTC', true);

function setupMocks(overrides?: {
  units?: Currency[];
  rates?: FxRateRow[];
  consolidation?: Currency;
}) {
  const units = overrides?.units ?? [GOLD];
  const rates = overrides?.rates ?? [];
  const consolidation = overrides?.consolidation ?? EUR;

  (listCustomUnits as Mock).mockResolvedValue(units);
  (listFxRates as Mock).mockResolvedValue(rates);
  (getConsolidationCurrency as Mock).mockResolvedValue(consolidation);
  (setFxRateManual as Mock).mockResolvedValue(undefined);
}

// ═══════════════════════════════════════════════════════════════════════════
// Pure function tests
// ═══════════════════════════════════════════════════════════════════════════

describe('formatPrice', () => {
  it('returns inverted rate for simple rate (rate=2 → price=0.5)', () => {
    const row = makeRate(1, '2025-01-01', 'GOLD', 2, 0);
    expect(formatPrice(row)).toBe('0.5');
  });

  it('returns inverted rate with exponent (rate=0.5 → price=2)', () => {
    // 0.5 = 5 × 10^-1
    const row = makeRate(1, '2025-01-01', 'GOLD', 5, -1);
    expect(formatPrice(row)).toBe('2');
  });

  it('handles 1:1 rate (rate=1 → price=1)', () => {
    const row = makeRate(1, '2025-01-01', 'GOLD', 1, 0);
    expect(formatPrice(row)).toBe('1');
  });

  it('handles large rate (rate=100000 → price=0.00001)', () => {
    const row = makeRate(1, '2025-01-01', 'GOLD', 1, 5);
    expect(formatPrice(row)).toBe('0.00001');
  });

  it('handles small rate (rate=0.001 → price=1000)', () => {
    // 0.001 = 1 × 10^-3
    const row = makeRate(1, '2025-01-01', 'GOLD', 1, -3);
    expect(formatPrice(row)).toBe('1000');
  });

  it('returns Infinity for zero mantissa (division by zero)', () => {
    const row = makeRate(1, '2025-01-01', 'GOLD', 0, 0);
    expect(formatPrice(row)).toBe('Infinity');
  });
});

describe('parsePriceAsRate', () => {
  it('parses a simple price and inverts it', () => {
    // price=2 → rate=0.5 → mantissa=5, exponent=-1
    const result = parsePriceAsRate('2');
    expect(result).toEqual({ mantissa: 5, exponent: -1 });
  });

  it('parses price=1 → rate=1', () => {
    const result = parsePriceAsRate('1');
    expect(result).toEqual({ mantissa: 1, exponent: 0 });
  });

  it('parses decimal price', () => {
    // price=0.5 → rate=2 → mantissa=2, exponent=0
    const result = parsePriceAsRate('0.5');
    expect(result).toEqual({ mantissa: 2, exponent: 0 });
  });

  it('parses large price', () => {
    // price=1000 → rate=0.001 → mantissa=1, exponent=-3
    const result = parsePriceAsRate('1000');
    expect(result).toEqual({ mantissa: 1, exponent: -3 });
  });

  it('returns null for zero', () => {
    expect(parsePriceAsRate('0')).toBeNull();
  });

  it('returns null for negative value', () => {
    expect(parsePriceAsRate('-5')).toBeNull();
  });

  it('returns null for empty string', () => {
    expect(parsePriceAsRate('')).toBeNull();
  });

  it('returns null for non-numeric input', () => {
    expect(parsePriceAsRate('abc')).toBeNull();
  });

  it('roundtrips with formatPrice', () => {
    // price=42.5 → parse → build FxRateRow → formatPrice should ≈ 42.5
    const parsed = parsePriceAsRate('42.5');
    expect(parsed).not.toBeNull();
    const row = makeRate(1, '2025-01-01', 'X', parsed!.mantissa, parsed!.exponent);
    expect(parseFloat(formatPrice(row))).toBeCloseTo(42.5, 4);
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// Component tests
// ═══════════════════════════════════════════════════════════════════════════

describe('UnitsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the title and subtitle', async () => {
    setupMocks();
    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText('units.title')).toBeInTheDocument();
      expect(screen.getByText('units.subtitle')).toBeInTheDocument();
    });
  });

  it('shows empty state when no custom units exist', async () => {
    setupMocks({ units: [] });
    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText('units.empty')).toBeInTheDocument();
    });
  });

  it('shows "no prices" when units exist but no rates', async () => {
    setupMocks({ units: [GOLD], rates: [] });
    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText('units.noPrices')).toBeInTheDocument();
    });
  });

  it('renders table with rate data', async () => {
    const rates = [
      makeRate(1, '2025-01-15', 'GOLD', 5, -1), // rate=0.5 → price=2
      makeRate(2, '2025-01-14', 'GOLD', 2, 0), // rate=2 → price=0.5
    ];
    setupMocks({ units: [GOLD], rates });
    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText('GOLD')).toBeInTheDocument();
      expect(screen.getByText('2025-01-15')).toBeInTheDocument();
      expect(screen.getByText('2025-01-14')).toBeInTheDocument();
    });

    // Price values
    expect(screen.getByText('2')).toBeInTheDocument();
    expect(screen.getByText('0.5')).toBeInTheDocument();
  });

  it('renders multiple unit columns', async () => {
    const rates = [
      makeRate(1, '2025-01-15', 'GOLD', 5, -1),
      makeRate(2, '2025-01-15', 'BTC', 1, -5),
    ];
    setupMocks({ units: [GOLD, BTC], rates });
    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText('GOLD')).toBeInTheDocument();
      expect(screen.getByText('BTC')).toBeInTheDocument();
    });
  });

  it('shows manual rate indicator', async () => {
    const rates = [makeRate(1, '2025-01-15', 'GOLD', 5, -1, true)];
    setupMocks({ units: [GOLD], rates });
    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText(/fxRates\.isManual/)).toBeInTheDocument();
    });
  });

  it('displays error when API fails', async () => {
    (listCustomUnits as Mock).mockRejectedValue(new Error('Network error'));
    (listFxRates as Mock).mockResolvedValue([]);
    (getConsolidationCurrency as Mock).mockResolvedValue(EUR);

    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText('Network error')).toBeInTheDocument();
    });
  });

  it('sorts dates in descending order', async () => {
    const rates = [
      makeRate(1, '2025-01-10', 'GOLD', 1, 0),
      makeRate(2, '2025-01-20', 'GOLD', 2, 0),
      makeRate(3, '2025-01-15', 'GOLD', 3, 0),
    ];
    setupMocks({ units: [GOLD], rates });
    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText('2025-01-20')).toBeInTheDocument();
    });

    const rows = screen.getAllByRole('row');
    // rows[0] is header, rows[1..3] are data rows
    expect(rows[1]).toHaveTextContent('2025-01-20');
    expect(rows[2]).toHaveTextContent('2025-01-15');
    expect(rows[3]).toHaveTextContent('2025-01-10');
  });

  // ── Inline editing ──────────────────────────────────────────────────────

  it('opens editor when a cell is clicked', async () => {
    const rates = [makeRate(1, '2025-01-15', 'GOLD', 5, -1)];
    setupMocks({ units: [GOLD], rates });
    const user = userEvent.setup();
    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText('2')).toBeInTheDocument();
    });

    await user.click(screen.getByText('2'));

    await waitFor(() => {
      const input = screen.getByRole('textbox');
      expect(input).toBeInTheDocument();
      expect(input).toHaveValue('2');
    });
  });

  it('saves edited value on Enter', async () => {
    const rates = [makeRate(1, '2025-01-15', 'GOLD', 5, -1)];
    setupMocks({ units: [GOLD], rates });
    const user = userEvent.setup();
    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText('2')).toBeInTheDocument();
    });

    await user.click(screen.getByText('2'));

    const input = await screen.findByRole('textbox');
    await user.clear(input);
    await user.type(input, '4');
    await user.keyboard('{Enter}');

    await waitFor(() => {
      expect(setFxRateManual).toHaveBeenCalledWith(
        1, // fromCurrencyId (EUR)
        10, // toCurrencyId (GOLD)
        '2025-01-15',
        expect.any(Number),
        expect.any(Number),
      );
    });
  });

  it('cancels editing on Escape', async () => {
    const rates = [makeRate(1, '2025-01-15', 'GOLD', 5, -1)];
    setupMocks({ units: [GOLD], rates });
    const user = userEvent.setup();
    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText('2')).toBeInTheDocument();
    });

    await user.click(screen.getByText('2'));
    expect(screen.getByRole('textbox')).toBeInTheDocument();

    await user.keyboard('{Escape}');

    await waitFor(() => {
      expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
    });
  });

  it('does not save when input is empty', async () => {
    const rates = [makeRate(1, '2025-01-15', 'GOLD', 5, -1)];
    setupMocks({ units: [GOLD], rates });
    const user = userEvent.setup();
    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText('2')).toBeInTheDocument();
    });

    await user.click(screen.getByText('2'));
    const input = await screen.findByRole('textbox');
    await user.clear(input);
    await user.keyboard('{Enter}');

    expect(setFxRateManual).not.toHaveBeenCalled();
  });

  it('shows error for invalid price input', async () => {
    const rates = [makeRate(1, '2025-01-15', 'GOLD', 5, -1)];
    setupMocks({ units: [GOLD], rates });
    const user = userEvent.setup();
    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText('2')).toBeInTheDocument();
    });

    await user.click(screen.getByText('2'));
    const input = await screen.findByRole('textbox');
    await user.clear(input);
    await user.type(input, '-5');
    await user.keyboard('{Enter}');

    await waitFor(() => {
      expect(screen.getByText('units.invalidPrice')).toBeInTheDocument();
    });
    expect(setFxRateManual).not.toHaveBeenCalled();
  });

  it('shows error when setFxRateManual fails', async () => {
    const rates = [makeRate(1, '2025-01-15', 'GOLD', 5, -1)];
    setupMocks({ units: [GOLD], rates });
    (setFxRateManual as Mock).mockRejectedValue(new Error('Save failed'));
    const user = userEvent.setup();
    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText('2')).toBeInTheDocument();
    });

    await user.click(screen.getByText('2'));
    const input = await screen.findByRole('textbox');
    await user.clear(input);
    await user.type(input, '4');
    await user.keyboard('{Enter}');

    await waitFor(() => {
      expect(screen.getByText('Save failed')).toBeInTheDocument();
    });
  });

  it('shows dash for missing rate in a cell', async () => {
    // Two dates but only one has a rate for GOLD
    const rates = [
      makeRate(1, '2025-01-15', 'GOLD', 5, -1),
      makeRate(2, '2025-01-14', 'BTC', 1, -5),
    ];
    setupMocks({ units: [GOLD, BTC], rates });
    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText('2025-01-15')).toBeInTheDocument();
      expect(screen.getByText('2025-01-14')).toBeInTheDocument();
    });

    // Both rows should have dashes for missing data
    const dashes = screen.getAllByText('—');
    expect(dashes.length).toBeGreaterThanOrEqual(2);
  });

  it('only shows rates for custom units (filters out non-custom unit rates)', async () => {
    // Rate for USD (not a custom unit) should not appear in the table
    const rates = [
      makeRate(1, '2025-01-15', 'GOLD', 5, -1),
      makeRate(2, '2025-01-15', 'USD', 11, -1),
    ];
    setupMocks({ units: [GOLD], rates });
    render(<UnitsPage />);

    await waitFor(() => {
      expect(screen.getByText('GOLD')).toBeInTheDocument();
    });

    // Only one date row should exist since USD rate is filtered out
    const dataRows = screen.getAllByRole('row');
    // header + 1 data row = 2 rows
    expect(dataRows).toHaveLength(2);
  });
});
