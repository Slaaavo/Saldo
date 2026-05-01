import { test, expect, type Page } from '@playwright/test';

/**
 * Playwright E2E tests for the self-IBAN suppression feature.
 *
 * When a CSV row's partner/IBAN column contains the importing account's own
 * IBAN, the wizard should suppress the "ownAccount" match so that the row is
 * treated as a plain expense/income (no "Transfer" badge) rather than a
 * self-referential transfer that the backend would reject.
 *
 * Test 2 is the regression guard: when the partner IBAN belongs to a
 * *different* own account, the "Transfer" badge must still appear.
 *
 * The app is assumed to be running at APP_URL (pnpm tauri dev).  Tests
 * accumulate data across runs; each test uses a unique "SIB-Tn" prefix to
 * avoid collisions.
 */
const APP_URL = 'http://localhost:1420';

// IBANs chosen to be realistic Slovak IBANs that will not collide across runs.
const IBAN_T1 = 'SK12 3456 7890 1234 5678';
const IBAN_T2B = 'SK98 7654 3210 9876 5432';

/**
 * Returns a single-row CSV whose "IBAN" header auto-detects to the partner
 * field in the Mapping step.
 */
const makePartnerCsv = (partnerIban: string): string =>
  `Date,Amount,IBAN\n2026-01-15,-5.00,${partnerIban}`;

// ─── Helpers ─────────────────────────────────────────────────────────────────

const navigateToApp = async (page: Page): Promise<void> => {
  await page.goto(APP_URL);
  await expect(page.getByText('Saldo')).toBeVisible();
};

/**
 * Creates a regular account with an optional IBAN via the "Add Account" dialog
 * on the Dashboard.
 */
const createAccount = async (page: Page, name: string, iban?: string): Promise<void> => {
  await page.getByRole('button', { name: 'Add Account', exact: true }).click();
  await page.locator('#create-account-name').fill(name);
  if (iban) {
    await page.locator('#create-account-iban').fill(iban);
  }
  await page.getByRole('button', { name: 'Create' }).click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
};

/**
 * Clicks the "Ledger" nav item in the sidebar and waits for the page heading.
 */
const navigateToLedger = async (page: Page): Promise<void> => {
  await page.getByRole('button', { name: 'Ledger' }).click();
  await expect(page.getByRole('heading', { name: 'Ledger', level: 2 })).toBeVisible();
};

/**
 * Opens the Import CSV wizard from the Ledger page toolbar, uploads the given
 * CSV content as a virtual file, selects the named account, and advances
 * through the Upload and Mapping steps to reach the Review step.
 *
 * The Mapping step relies on auto-detection: "Date" → date, "Amount" → amount,
 * "IBAN" → partner — so no manual mapping is required.
 */
const openImportAndReachReview = async (
  page: Page,
  accountName: string,
  csvContent: string,
): Promise<void> => {
  await page.getByRole('button', { name: 'Import CSV' }).click();
  await expect(page.getByRole('dialog')).toBeVisible();

  // Playwright can write to hidden <input type="file"> elements directly.
  await page.locator('input[type="file"]').setInputFiles({
    name: 'test.csv',
    mimeType: 'text/csv',
    buffer: new TextEncoder().encode(csvContent),
  });

  // Select account to import into (Radix Select combobox).
  await page.getByRole('combobox').filter({ hasText: 'Select account to import into' }).click();
  await page.getByRole('option', { name: accountName }).click();

  // Advance through Upload → Mapping (auto-detected) → Review.
  await page.getByRole('button', { name: 'Next' }).click();
  await page.getByRole('button', { name: 'Next' }).click();
};

// ─── Tests ───────────────────────────────────────────────────────────────────

test.describe('CSV Import — Self-IBAN suppression', () => {
  test.beforeEach(async ({ page }) => {
    await navigateToApp(page);
  });

  // ── Test 1: Fee row — own IBAN must NOT show the transfer badge ───────────

  test('fee row — self-IBAN has no transfer badge and import succeeds', async ({ page }) => {
    // Create the importing account and assign it a known IBAN.
    await createAccount(page, 'SIB-T1 Checking', IBAN_T1);
    await navigateToLedger(page);

    // Upload a CSV row whose partner IBAN equals the importing account's own IBAN.
    await openImportAndReachReview(page, 'SIB-T1 Checking', makePartnerCsv(IBAN_T1));

    // The "Transfer" badge must NOT be visible — the self-IBAN match is suppressed.
    await expect(page.getByRole('dialog').getByText('Transfer', { exact: true })).not.toBeVisible();

    // The Import button must be enabled (row counts as a normal importable row).
    const importButton = page.getByRole('button', { name: /Import \d+ rows/ });
    await expect(importButton).toBeEnabled();

    // Clicking Import must close the dialog (import succeeds).
    await importButton.click();
    await expect(page.getByRole('dialog')).not.toBeVisible();
  });

  // ── Test 2: Different account row — transfer badge must be preserved ───────

  test('different-account row — transfer badge is preserved', async ({ page }) => {
    // SIB-T2A is the account being imported into (no IBAN needed for it).
    await createAccount(page, 'SIB-T2A Checking');
    // SIB-T2B is a different own account whose IBAN will appear in the CSV.
    await createAccount(page, 'SIB-T2B Checking', IBAN_T2B);
    await navigateToLedger(page);

    // Upload a CSV row whose partner IBAN belongs to SIB-T2B, not SIB-T2A.
    // The guard must NOT suppress this match — it is a legitimate transfer.
    await openImportAndReachReview(page, 'SIB-T2A Checking', makePartnerCsv(IBAN_T2B));

    // The "Transfer" badge must be visible for this inter-account row.
    await expect(page.getByRole('dialog').getByText('Transfer', { exact: true })).toBeVisible();
  });
});
