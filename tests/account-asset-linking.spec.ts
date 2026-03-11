import { test, expect, type Page } from '@playwright/test';

/**
 * Playwright E2E tests for the Account–Asset Linking feature.
 *
 * Tests verify that accounts can be linked to assets, that linked accounts are
 * excluded from the Liquid / Left to Spend metrics, that the asset equity
 * tooltip renders correctly, and that deletions cascade properly.
 *
 * The app is assumed to be running at APP_URL (pnpm tauri dev).  Tests
 * accumulate data in the SQLite database across runs; each test uses a unique
 * "AL-Tn" prefix to avoid collisions with other test runs.
 */
const APP_URL = 'http://localhost:1420';

// ─── Helpers ─────────────────────────────────────────────────────────────────

async function navigateToApp(page: Page): Promise<void> {
  await page.goto(APP_URL);
  await expect(page.getByText('Saldo')).toBeVisible();
}

/**
 * Creates a regular account via the "Add Account" button.
 * `initialBalance` is a decimal string (negative values are allowed).
 */
async function createAccount(
  page: Page,
  name: string,
  initialBalance?: string,
): Promise<void> {
  await page.getByRole('button', { name: 'Add Account', exact: true }).click();
  await page.locator('#create-account-name').fill(name);
  if (initialBalance !== undefined) {
    await page.locator('#create-account-balance').fill(initialBalance);
  }
  await page.getByRole('button', { name: 'Create' }).click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
}

/**
 * Creates a currency-denominated asset via the "Add Asset" button.
 * Assumes the "Currency" denomination tab is selected by default.
 */
async function createAsset(
  page: Page,
  name: string,
  initialValue?: string,
): Promise<void> {
  await page.getByRole('button', { name: 'Add Asset', exact: true }).click();
  await page.locator('#create-asset-name').fill(name);
  if (initialValue !== undefined) {
    await page.locator('#create-asset-value').fill(initialValue);
  }
  await page.getByRole('button', { name: 'Create' }).click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
}

/**
 * Returns a locator scoped to the account/asset card whose name span carries
 * the given title attribute.
 */
function getCard(page: Page, name: string) {
  return page
    .locator(`[title="${name}"]`)
    .locator('xpath=ancestor::div[contains(@class,"shrink-0")]');
}

/**
 * Clicks the dropdown trigger (first <button> in the card) and then clicks
 * the named menu item.
 */
async function openCardMenuAndClick(
  page: Page,
  cardName: string,
  menuItemLabel: string,
): Promise<void> {
  const card = getCard(page, cardName);
  await card.locator('button').first().click();
  await page.getByRole('menuitem', { name: menuItemLabel }).click();
}

/**
 * Opens the "Manage Linked Assets" modal for the given account, clicks the
 * asset name button to add the link, then saves.
 */
async function linkAccountToAsset(
  page: Page,
  accountName: string,
  assetName: string,
): Promise<void> {
  await openCardMenuAndClick(page, accountName, 'Manage Linked Assets');
  await expect(page.getByRole('dialog')).toBeVisible();
  // The available asset is rendered as a button inside the dialog
  await page.getByRole('dialog').getByRole('button', { name: assetName }).click();
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
}

/**
 * Opens the "Manage Linked Assets" modal for the given account, clicks the
 * Unlink icon button to remove the first linked asset, then saves.
 */
async function unlinkAccountFromAsset(page: Page, accountName: string): Promise<void> {
  await openCardMenuAndClick(page, accountName, 'Manage Linked Assets');
  await expect(page.getByRole('dialog')).toBeVisible();
  // The unlink button has aria-label="Unlink" (icon button with X)
  await page.getByRole('button', { name: 'Unlink' }).click();
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
}

// ─── Tests ───────────────────────────────────────────────────────────────────

test.describe('Account–Asset Linking', () => {
  test.beforeEach(async ({ page }) => {
    await navigateToApp(page);
  });

  // ── Test 1: Link an account to an asset via the account card menu ─────────

  test('links an account to an asset and shows the link icon on the account card', async ({
    page,
  }) => {
    await createAccount(page, 'AL-T1 Mortgage', '-5000');
    await createAsset(page, 'AL-T1 House', '100000');

    await linkAccountToAsset(page, 'AL-T1 Mortgage', 'AL-T1 House');

    // After linking, the account card should display the link indicator
    // (a <span class="cursor-help ..."> wrapping the Link2 icon)
    await expect(
      getCard(page, 'AL-T1 Mortgage').locator('span.cursor-help').first(),
    ).toBeVisible();
  });

  // ── Test 2: Linked account is excluded from the Liquid metric ─────────────

  test('linked account is excluded from the Liquid (Left to Spend) metric', async ({ page }) => {
    await createAccount(page, 'AL-T2 Savings', '8000');
    await createAsset(page, 'AL-T2 Asset');

    // With an asset present, the dashboard shows the 3-column layout:
    // Net Worth | Liquid | Left to Spend
    const liquidLabel = page.getByText('Liquid', { exact: true });
    await expect(liquidLabel).toBeVisible();

    // Capture the current Liquid value text (parent div → sibling <p>)
    const liquidSection = liquidLabel.locator('..');
    const liquidBefore = await liquidSection.innerText();

    await linkAccountToAsset(page, 'AL-T2 Savings', 'AL-T2 Asset');

    // The Liquid metric should have changed because AL-T2 Savings is now excluded
    await expect
      .poll(() => liquidSection.innerText(), { timeout: 5000 })
      .not.toEqual(liquidBefore);
  });

  // ── Test 3: Unlink restores account to Left to Spend ─────────────────────

  test('unlinking an account restores its balance to the Liquid metric', async ({ page }) => {
    await createAccount(page, 'AL-T3 Savings', '6000');
    await createAsset(page, 'AL-T3 Asset');

    // Link first
    await linkAccountToAsset(page, 'AL-T3 Savings', 'AL-T3 Asset');

    const liquidLabel = page.getByText('Liquid', { exact: true });
    const liquidSection = liquidLabel.locator('..');
    const liquidAfterLink = await liquidSection.innerText();

    // Unlink and verify metric reverts
    await unlinkAccountFromAsset(page, 'AL-T3 Savings');

    await expect
      .poll(() => liquidSection.innerText(), { timeout: 5000 })
      .not.toEqual(liquidAfterLink);

    // Link icon should no longer be visible on the account card
    await expect(getCard(page, 'AL-T3 Savings').locator('span.cursor-help')).not.toBeVisible();
  });

  // ── Test 4: Asset equity tooltip ─────────────────────────────────────────

  test('asset card shows an equity tooltip when an account is linked to it', async ({ page }) => {
    await createAccount(page, 'AL-T4 Mortgage', '-5000');
    await createAsset(page, 'AL-T4 Property', '80000');

    await linkAccountToAsset(page, 'AL-T4 Mortgage', 'AL-T4 Property');

    // The equity trigger (span.cursor-help wrapping the equity row) should
    // appear on the asset card after linking
    const equityTrigger = getCard(page, 'AL-T4 Property').locator('span.cursor-help');
    await expect(equityTrigger).toBeVisible();

    // Hovering reveals the equity breakdown tooltip
    await equityTrigger.hover();
    const tooltip = page.locator('[role="tooltip"]');
    await expect(tooltip).toBeVisible();
    await expect(tooltip).toContainText('Equity');
  });

  // ── Test 5: Delete asset unlinks accounts ─────────────────────────────────

  test('deleting an asset removes the link icon from the previously linked account card', async ({
    page,
  }) => {
    await createAccount(page, 'AL-T5 Loan', '-2000');
    await createAsset(page, 'AL-T5 Vehicle', '20000');

    await linkAccountToAsset(page, 'AL-T5 Loan', 'AL-T5 Vehicle');
    await expect(
      getCard(page, 'AL-T5 Loan').locator('span.cursor-help').first(),
    ).toBeVisible();

    // Delete the asset via its card dropdown menu
    await openCardMenuAndClick(page, 'AL-T5 Vehicle', 'Delete');
    await page.getByRole('button', { name: 'Confirm' }).click();
    await expect(page.getByRole('dialog')).not.toBeVisible();

    // The ON DELETE CASCADE removes the link; the account card should no
    // longer show the link indicator
    await expect(getCard(page, 'AL-T5 Loan').locator('span.cursor-help')).not.toBeVisible();
  });
});
