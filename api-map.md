# API Map

All FE API calls are defined in `src/shared/api/index.ts` and invoke Tauri backend commands via `@tauri-apps/api/core`.

---

## Accounts

### `createAccount`

Creates a new account with name, currency, optional initial balance, account type, price per unit, asset links, and IBAN.

- **BE command:** `create_account` → `src-tauri/src/features/accounts/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)
  - `src/features/assets/CreateAssetModal.tsx` (CreateAssetModal)

### `updateAccount`

Updates an account's name and IBAN.

- **BE command:** `update_account` → `src-tauri/src/features/accounts/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)

### `deleteAccount`

Deletes an account by ID.

- **BE command:** `delete_account` → `src-tauri/src/features/accounts/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)

### `updateSortOrder`

Updates display sort order for multiple accounts.

- **BE command:** `update_sort_order` → `src-tauri/src/features/accounts/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)

---

## Transactions / Events

### `getAccountsSnapshot`
<!-- this should go to shared -->
Returns all account balances as of a given date (end-of-day).

- **BE command:** `get_accounts_snapshot` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/shared/hooks/useSnapshotQuery.ts` (useSnapshotQuery hook)
    - → `src/app/AppModals.tsx` (AppModals)
    - → `src/app/useModalActions.ts` (useModalActions hook)
    - → `src/features/dashboard/DashboardView.tsx` (DashboardView)
    - → `src/features/ledger/LedgerPage.tsx` (LedgerPage)
    - → `src/features/settings/SettingsPage.tsx` (SettingsPage)

### `listEvents`

Lists events with optional filtering by account(s), bucket(s), date range, event types, and limit. Returns `{ events, totalCount }`.

- **BE command:** `list_events` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/shared/hooks/useDashboardEventsQuery.ts` (useDashboardEventsQuery hook)
    - → `src/features/dashboard/DashboardView.tsx` (DashboardView)
  - `src/features/ledger/useLedgerData.ts` (useLedgerData hook)
    - → `src/features/ledger/LedgerPage.tsx` (LedgerPage)

### `createBalanceUpdate`

Creates a single balance update event on an account.

- **BE command:** `create_balance_update` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)

### `updateEvent`

Updates an existing event's amount, date, and note.

- **BE command:** `update_event` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)

### `deleteEvent`

Deletes an event by ID.

- **BE command:** `delete_event` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)

### `getEventById`

Retrieves a single event with all its data. For transfers, includes counterpart leg info. Used to fetch the counterpart leg of a transfer when opening the edit-transfer modal — the event list only contains one leg, so this call fetches the linked event to populate both sides.

- **BE command:** `get_event_by_id` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/features/ledger/LedgerPage.tsx` (LedgerPage)
  - `src/features/dashboard/DashboardView.tsx` (DashboardView)

### `bulkCreateBalanceUpdates`

Creates multiple balance updates across accounts on the same date with a shared note.

- **BE command:** `bulk_create_balance_updates` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)

### `updateTransfer`

Updates a transfer between two accounts (both legs' amounts, dates, FX rates).

- **BE command:** `update_transfer` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)

---

## Cashflows

### `createCashflow`

Creates a cashflow event (income/expense or transfer with optional FX conversion, counterpart account, bucket).

- **BE command:** `create_cashflow` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - **Not used in production code** (test mocks only)

### `bulkCreateCashflows`

Creates multiple cashflow entries in bulk.

- **BE command:** `bulk_create_cashflows` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/features/transactions/csv-import/useImportWizard.ts` (useImportWizard hook)
    - → `src/features/transactions/CsvImportModal.tsx` (CsvImportModal)

### `createSplitGroup`

Creates a split group with multiple legs for splitting one transaction across categories/counterparts.

- **BE command:** `create_split_group` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/features/transactions/csv-import/useImportWizard.ts` (useImportWizard hook)
    - → `src/features/transactions/CsvImportModal.tsx` (CsvImportModal)

### `updateSplitGroupDate`

Updates the date for all legs of a split group.

- **BE command:** `update_split_group_date` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - **Not used in production code** (test mocks only)

---

## Buckets

### `createBucketBalanceUpdate`

Creates a balance update on a bucket account with linked regular account IDs.

- **BE command:** `create_bucket_balance_update` → `src-tauri/src/features/buckets/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)

### `updateBucketBalanceUpdate`

Updates an existing bucket balance update (amount, date, linked accounts).

- **BE command:** `update_bucket_balance_update` → `src-tauri/src/features/buckets/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)

### `listLinksForEvent`

Lists all bucket–account links created for a specific event.

- **BE command:** `list_links_for_event` → `src-tauri/src/features/buckets/commands.rs`
- **Used in:**
  - `src/features/buckets/useBucketLinks.ts` (useBucketLinks hook)
    - → `src/features/transactions/EditBalanceUpdateModal.tsx`
    - → `src/features/transactions/CreateBalanceUpdateModal.tsx`

### `getLatestBucketLinks`

Gets the latest active bucket–account links as of a specific date.

- **BE command:** `get_latest_bucket_links` → `src-tauri/src/features/buckets/commands.rs`
- **Used in:**
  - `src/features/buckets/useBucketLinks.ts` (useBucketLinks hook)
    - → `src/features/transactions/EditBalanceUpdateModal.tsx`
    - → `src/features/transactions/CreateBalanceUpdateModal.tsx`

---

## Currencies & FX Rates

### `listCurrencies`

Lists all currencies, optionally including custom units.

- **BE command:** `list_currencies` → `src-tauri/src/features/currency/commands.rs`
- **Used in:**
  - `src/features/accounts/CreateAccountModal.tsx` (CreateAccountModal)
  - `src/features/currency/useFxRates.ts` (useFxRates hook → FxRatesPage)
  - `src/features/settings/useSettings.ts` (useSettings hook → SettingsPage)

### `getConsolidationCurrency`

Gets the current consolidation/base currency.

- **BE command:** `get_consolidation_currency` → `src-tauri/src/features/currency/commands.rs`
- **Used in:**
  - `src/features/accounts/CreateAccountModal.tsx` (CreateAccountModal)
  - `src/features/currency/useFxRates.ts` (useFxRates hook → FxRatesPage)
  - `src/features/settings/useSettings.ts` (useSettings hook → SettingsPage)
  - `src/features/partners/usePartners.ts` (usePartners hook → PartnersPage)
  - `src/features/assets/CreateAssetModal.tsx` (CreateAssetModal)
  - `src/features/assets/UnitsPage.tsx` (UnitsPage)

### `setConsolidationCurrency`

Sets the consolidation currency by ID.

- **BE command:** `set_consolidation_currency` → `src-tauri/src/features/currency/commands.rs`
- **Used in:**
  - `src/features/settings/useSettings.ts` (useSettings hook → SettingsPage)

### `setFxRateManual`

Manually sets an FX rate between two currencies for a specific date.

- **BE command:** `set_fx_rate_manual` → `src-tauri/src/features/currency/commands.rs`
- **Used in:**
  - `src/features/currency/useFxRates.ts` (useFxRates hook → FxRatesPage)
  - `src/features/assets/UnitsPage.tsx` (UnitsPage)

### `listFxRates`

Lists all stored FX rates, optionally filtered by date.

- **BE command:** `list_fx_rates` → `src-tauri/src/features/currency/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)
  - `src/features/assets/UpdateAssetValueModal.tsx` (UpdateAssetValueModal)
  - `src/features/currency/useFxRates.ts` (useFxRates hook → FxRatesPage)
  - `src/features/assets/UnitsPage.tsx` (UnitsPage)

### `fetchFxRates`

Fetches FX rates from OXR API. Skips if all rates already exist unless `force=true`.

- **BE command:** `fetch_fx_rates` → `src-tauri/src/features/currency/commands.rs`
- **Used in:**
  - `src/app/AppModals.tsx` (AppModals — startup auto-refresh)
  - `src/features/currency/useFxRates.ts` (useFxRates hook → FxRatesPage)

### `getMissingRateDates`

Returns dates that need FX rates fetched.

- **BE command:** `get_missing_rate_dates` → `src-tauri/src/features/currency/commands.rs`
- **Used in:**
  - `src/features/currency/useFxRates.ts` (useFxRates hook → FxRatesPage)

---

## Settings & App Configuration

### `getAppSetting`

Gets a settings value by key (e.g. `oxr_app_id`).

- **BE command:** `get_app_setting` → `src-tauri/src/features/settings/commands.rs`
- **Used in:**
  - `src/features/settings/useSettings.ts` (useSettings hook → SettingsPage)

### `setAppSetting`

Sets a settings value by key.

- **BE command:** `set_app_setting` → `src-tauri/src/features/settings/commands.rs`
- **Used in:**
  - `src/features/settings/useSettings.ts` (useSettings hook → SettingsPage)

### `getBulkUpdateExclusions`

Gets list of account IDs excluded from bulk updates.

- **BE command:** `get_bulk_update_exclusions` → `src-tauri/src/features/settings/commands.rs`
- **Used in:**
  - `src/shared/hooks/useBulkUpdateExclusionsQuery.ts` (useBulkUpdateExclusionsQuery hook)
    - → `src/app/AppModals.tsx` (AppModals)
    - → `src/features/settings/SettingsPage.tsx` (SettingsPage)

### `setBulkUpdateExclusions`

Sets which accounts are excluded from bulk updates.

- **BE command:** `set_bulk_update_exclusions` → `src-tauri/src/features/settings/commands.rs`
- **Used in:**
  - `src/features/settings/SettingsPage.tsx` (SettingsPage)

---

## Demo Mode & DB Location

### `enterDemoMode`

Activates demo mode with an ephemeral in-memory database.

- **BE command:** `enter_demo_mode` → `src-tauri/src/features/settings/commands.rs`
- **Used in:**
  - `src/features/settings/useDemoMode.ts` (useDemoMode hook → App)

### `exitDemoMode`

Exits demo mode, restoring the persistent database.

- **BE command:** `exit_demo_mode` → `src-tauri/src/features/settings/commands.rs`
- **Used in:**
  - `src/features/settings/useDemoMode.ts` (useDemoMode hook → App)

### `isDemoMode`

Checks if app is currently in demo mode.

- **BE command:** `is_demo_mode` → `src-tauri/src/features/settings/commands.rs`
- **Used in:**
  - `src/features/settings/useDemoMode.ts` (useDemoMode hook → App)

### `getDbLocation`

Gets current database location info (path, custom folder flag, missing flag).

- **BE command:** `get_db_location` → `src-tauri/src/features/settings/commands.rs`
- **Used in:**
  - `src/features/settings/useDbLocation.ts` (useDbLocation hook → App, SettingsPage)

### `pickDbFolder`

Opens a system folder picker dialog to select a custom database location.

- **BE command:** `pick_db_folder` → `src-tauri/src/features/settings/commands.rs`
- **Used in:**
  - `src/features/settings/useDbLocation.ts` (useDbLocation hook → App, SettingsPage)

### `changeDbLocation`

Migrates database to a new folder with an action (copy/move/use existing).

- **BE command:** `change_db_location` → `src-tauri/src/features/settings/commands.rs`
- **Used in:**
  - `src/features/settings/useDbLocation.ts` (useDbLocation hook → App, SettingsPage)

### `resetDbLocation`

Resets database location back to the default OS path.

- **BE command:** `reset_db_location` → `src-tauri/src/features/settings/commands.rs`
- **Used in:**
  - `src/features/settings/useDbLocation.ts` (useDbLocation hook → App, SettingsPage)

### `checkDefaultDb`

Checks if a database file exists at the default OS location.

- **BE command:** `check_default_db` → `src-tauri/src/features/settings/commands.rs`
- **Used in:**
  - `src/features/settings/useDbLocation.ts` (useDbLocation hook → App, SettingsPage)

---

## Assets & Custom Units

### `createCustomUnit`

Creates a custom unit/currency with name and decimal places.

- **BE command:** `create_custom_unit` → `src-tauri/src/features/assets/commands.rs`
- **Used in:**
  - `src/features/assets/CreateAssetModal.tsx` (CreateAssetModal)

### `listCustomUnits`

Lists all custom units (non-standard currencies).

- **BE command:** `list_custom_units` → `src-tauri/src/features/assets/commands.rs`
- **Used in:**
  - `src/features/assets/CreateAssetModal.tsx` (CreateAssetModal)
  - `src/features/assets/UnitsPage.tsx` (UnitsPage)

### `updateCustomUnit`

Updates a custom unit's name.

- **BE command:** `update_custom_unit` → `src-tauri/src/features/assets/commands.rs`
- **Used in:**
  - **Not used in production code** (test mocks only)

### `updateAssetValue`

Updates asset account value (amount and/or price per unit) on a specific date.

- **BE command:** `update_asset_value` → `src-tauri/src/features/assets/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)

### `listAccountAssetLinks`

Lists asset accounts linked to regular accounts, optionally filtered by account.

- **BE command:** `list_account_asset_links` → `src-tauri/src/features/assets/commands.rs`
- **Used in:**
  - **Not used in production code** (test mocks only)

### `setAccountAssetLinks`

Updates which asset accounts are linked to a regular account.

- **BE command:** `set_account_asset_links` → `src-tauri/src/features/assets/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)

---

## Partner Accounts

### `createPartnerAccount`

Creates an external partner account with name, currency, and IBAN.

- **BE command:** `create_partner_account` → `src-tauri/src/features/partner_accounts/commands.rs`
- **Used in:**
  - `src/features/transactions/csv-import/useImportWizard.ts` (useImportWizard hook → CsvImportModal)
  - `src/features/partners/usePartners.ts` (usePartners hook → PartnersPage)

### `listPartnerAccounts`

Lists all partner accounts.

- **BE command:** `list_partner_accounts` → `src-tauri/src/features/partner_accounts/commands.rs`
- **Used in:**
  - `src/features/partners/usePartners.ts` (usePartners hook → PartnersPage)

### `updatePartnerAccount`

Updates a partner account's name and IBAN.

- **BE command:** `update_partner_account` → `src-tauri/src/features/partner_accounts/commands.rs`
- **Used in:**
  - `src/features/partners/usePartners.ts` (usePartners hook → PartnersPage)

### `deletePartnerAccount`

Deletes a partner account by ID.

- **BE command:** `delete_partner_account` → `src-tauri/src/features/partner_accounts/commands.rs`
- **Used in:**
  - `src/features/partners/usePartners.ts` (usePartners hook → PartnersPage)

---

## CSV Import Profiles

### `listImportProfiles`

Lists all saved CSV import profiles.

- **BE command:** `list_import_profiles` → `src-tauri/src/features/csv_profiles/commands.rs`
- **Used in:**
  - `src/features/csv-profiles/useImportProfiles.ts` (useImportProfiles hook → ImportProfilesPage)
  - `src/features/transactions/csv-import/useImportWizard.ts` (useImportWizard hook → CsvImportModal)

### `createImportProfile`

Creates a new CSV import profile with column mapping and transformation rules.

- **BE command:** `create_import_profile` → `src-tauri/src/features/csv_profiles/commands.rs`
- **Used in:**
  - `src/features/transactions/csv-import/useImportWizard.ts` (useImportWizard hook → CsvImportModal)

### `updateImportProfile`

Updates an existing CSV import profile.

- **BE command:** `update_import_profile` → `src-tauri/src/features/csv_profiles/commands.rs`
- **Used in:**
  - `src/features/transactions/csv-import/useImportWizard.ts` (useImportWizard hook → CsvImportModal)

### `deleteImportProfile`

Deletes a CSV import profile by ID.

- **BE command:** `delete_import_profile` → `src-tauri/src/features/csv_profiles/commands.rs`
- **Used in:**
  - `src/features/csv-profiles/useImportProfiles.ts` (useImportProfiles hook → ImportProfilesPage)

### `getPreferredProfile`

Gets the CSV import profile preferred for a specific account.

- **BE command:** `get_preferred_profile` → `src-tauri/src/features/csv_profiles/commands.rs`
- **Used in:**
  - `src/features/transactions/csv-import/useImportWizard.ts` (useImportWizard hook → CsvImportModal)

### `setPreferredProfile`

Sets which CSV import profile is preferred for an account.

- **BE command:** `set_preferred_profile` → `src-tauri/src/features/csv_profiles/commands.rs`
- **Used in:**
  - `src/features/transactions/csv-import/useImportWizard.ts` (useImportWizard hook → CsvImportModal)

---

## Unused API Calls

### FE API functions not called in production code

| Function | Notes |
|----------|-------|
| `createCashflow` | Only mocked in tests. Bulk variant `bulkCreateCashflows` is used instead. |
| `updateSplitGroupDate` | Only mocked in tests. No UI for editing split group dates yet. |
| `updateCustomUnit` | Only mocked in tests. Units page shows units but doesn't offer rename. |
| `listAccountAssetLinks` | Only mocked in tests. Links are managed via `setAccountAssetLinks` but never queried standalone in UI. |

### BE commands with no FE usage gaps

All 55 registered BE commands have corresponding FE API wrappers. No orphaned BE commands exist.
