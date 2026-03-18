import { test, expect, type Page } from '@playwright/test';

/**
 * Playwright E2E tests for the Split Transaction Groups feature.
 *
 * Tests verify that a CSV row can be split into multiple legs in the Review
 * step, that the Import button is blocked while legs are unbalanced, and that
 * the Ledger renders the imported group as a collapsible card.
 *
 * The app is assumed to be running at APP_URL (pnpm tauri dev).  Tests
 * accumulate data across runs; each test uses a unique "ST-Tn" prefix to
 * avoid collisions.
 */
const APP_URL = 'http://localhost:1420';

// Single-row CSV with headers that auto-detect to the date and amount columns.
const TEST_CSV = `Date,Amount,Note\n2026-01-15,-50.00,ST split test`;

// ─── Helpers ─────────────────────────────────────────────────────────────────

async function navigateToApp(page: Page): Promise<void> {
  await page.goto(APP_URL);
  await expect(page.getByText('Saldo')).toBeVisible();
}

/**
 * Creates a regular account via the "Add Account" button on the Dashboard.
 */
async function createAccount(page: Page, name: string): Promise<void> {
  await page.getByRole('button', { name: 'Add Account', exact: true }).click();
  await page.locator('#create-account-name').fill(name);
  await page.getByRole('button', { name: 'Create' }).click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
}

/**
 * Clicks the "Ledger" nav item in the sidebar and waits for the page heading.
 */
async function navigateToLedger(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'Ledger' }).click();
  await expect(page.getByRole('heading', { name: 'Ledger', level: 2 })).toBeVisible();
}

/**
 * Opens the Import CSV wizard from the Ledger page toolbar, uploads the given
 * CSV content as a virtual file, selects the named account, and advances
 * through the Upload and Mapping steps to reach the Review step.
 *
 * The Mapping step relies on auto-detection: the "Date" and "Amount" headers
 * in TEST_CSV are matched automatically, so no manual mapping is required.
 */
async function openImportAndReachReview(
  page: Page,
  accountName: string,
  csvContent: string,
): Promise<void> {
  await page.getByRole('button', { name: 'Import CSV' }).click();
  await expect(page.getByRole('dialog')).toBeVisible();

  // Playwright can write to hidden <input type="file"> elements directly.
  await page.locator('input[type="file"]').setInputFiles({
    name: 'test.csv',
    mimeType: 'text/csv',
    buffer: Buffer.from(csvContent),
  });

  // Select account to import into (Radix Select combobox).
  await page.getByRole('combobox').filter({ hasText: 'Select account to import into' }).click();
  await page.getByRole('option', { name: accountName }).click();

  // Advance through Upload → Mapping (auto-detected) → Review.
  await page.getByRole('button', { name: 'Next' }).click();
  await page.getByRole('button', { name: 'Next' }).click();
}

// ─── Tests ───────────────────────────────────────────────────────────────────

test.describe('Split Transaction Groups', () => {
  test.beforeEach(async ({ page }) => {
    await navigateToApp(page);
  });

  // ── Test 1: Happy path — import a split group with 2 legs ─────────────────

  test('imports a split group with 2 legs and succeeds', async ({ page }) => {
    await createAccount(page, 'ST-T1 Checking');
    await navigateToLedger(page);
    await openImportAndReachReview(page, 'ST-T1 Checking', TEST_CSV);

    // Click the "Split" button in the Actions column for the CSV row.
    await page.getByRole('button', { name: 'Split' }).click();

    // The split editor panel opens and shows two leg rows.
    await expect(page.getByText('Split 1', { exact: true })).toBeVisible();
    await expect(page.getByText('Split 2', { exact: true })).toBeVisible();

    // Enter partial amounts so that both legs contribute non-zero values that
    // together sum to the original -50.00 total.
    // Leg 0 starts at -50.00; change it to -30 so the total is unbalanced.
    const leg1Row = page
      .getByText('Split 1', { exact: true })
      .locator('xpath=ancestor::tr[1]');
    await leg1Row.locator('input[type="number"]').fill('-30');

    // Change Leg 1 from 0 to -20 — sum is now -50.00 again, so balanced.
    const leg2Row = page
      .getByText('Split 2', { exact: true })
      .locator('xpath=ancestor::tr[1]');
    await leg2Row.locator('input[type="number"]').fill('-20');

    // The Remaining indicator should turn green (diff === 0).
    const remainingAmount = page
      .getByText('Remaining', { exact: true })
      .locator('xpath=ancestor::tr[1]/td[3]/div');
    await expect(remainingAmount).toHaveClass(/text-green/);

    // Import button becomes enabled once balanced — click it.
    await page.getByRole('button', { name: /Import \d+ rows/ }).click();

    // The dialog closes on success.
    await expect(page.getByRole('dialog')).not.toBeVisible();
  });

  // ── Test 2: Import button disabled while remaining ≠ 0 ────────────────────

  test('disables Import and shows validation banner when legs do not balance', async ({
    page,
  }) => {
    await createAccount(page, 'ST-T2 Checking');
    await navigateToLedger(page);
    await openImportAndReachReview(page, 'ST-T2 Checking', TEST_CSV);

    await page.getByRole('button', { name: 'Split' }).click();

    // Change Leg 0 to a partial amount — Leg 1 stays at 0, so sum ≠ total.
    const leg1Row = page
      .getByText('Split 1', { exact: true })
      .locator('xpath=ancestor::tr[1]');
    await leg1Row.locator('input[type="number"]').fill('-30');

    // Import button must be disabled while the split is unbalanced.
    await expect(page.getByRole('button', { name: /Import \d+ rows/ })).toBeDisabled();

    // The validation banner explaining the problem must be visible.
    await expect(page.getByText('Some split groups are incomplete')).toBeVisible();
  });

  // ── Test 3: Ledger shows collapsed split group after import ────────────────

  test('ledger shows collapsed split group with badge after import', async ({ page }) => {
    await createAccount(page, 'ST-T3 Checking');
    await navigateToLedger(page);
    await openImportAndReachReview(page, 'ST-T3 Checking', TEST_CSV);

    // Open split editor — the default state has Leg 0 = full amount and Leg 1
    // = 0, which already sums to the total (balanced), so we can import
    // straight away without adjusting any amounts.
    await page.getByRole('button', { name: 'Split' }).click();
    await page.getByRole('button', { name: /Import \d+ rows/ }).click();
    await expect(page.getByRole('dialog')).not.toBeVisible();

    // After close we are still on the Ledger page.  The list refreshes and
    // the split group should appear as a single collapsed card.
    const splitGroupCard = page
      .locator('div.group')
      .filter({ hasText: 'ST-T3 Checking' })
      .filter({ hasText: 'Split ×2' })
      .first();

    await expect(splitGroupCard.getByText('Split ×2')).toBeVisible();

    // The expand chevron is hidden until hover; use force:true to click it.
    await splitGroupCard.hover();
    await splitGroupCard.locator('div.cursor-pointer').click({ force: true });

    // After expanding, the individual leg labels become visible.
    await expect(page.getByText('Split 1', { exact: true }).first()).toBeVisible();
    await expect(page.getByText('Split 2', { exact: true }).first()).toBeVisible();
  });
});
