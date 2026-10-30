# API Map

All FE API calls are defined in `src/shared/api/index.ts` and invoke Tauri backend commands via `@tauri-apps/api/core`.

---

## Accounts

### `createAccount`

Creates a new account with name, currency, optional initial balance, account type, price per unit, asset links, and IBAN.

- **BE command:** `create_account` → `src-tauri/src/features/accounts/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)
  - `src/features/assets/CreateAssetModal/CreateAssetModal.tsx` (CreateAssetModal)

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

Reorders the listed accounts: they swap among the `sort_order` slots they already hold, so accounts outside the list keep theirs.

- **BE command:** `update_sort_order` → `src-tauri/src/features/accounts/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)

---

## Transactions / Events

### `getAccountsSnapshot`
<!-- this should go to shared -->
Returns every account, bucket and asset balance live (no cutoff) or at a picked date or datetime, plus the headline metrics with their breakdown lines and the rate warnings. Takes `asOf` (`{ kind: 'live' }` or `{ kind: 'at', until }`) and an optional person.

- **BE command:** `get_accounts_snapshot` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/shared/hooks/useSnapshotQuery.ts` (useSnapshotQuery hook)
    - → `src/app/AppModals.tsx` (AppModals, live)
    - → `src/app/useModalActions.ts` (useModalActions hook, live)
    - → `src/features/dashboard/DashboardView.tsx` (DashboardView, live or the time-machine date)
    - → `src/features/ledger/LedgerPage.tsx` (LedgerPage, live)
    - → `src/features/settings/SettingsPage.tsx` (SettingsPage, live)
    - → `src/features/transactions/BulkUpdateBalanceModal.tsx` (BulkUpdateBalanceModal, at the dialog's date)

### `listEvents`

Lists events with optional filtering by account(s), bucket(s), date range, event types, person, unmatched-only, and limit. `limit` counts cards: a split group counts once and returns all its legs. Returns `{ events, totalCount }`, where `totalCount` counts all matching legs.

- **BE command:** `list_events` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/shared/hooks/useDashboardEventsQuery.ts` (useDashboardEventsQuery hook)
    - → `src/features/dashboard/DashboardView.tsx` (DashboardView)
  - `src/features/ledger/useLedgerData.ts` (useLedgerData hook)
    - → `src/features/ledger/LedgerPage.tsx` (LedgerPage)
  - `src/features/transactions/csv-import/useImportWizard.ts` (useImportWizard hook)
    - → `src/features/transactions/CsvImportModal.tsx` (CsvImportModal)

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
  - `src/shared/hooks/useEditEventHandler.ts` (useEditEventHandler hook)
    - → `src/features/ledger/LedgerPage.tsx` (LedgerPage)
    - → `src/features/dashboard/DashboardView.tsx` (DashboardView)

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

### `deleteSplitGroup`

Deletes a split group and all its legs.

- **BE command:** `delete_split_group` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/app/useModalActions.ts` (useModalActions hook)
    - → `src/app/AppModals.tsx` (AppModals)

### `createTaxableEvent`

Creates a taxable `revenue`, `expense` or `prepaid_expense` event for a person, with VAT and deductibility fields. Accepts an optional `sourceRef` (the document the entry came from, such as `online:…` for an eKasa receipt), which is stored with the event (set by the eKasa import).

- **BE command:** `create_taxable_event` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/features/transactions/CreateRevenueModal.tsx` (CreateRevenueModal)
  - `src/features/transactions/CreateExpenseModal.tsx` (CreateExpenseModal)
  - `src/features/transactions/CreatePrepaidExpenseModal.tsx` (CreatePrepaidExpenseModal)
  - `src/features/ekasa-import/ReviewStep.tsx` (ReviewStep)

### `createTaxableSplitGroup`

Creates a taxable split group (revenue or expense) with several legs. Accepts an optional `sourceRef`, which is stored with the group's entries (set by the eKasa import).

- **BE command:** `create_taxable_split_group` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/features/transactions/CreateRevenueModal.tsx` (CreateRevenueModal)
  - `src/features/transactions/CreateExpenseModal.tsx` (CreateExpenseModal)
  - `src/features/ekasa-import/ReviewStep.tsx` (ReviewStep)

### `updateTaxableEvent`

Updates a taxable event's amount, date, note and tax fields.

- **BE command:** `update_taxable_event` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/features/transactions/EditTaxableEventModal.tsx` (EditTaxableEventModal)

### `updateTaxableSplitGroup`

Updates a taxable split group and its legs.

- **BE command:** `update_taxable_split_group` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/features/transactions/EditTaxableSplitGroupModal.tsx` (EditTaxableSplitGroupModal)

### `linkCashflowsToTaxable`

Links cashflow events to a taxable event.

- **BE command:** `link_cashflows_to_taxable` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/features/transactions/EditTaxableEventModal.tsx` (EditTaxableEventModal)
  - `src/features/transactions/CreateRevenueModal.tsx` (CreateRevenueModal)
  - `src/features/transactions/CreateExpenseModal.tsx` (CreateExpenseModal)
  - `src/features/ekasa-import/ReviewStep.tsx` (ReviewStep)

### `linkCashflowsToSplitGroup`

Links cashflow events to a taxable split group.

- **BE command:** `link_cashflows_to_split_group` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/features/transactions/CreateExpenseModal.tsx` (CreateExpenseModal)
  - `src/features/ekasa-import/ReviewStep.tsx` (ReviewStep)

### `unlinkCashflowFromTaxable`

Removes the link between a cashflow and a taxable event.

- **BE command:** `unlink_cashflow_from_taxable` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/features/transactions/EditTaxableEventModal.tsx` (EditTaxableEventModal)

### `listLinkedCashflows`

Lists the cashflow events linked to a taxable event.

- **BE command:** `list_linked_cashflows` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/features/transactions/EditTaxableEventModal.tsx` (EditTaxableEventModal)

### `listEligibleCashflows`

Lists cashflows of a person that can be linked to a taxable event, optionally matching an amount and excluding already linked ones.

- **BE command:** `list_eligible_cashflows` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/features/transactions/CashflowPicker.tsx` (CashflowPicker)
    - → `src/features/transactions/EditTaxableEventModal.tsx`
    - → `src/features/transactions/CreateRevenueModal.tsx`
    - → `src/features/transactions/CreateExpenseModal.tsx`
  - `src/features/ekasa-import/ReviewStep.tsx` (ReviewStep)

### `getUnmatchedCashflowCount`

Counts cashflows not yet linked to a taxable event, optionally for one person.

- **BE command:** `get_unmatched_cashflow_count` → `src-tauri/src/features/transactions/commands.rs`
- **Used in:**
  - `src/features/ledger/LedgerPage.tsx` (LedgerPage)

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
  - `src/features/assets/CreateAssetModal/CreateAssetModal.tsx` (CreateAssetModal)
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

### `getDbDataVersion`

Gets the `PRAGMA data_version` of the app's database connection. It changes only when another process (`saldo mcp`) wrote, never for the app's own writes.

- **BE command:** `get_db_data_version` → `src-tauri/src/features/settings/commands.rs`
- **Used in:**
  - `src/app/useRefreshOnExternalWrites.ts` (useRefreshOnExternalWrites hook → App: invalidates every query when the window comes back and the version moved)

---

## Assets & Custom Units

### `createCustomUnit`

Creates a custom unit/currency with name and decimal places.

- **BE command:** `create_custom_unit` → `src-tauri/src/features/assets/commands.rs`
- **Used in:**
  - `src/features/assets/CreateAssetModal/CreateAssetModal.tsx` (CreateAssetModal)

### `listCustomUnits`

Lists all custom units (non-standard currencies).

- **BE command:** `list_custom_units` → `src-tauri/src/features/assets/commands.rs`
- **Used in:**
  - `src/features/assets/CreateAssetModal/CreateAssetModal.tsx` (CreateAssetModal)
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

## Persons

### `listPersons`

Lists all persons.

- **BE command:** `list_persons` → `src-tauri/src/features/persons/commands.rs`
- **Used in:**
  - `src/features/persons/usePersons.ts` (usePersons hook → PersonsPage)
  - `src/features/dashboard/PersonSelector.tsx` (PersonSelector)
  - `src/features/dashboard/DashboardView.tsx` (DashboardView)
  - `src/features/accounts/CreateAccountModal.tsx` (CreateAccountModal)
  - `src/features/accounts/EditAccountModal.tsx` (EditAccountModal)
  - `src/features/assets/CreateAssetModal/CreateAssetModal.tsx` (CreateAssetModal)
  - `src/features/transactions/EditTaxableEventModal.tsx` (EditTaxableEventModal)
  - `src/features/transactions/useResolvedPersonId.ts` (useResolvedPersonId hook)
  - `src/features/tax-models/TaxModelFormPage.tsx` (TaxModelFormPage)
  - `src/features/csv-profiles/ImportProfilesPage.tsx` (ImportProfilesPage)

### `createPerson`

Creates a person with name, type and VAT-payer flag.

- **BE command:** `create_person` → `src-tauri/src/features/persons/commands.rs`
- **Used in:**
  - `src/features/persons/usePersons.ts` (usePersons hook → PersonsPage)
  - `src/features/persons/CreatePersonDialog.tsx` (CreatePersonDialog)

### `updatePerson`

Updates a person's name, type and VAT-payer flag.

- **BE command:** `update_person` → `src-tauri/src/features/persons/commands.rs`
- **Used in:**
  - `src/features/persons/usePersons.ts` (usePersons hook → PersonsPage)

### `deletePerson`

Deletes a person by ID.

- **BE command:** `delete_person` → `src-tauri/src/features/persons/commands.rs`
- **Used in:**
  - `src/features/persons/usePersons.ts` (usePersons hook → PersonsPage)

---

## Tax Models

### `listTaxModels`

Lists all tax models.

- **BE command:** `list_tax_models` → `src-tauri/src/features/tax_models/commands.rs`
- **Used in:**
  - `src/features/tax-models/useTaxModels.ts` (useTaxModels hook → TaxModelsPage)

### `getTaxModel`

Gets one tax model with its brackets, minimum tax bands and forecast figures.

- **BE command:** `get_tax_model` → `src-tauri/src/features/tax_models/commands.rs`
- **Used in:**
  - `src/features/tax-models/TaxModelFormPage.tsx` (TaxModelFormPage)
  - `src/features/tax-models/TaxModelResultsPage/TaxModelResultsPage.tsx` (TaxModelResultsPage)
  - `src/features/tax-models/TaxForecastFiguresPage/TaxForecastFiguresPage.tsx` (TaxForecastFiguresPage)

### `createTaxModel`

Creates a tax model with brackets, minimum tax bands, VAT status and reserve fund settings. Its actual months default to the last ended month for the current year, December for a past year and none for a future year.

- **BE command:** `create_tax_model` → `src-tauri/src/features/tax_models/commands.rs`
- **Used in:**
  - `src/features/tax-models/useTaxModelForm.ts` (useTaxModelForm hook → TaxModelFormPage)

### `updateTaxModel`

Updates an existing tax model's settings and minimum tax bands. It leaves the forecast figures alone, and rejects a calendar year that is not after every stored earlier-profit year.

- **BE command:** `update_tax_model` → `src-tauri/src/features/tax_models/commands.rs`
- **Used in:**
  - `src/features/tax-models/useTaxModelForm.ts` (useTaxModelForm hook → TaxModelFormPage)

### `deleteTaxModel`

Deletes a tax model by ID.

- **BE command:** `delete_tax_model` → `src-tauri/src/features/tax_models/commands.rs`
- **Used in:**
  - `src/features/tax-models/useTaxModels.ts` (useTaxModels hook → TaxModelsPage)

### `calculateTaxModel`

Calculates a tax model's forecast: the five lines with their year so far, rest of year and year, the tax at the picked rate or the minimum tax, the dividends including earlier profit, and the entries behind the sheet with the reason any of them don't count.

- **BE command:** `calculate_tax_model` → `src-tauri/src/features/tax_models/commands.rs`
- **Used in:**
  - `src/features/tax-models/TaxModelResultsPage/TaxModelResultsPage.tsx` (TaxModelResultsPage)
  - `src/features/tax-models/TaxForecastFiguresPage/useTaxForecastFigures.ts` (useTaxForecastFigures hook → TaxForecastFiguresPage, for the placeholders)

### `saveTaxForecastFigures`

Replaces a tax model's forecast figures: the month the actual figures cover, the monthly payroll amount, the typed values per line, and the earlier unpaid profit per year.

- **BE command:** `save_tax_forecast_figures` → `src-tauri/src/features/tax_models/commands.rs`
- **Used in:**
  - `src/features/tax-models/TaxForecastFiguresPage/useTaxForecastFigures.ts` (useTaxForecastFigures hook → TaxForecastFiguresPage)

---

## eKasa Import

### `processReceiptFile`

Decodes the QR code from a receipt file (PDF or image), looks an online receipt up with the eKasa service and returns its items grouped by name and VAT rate, and `receiptDate` (YYYY-MM-DD). Also returns `sourceRef` (identifies the receipt) and `alreadyImported` (whether a live entry already carries that reference). An online receipt the service does not know is the error `RECEIPT_NOT_FOUND`.

- **BE command:** `process_receipt_file` → `src-tauri/src/features/ekasa/commands.rs`
- **Used in:**
  - `src/features/ekasa-import/useEkasaImportWizard.ts` (useEkasaImportWizard hook → EkasaImportModal)

### `matchEkasaRules`

Matches the wizard's rules, saved or not, against a receipt's item names with Saldo's one rule matcher (case- and accent-insensitive regular expressions, first rule wins). Returns the matched rule of each item (`null` when none matched; the wizard then uses its default row) and the rules whose pattern does not compile. Pure: no database.

- **BE command:** `match_ekasa_rules` → `src-tauri/src/features/ekasa/commands.rs`
- **Used in:**
  - `src/features/ekasa-import/useEkasaImportWizard.ts` (useEkasaImportWizard hook → EkasaImportModal)

### `listEkasaProfiles`

Lists all eKasa import profiles.

- **BE command:** `list_ekasa_profiles` → `src-tauri/src/features/ekasa/commands.rs`
- **Used in:**
  - `src/features/csv-profiles/ImportProfilesPage.tsx` (ImportProfilesPage)

### `getEkasaProfile`

Gets the eKasa import profile of a person.

- **BE command:** `get_ekasa_profile` → `src-tauri/src/features/ekasa/commands.rs`
- **Used in:**
  - `src/features/ekasa-import/useEkasaImportWizard.ts` (useEkasaImportWizard hook → EkasaImportModal)

### `upsertEkasaProfile`

Creates or replaces a person's eKasa import profile (default percentages and name-pattern rules). A pattern the rule matcher cannot compile is refused (`VALIDATION`).

- **BE command:** `upsert_ekasa_profile` → `src-tauri/src/features/ekasa/commands.rs`
- **Used in:**
  - `src/features/ekasa-import/useEkasaImportWizard.ts` (useEkasaImportWizard hook → EkasaImportModal)

### `deleteEkasaProfile`

Deletes an eKasa import profile by ID.

- **BE command:** `delete_ekasa_profile` → `src-tauri/src/features/ekasa/commands.rs`
- **Used in:**
  - `src/features/csv-profiles/ImportProfilesPage.tsx` (ImportProfilesPage)

---

## Unused API Calls

### FE API functions not called in production code

| Function | Notes |
|----------|-------|
| `createCashflow` | Only mocked in tests. Bulk variant `bulkCreateCashflows` is used instead. |
| `updateCustomUnit` | Only mocked in tests. Units page shows units but doesn't offer rename. |
| `listAccountAssetLinks` | Only mocked in tests. Links are managed via `setAccountAssetLinks` but never queried standalone in UI. |

### BE commands with no FE usage gaps

All 80 registered BE commands have corresponding FE API wrappers. No orphaned BE commands exist.
