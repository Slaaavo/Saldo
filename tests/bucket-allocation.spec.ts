import { test, expect, type Page } from '@playwright/test';

/**
 * Playwright E2E tests for the Bucket–Account Linking (Capital Allocation) feature.
 *
 * The app is assumed to be running at APP_URL (pnpm tauri dev, which serves the
 * Vite dev server at the Tauri devUrl port).  Tests accumulate data in the
 * SQLite database across runs because there is no DB-reset mechanism yet; each
 * test therefore uses a unique numeric suffix in entity names to avoid
 * collisions.
 */
const APP_URL = 'http://localhost:1420';

// ─── Helpers ─────────────────────────────────────────────────────────────────

async function navigateToApp(page: Page): Promise<void> {
  await page.goto(APP_URL);
  await expect(page.getByText('Our Finances')).toBeVisible();
}

/**
 * Creates a real account via the "Add Account" button.
 * `initialBalance` is a decimal string in the consolidation currency.
 */
async function createAccount(
  page: Page,
  name: string,
  initialBalance?: string,
): Promise<void> {
  await page.getByRole('button', { name: 'Add Account' }).click();
  await page.locator('#create-account-name').fill(name);
  if (initialBalance !== undefined) {
    await page.locator('#create-account-balance').fill(initialBalance);
  }
  await page.getByRole('button', { name: 'Create' }).click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
}

/**
 * Creates a bucket via the "Add Bucket" button.
 */
async function createBucket(page: Page, name: string): Promise<void> {
  await page.getByRole('button', { name: 'Add Bucket' }).click();
  await page.locator('#create-account-name').fill(name);
  await page.getByRole('button', { name: 'Create' }).click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
}

/**
 * Returns a locator scoped to the account/bucket card whose name span carries
 * the given title attribute.  The card is the nearest ancestor div that has
 * the `shrink-0` Tailwind class applied by AccountCards.tsx.
 */
function getCard(page: Page, name: string) {
  return page
    .locator(`[title="${name}"]`)
    .locator('xpath=ancestor::div[contains(@class,"shrink-0")]');
}

/**
 * Opens the "Update Balance" modal for the card matching `name` and waits for
 * the dialog to become visible.
 */
async function openUpdateBalanceModal(page: Page, name: string): Promise<void> {
  await getCard(page, name).getByRole('button', { name: /update balance/i }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
}

/**
 * Sets a simple balance update on an account: opens its modal, fills the
 * amount, and submits.
 */
async function setAccountBalance(
  page: Page,
  accountName: string,
  amountStr: string,
): Promise<void> {
  await openUpdateBalanceModal(page, accountName);
  await page.locator('#cbu-amount').fill(amountStr);
  await page.getByRole('button', { name: 'Create' }).click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
}

/**
 * Opens the bucket's "Update Balance" modal, clicks "Link Account", selects
 * the given source account from the allocation-row combobox, enters the
 * allocation amount, and submits.  All operations use today's effective date
 * (the modal default).
 */
async function linkAccountToBucket(
  page: Page,
  bucketName: string,
  sourceAccountName: string,
  allocationAmount: string,
): Promise<void> {
  await openUpdateBalanceModal(page, bucketName);
  await expect(page.getByText('Linked Accounts')).toBeVisible();

  await page.getByRole('button', { name: 'Link Account' }).click();

  // Select the source account from the new allocation row's Radix Select combobox
  await page.getByRole('combobox').filter({ hasText: 'Select account to link' }).click();
  await page.getByRole('option', { name: sourceAccountName }).click();

  // Fill in the allocation amount (placeholder text: "Allocation Amount")
  await page.getByPlaceholder('Allocation Amount').fill(allocationAmount);

  // Submit — use .last() to select the footer's "Create" button (not "Cancel")
  await page.getByRole('button', { name: 'Create' }).last().click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
}

/**
 * Clicks the dropdown trigger (MoreVertical icon button, first button in the
 * card) and then clicks the named menu item.
 */
async function openCardMenuAndClick(
  page: Page,
  cardName: string,
  menuItemLabel: string,
): Promise<void> {
  const card = getCard(page, cardName);
  // The DropdownMenuTrigger button is the first <button> inside the card.
  // DOM order: dropdown trigger → balance display → Update Balance button.
  await card.locator('button').first().click();
  await page.getByRole('menuitem', { name: menuItemLabel }).click();
}

/**
 * Clicks a day button in whichever calendar popover is currently open.
 * react-day-picker v9 renders each day as a <button> inside a [role="gridcell"].
 * If the target month differs from the currently shown month, pass +1 / -1 for
 * `monthOffset` to navigate via the next/previous-month button first.
 *
 * @param dayStr  The day number as a string, e.g. "8".
 * @param monthOffset  How many times to click the next (+1) or previous (-1)
 *                     month arrow before clicking the day.
 */
async function clickCalendarDay(
  page: Page,
  dayStr: string,
  monthOffset: 0 | 1 | -1 = 0,
): Promise<void> {
  if (monthOffset > 0) {
    await page.getByRole('button', { name: /next month/i }).click();
  } else if (monthOffset < 0) {
    await page.getByRole('button', { name: /previous month|prev month/i }).click();
  }
  // Each day is rendered as a <button> whose complete text content is the day number.
  await page.locator(`[role="gridcell"] button:text-is("${dayStr}")`).click();
}

// ─── Tests ───────────────────────────────────────────────────────────────────

test.describe('Bucket–Account Linking (Capital Allocation)', () => {
  test.beforeEach(async ({ page }) => {
    await navigateToApp(page);
  });

  // ── Test 1: Link an account to a bucket ──────────────────────────────────

  test('links an account to a bucket and shows the allocation in the bucket card balance', async ({
    page,
  }) => {
    await createAccount(page, 'Savings T1', '10000');
    await createBucket(page, 'Emergency Fund T1');

    await linkAccountToBucket(page, 'Emergency Fund T1', 'Savings T1', '5000');

    // Bucket balance should now reflect the 5 000 allocation
    // NumberValue formats 500000 minor units as "5 000.00" (space thousands sep, dot decimal)
    await expect(getCard(page, 'Emergency Fund T1')).toContainText('5 000');
  });

  // ── Test 2: Edit an existing allocation ──────────────────────────────────

  test('edits an existing allocation amount and reflects the updated total on the bucket card', async ({
    page,
  }) => {
    await createAccount(page, 'Savings T2', '10000');
    await createBucket(page, 'Emergency Fund T2');
    await linkAccountToBucket(page, 'Emergency Fund T2', 'Savings T2', '5000');

    // Re-open the bucket modal to edit the existing allocation
    await openUpdateBalanceModal(page, 'Emergency Fund T2');
    await expect(page.getByText('Linked Accounts')).toBeVisible();

    // Existing allocation row should show the source account name (not a dropdown)
    await expect(page.getByRole('dialog').getByText('Savings T2')).toBeVisible();

    // Replace the allocation amount with a higher value
    await page.getByPlaceholder('Allocation Amount').fill('7000');

    await page.getByRole('button', { name: 'Create' }).last().click();
    await expect(page.getByRole('dialog')).not.toBeVisible();

    // Bucket card should now show the updated 7 000 total
    await expect(getCard(page, 'Emergency Fund T2')).toContainText('7 000');
  });

  // ── Test 3: Unlink an account from a bucket ───────────────────────────────

  test('unlinking an account removes its contribution from the bucket card balance', async ({
    page,
  }) => {
    await createAccount(page, 'Savings T3', '10000');
    await createBucket(page, 'Emergency Fund T3');
    await linkAccountToBucket(page, 'Emergency Fund T3', 'Savings T3', '5000');

    // Confirm the allocation is reflected before unlinking
    await expect(getCard(page, 'Emergency Fund T3')).toContainText('5 000');

    // Unlink via the bucket modal
    await openUpdateBalanceModal(page, 'Emergency Fund T3');
    await expect(page.getByText('Linked Accounts')).toBeVisible();
    await page.getByRole('button', { name: 'Unlink' }).click();

    await page.getByRole('button', { name: 'Create' }).last().click();
    await expect(page.getByRole('dialog')).not.toBeVisible();

    // Bucket balance should be 0: no manual balance and no linked allocations
    await expect(getCard(page, 'Emergency Fund T3')).toContainText('0.00');
  });

  // ── Test 4: Over-allocation validation ───────────────────────────────────

  test('shows an error message and disables submit when allocation exceeds available balance', async ({
    page,
  }) => {
    await createAccount(page, 'Savings T4', '10000');
    await createBucket(page, 'Bucket A T4');
    await createBucket(page, 'Bucket B T4');

    // Allocate the full account balance (10 000) to Bucket A — leaves 0 available
    await linkAccountToBucket(page, 'Bucket A T4', 'Savings T4', '10000');

    // Try to link the same (now fully allocated) account to Bucket B
    await openUpdateBalanceModal(page, 'Bucket B T4');
    await expect(page.getByText('Linked Accounts')).toBeVisible();
    await page.getByRole('button', { name: 'Link Account' }).click();

    await page.getByRole('combobox').filter({ hasText: 'Select account to link' }).click();
    await page.getByRole('option', { name: 'Savings T4' }).click();

    // Enter 1 — any positive amount exceeds the 0 available
    await page.getByPlaceholder('Allocation Amount').fill('1');

    // Client-side validation error must appear
    await expect(page.getByText(/Exceeds available balance/)).toBeVisible();

    // The Create / submit button must be disabled when there are errors
    await expect(page.getByRole('button', { name: 'Create' }).last()).toBeDisabled();
  });

  // ── Test 5: Over-allocation warning on account card ──────────────────────

  test('shows an amber warning indicator on the account card when balance drops below total allocations', async ({
    page,
  }) => {
    await createAccount(page, 'Savings T5', '10000');
    await createBucket(page, 'Emergency Fund T5');

    // Allocate the full account balance to the bucket
    await linkAccountToBucket(page, 'Emergency Fund T5', 'Savings T5', '10000');

    // Reduce the account balance to 5 000 — now 10 000 is allocated but only 5 000 is available
    await setAccountBalance(page, 'Savings T5', '5000');

    // The account card must show the over-allocation warning:
    // AccountCards.tsx renders <span title={overAllocationTooltip}> which starts with "Allocated …"
    const accountCard = getCard(page, 'Savings T5');
    await expect(accountCard.locator('span[title*="Allocated"]')).toBeVisible();
  });

  // ── Test 6: Cannot delete account with active allocations ─────────────────

  test('prevents deleting an account with active allocations and surfaces an error alert', async ({
    page,
  }) => {
    await createAccount(page, 'Savings T6', '10000');
    await createBucket(page, 'Emergency Fund T6');
    await linkAccountToBucket(page, 'Emergency Fund T6', 'Savings T6', '5000');

    // Capture browser-native alert dialogs triggered by handleDeleteAccount
    const alertMessages: string[] = [];
    page.on('dialog', async (dialog) => {
      alertMessages.push(dialog.message());
      await dialog.accept();
    });

    // Trigger deletion via the card dropdown menu
    await openCardMenuAndClick(page, 'Savings T6', 'Delete');

    // ConfirmDialog appears — click the destructive Confirm button
    await page.getByRole('button', { name: 'Confirm' }).click();

    // Wait until the backend error surfaces as a window.alert
    await expect
      .poll(() => alertMessages.length, { timeout: 5000 })
      .toBeGreaterThan(0);

    // The alert should mention that the account has active allocations
    expect(alertMessages.some((msg) => msg.toLowerCase().includes('allocations'))).toBe(true);

    // Deletion was blocked — the account card must still be present
    await expect(getCard(page, 'Savings T6')).toBeVisible();
  });

  // ── Test 7: Allocations respect the snapshot date ─────────────────────────

  test('allocations with a future effective date are hidden when the snapshot date is earlier', async ({
    page,
  }) => {
    await createAccount(page, 'Savings T7', '10000');
    await createBucket(page, 'Emergency Fund T7');

    // Open the bucket modal and set the allocation effective date to tomorrow
    // before linking, so the allocation does not appear at today's snapshot date.
    await openUpdateBalanceModal(page, 'Emergency Fund T7');
    await expect(page.getByText('Linked Accounts')).toBeVisible();

    // Compute tomorrow's day number and whether it falls in the next calendar month
    const today = new Date();
    const tomorrow = new Date(today);
    tomorrow.setDate(today.getDate() + 1);
    const tomorrowDay = String(tomorrow.getDate());
    const tomorrowIsNextMonth = tomorrow.getMonth() !== today.getMonth();

    // Change the modal's effective-date picker to tomorrow
    await page.locator('#cbu-date').click();
    await clickCalendarDay(page, tomorrowDay, tomorrowIsNextMonth ? 1 : 0);

    // Now add the linked account with that future effective date
    await page.getByRole('button', { name: 'Link Account' }).click();
    await page.getByRole('combobox').filter({ hasText: 'Select account to link' }).click();
    await page.getByRole('option', { name: 'Savings T7' }).click();
    await page.getByPlaceholder('Allocation Amount').fill('5000');
    await page.getByRole('button', { name: 'Create' }).last().click();
    await expect(page.getByRole('dialog')).not.toBeVisible();

    // With the snapshot date at TODAY, the allocation (effective tomorrow) must NOT appear
    await expect(getCard(page, 'Emergency Fund T7')).not.toContainText('5 000');

    // Move the header snapshot date forward to TOMORROW
    await page.locator('#date-picker').click();
    await clickCalendarDay(page, tomorrowDay, tomorrowIsNextMonth ? 1 : 0);

    // Now the allocation is within the snapshot window — it must appear on the bucket card
    await expect(getCard(page, 'Emergency Fund T7')).toContainText('5 000');
  });
});
