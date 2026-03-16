import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import type { SnapshotRow, Currency, EventWithData } from '../../../shared/types/index';
import {
  listEvents,
  listPartnerAccounts,
  listCurrencies,
  createPartnerAccount,
  updateAccount,
  bulkCreateCashflows,
} from '../../../shared/api';
import { toMinorUnits } from '../../../shared/utils/format';
import { extractErrorMessage } from '../../../shared/utils/errors';
import type { WizardState, CashflowFieldKey, ImportRow, ColumnMapping } from './types';
import type { IbanLookupEntry } from './ibanMatcher';
import { buildIbanLookup, matchIban, normalizeIban } from './ibanMatcher';
import { parseCsvFile, autoDetectMapping, parseAmount, parseDateString } from './csvParser';

const INITIAL_COLUMN_MAPPING: ColumnMapping = {
  date: null,
  amount: null,
  partner: null,
  note: null,
  currency: null,
  fxRate: null,
};

const INITIAL_WIZARD_STATE: WizardState = {
  step: 'upload',
  file: null,
  csvHeaders: [],
  csvRows: [],
  selectedAccountId: null,
  columnMapping: { ...INITIAL_COLUMN_MAPPING },
  importRows: [],
  importing: false,
};

function rateToMantissaExponent(rate: number): { mantissa: number; exponent: number } {
  const str = rate.toString();
  const dotIndex = str.indexOf('.');
  if (dotIndex === -1) return { mantissa: rate, exponent: 0 };
  const decimalPlaces = str.length - dotIndex - 1;
  const mantissa = Math.round(rate * Math.pow(10, decimalPlaces));
  return { mantissa, exponent: -decimalPlaces };
}

export function useImportWizard(params: {
  snapshot: SnapshotRow[];
  onClose: () => void;
  onSuccess: () => Promise<void>;
}) {
  const { snapshot, onClose, onSuccess } = params;
  const { t } = useTranslation();

  const [wizardState, setWizardState] = useState<WizardState>(INITIAL_WIZARD_STATE);
  const [ibanLookup, setIbanLookup] = useState<Map<string, IbanLookupEntry>>(new Map());
  const [existingEvents, setExistingEvents] = useState<EventWithData[]>([]);
  const [currencies, setCurrencies] = useState<Currency[]>([]);
  const [balanceWarningDates, setBalanceWarningDates] = useState<string[]>([]);

  // Derived: selected account row from snapshot
  const selectedAccount =
    snapshot.find((r) => r.accountId === wizardState.selectedAccountId) ?? null;
  const selectedAccountCurrencyCode = selectedAccount?.currencyCode ?? '';
  const selectedAccountMinorUnits = selectedAccount?.currencyMinorUnits ?? 2;
  const selectedAccountCurrencyId =
    currencies.find((c) => c.code === selectedAccountCurrencyCode)?.id ?? 0;

  // Derived: bucket rows whose linkedAllocations don't include the selected account
  const availableBuckets = snapshot.filter(
    (r) =>
      r.accountType === 'bucket' &&
      !r.linkedAllocations.some((a) => a.sourceAccountId === wizardState.selectedAccountId),
  );

  // Derived: account-type rows that have no IBAN set
  const accountsWithoutIban = snapshot.filter((r) => r.accountType === 'account' && !r.iban);

  // Derived: selection counts
  const selectedCount = wizardState.importRows.filter((r) => r.isSelected).length;
  const duplicateCount = wizardState.importRows.filter((r) => r.isDuplicate).length;

  // ── Step 1 actions ──────────────────────────────────────────────────────────

  const handleFileSelect = useCallback(
    async (file: File) => {
      if (!file.name.toLowerCase().endsWith('.csv')) {
        toast.error(t('import.uploadStep.invalidFile'));
        return;
      }
      try {
        const { headers, rows } = await parseCsvFile(file);
        setWizardState((prev) => ({ ...prev, file, csvHeaders: headers, csvRows: rows }));
      } catch (err) {
        toast.error(extractErrorMessage(err));
      }
    },
    [t],
  );

  const handleAccountSelect = useCallback(
    async (accountId: number) => {
      setWizardState((prev) => ({ ...prev, selectedAccountId: accountId }));
      try {
        const [{ events }, partners, loadedCurrencies] = await Promise.all([
          listEvents({ accountId }),
          listPartnerAccounts(),
          listCurrencies(),
        ]);
        setExistingEvents(events);
        setCurrencies(loadedCurrencies);
        setIbanLookup(buildIbanLookup(snapshot, partners));
      } catch (err) {
        toast.error(t('errors.loadData', { error: extractErrorMessage(err) }));
      }
    },
    [snapshot, t],
  );

  const canProceedToMapping =
    wizardState.csvHeaders.length > 0 && wizardState.selectedAccountId !== null;

  const goToMapping = useCallback(() => {
    const detected = autoDetectMapping(wizardState.csvHeaders);
    setWizardState((prev) => ({ ...prev, step: 'mapping', columnMapping: detected }));
  }, [wizardState.csvHeaders]);

  // ── Step 2 actions ──────────────────────────────────────────────────────────

  const handleMappingChange = useCallback((field: CashflowFieldKey, csvColumn: string | null) => {
    setWizardState((prev) => ({
      ...prev,
      columnMapping: { ...prev.columnMapping, [field]: csvColumn },
    }));
  }, []);

  const canProceedToReview =
    wizardState.columnMapping.date !== null && wizardState.columnMapping.amount !== null;

  const goToReview = useCallback(() => {
    const { csvRows, columnMapping, selectedAccountId } = wizardState;
    if (!selectedAccountId) return;

    // Split existing events by type for duplicate detection vs balance warning
    const cashflowEvents = existingEvents.filter(
      (e) => e.eventType === 'cashflow' || e.eventType === 'transfer',
    );
    const balanceUpdateDateSet = new Set(
      existingEvents
        .filter((e) => e.eventType === 'balance_update')
        .map((e) => e.eventDate.substring(0, 10)),
    );

    const importRows: ImportRow[] = [];

    for (let i = 0; i < csvRows.length; i++) {
      const csvRow = csvRows[i];
      const rawDate = columnMapping.date ? (csvRow[columnMapping.date] ?? '') : '';
      const rawAmount = columnMapping.amount ? (csvRow[columnMapping.amount] ?? '') : '';
      const rawPartner = columnMapping.partner ? (csvRow[columnMapping.partner] ?? '') : '';
      const rawCurrency = columnMapping.currency ? (csvRow[columnMapping.currency] ?? '') : '';
      const rawFxRate = columnMapping.fxRate ? (csvRow[columnMapping.fxRate] ?? '') : '';
      const rawNote = columnMapping.note ? (csvRow[columnMapping.note] ?? '') : '';

      const parsedDate = parseDateString(rawDate);
      const parsedAmount = parseAmount(rawAmount);
      if (!parsedDate || parsedAmount === null) continue;

      const amountMinor = toMinorUnits(String(parsedAmount), selectedAccountMinorUnits);

      // IBAN matching
      let rawIban: string | null = null;
      let ibanMatch: ImportRow['ibanMatch'] = { type: 'none' };
      if (rawPartner.trim() !== '') {
        rawIban = rawPartner.trim();
        ibanMatch = matchIban(normalizeIban(rawIban), ibanLookup);
      }

      // Foreign currency handling
      let originalAmountMinor: number | null = null;
      let originalCurrencyCode: string | null = null;
      let fxRateMantissa: number | null = null;
      let fxRateExponent: number | null = null;
      const csvCurrencyCode = rawCurrency.trim().toUpperCase();
      if (csvCurrencyCode !== '' && csvCurrencyCode !== selectedAccountCurrencyCode.toUpperCase()) {
        const foreignCurrency = currencies.find((c) => c.code === csvCurrencyCode);
        if (foreignCurrency) {
          originalCurrencyCode = foreignCurrency.code;
          originalAmountMinor = toMinorUnits(String(parsedAmount), foreignCurrency.minorUnits);
        }
      }

      // FX rate
      if (rawFxRate.trim() !== '') {
        const parsedRate = parseAmount(rawFxRate);
        if (parsedRate !== null && parsedRate > 0) {
          const { mantissa, exponent } = rateToMantissaExponent(parsedRate);
          fxRateMantissa = mantissa;
          fxRateExponent = exponent;
        }
      }

      // Duplicate detection: match YYYY-MM-DD and amount against existing cashflow/transfer events
      const datePrefix = parsedDate.substring(0, 10);
      const isDuplicate = cashflowEvents.some(
        (e) => e.eventDate.substring(0, 10) === datePrefix && e.amountMinor === amountMinor,
      );

      importRows.push({
        index: i,
        date: parsedDate,
        amountMinor,
        currencyCode: selectedAccountCurrencyCode,
        originalAmountMinor,
        originalCurrencyCode,
        fxRateMantissa,
        fxRateExponent,
        note: rawNote.trim() !== '' ? rawNote.trim() : null,
        rawIban,
        ibanMatch,
        isDuplicate,
        isSelected: !isDuplicate,
        bucketId: null,
        counterpartAccountId: null,
      });
    }

    // Balance warning: CSV row dates that also have a balance_update event
    const csvDateSet = new Set(importRows.map((r) => r.date.substring(0, 10)));
    const warnings = [...csvDateSet].filter((d) => balanceUpdateDateSet.has(d));

    setBalanceWarningDates(warnings);
    setWizardState((prev) => ({ ...prev, step: 'review', importRows }));
  }, [
    wizardState,
    existingEvents,
    ibanLookup,
    currencies,
    selectedAccountCurrencyCode,
    selectedAccountMinorUnits,
  ]);

  // ── Step 3 actions ──────────────────────────────────────────────────────────

  const handleToggleRow = useCallback((index: number) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) =>
        r.index === index ? { ...r, isSelected: !r.isSelected } : r,
      ),
    }));
  }, []);

  const handleSelectAll = useCallback(() => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => ({ ...r, isSelected: true })),
    }));
  }, []);

  const handleDeselectAll = useCallback(() => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => ({ ...r, isSelected: false })),
    }));
  }, []);

  const handleBucketChange = useCallback((index: number, bucketId: number | null) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => (r.index === index ? { ...r, bucketId } : r)),
    }));
  }, []);

  const handleCounterpartChange = useCallback(
    (index: number, accountId: number | null) => {
      setWizardState((prev) => ({
        ...prev,
        importRows: prev.importRows.map((r) => {
          if (r.index !== index) return r;
          if (accountId === null) {
            return { ...r, counterpartAccountId: null, ibanMatch: { type: 'none' } };
          }
          const account = snapshot.find((s) => s.accountId === accountId);
          if (!account) return r;
          const ibanMatch: ImportRow['ibanMatch'] =
            account.accountType === 'account'
              ? { type: 'ownAccount', accountId, accountName: account.accountName }
              : { type: 'partner', accountId, accountName: account.accountName };
          return { ...r, counterpartAccountId: accountId, ibanMatch };
        }),
      }));
    },
    [snapshot],
  );

  const handleCreatePartner = useCallback(
    async (iban: string, name: string) => {
      if (!selectedAccountCurrencyId) return;
      const normalizedIban = normalizeIban(iban);
      try {
        const newId = await createPartnerAccount(name, selectedAccountCurrencyId, normalizedIban);
        const entry: IbanLookupEntry = {
          accountId: newId,
          accountName: name,
          accountType: 'partner',
          iban: normalizedIban,
        };
        setIbanLookup((prev) => new Map(prev).set(normalizedIban, entry));
        setWizardState((prev) => ({
          ...prev,
          importRows: prev.importRows.map((r) => {
            if (!r.rawIban || normalizeIban(r.rawIban) !== normalizedIban) return r;
            return { ...r, ibanMatch: { type: 'partner', accountId: newId, accountName: name } };
          }),
        }));
        toast.success(t('import.reviewStep.partnerCreated'));
      } catch (err) {
        toast.error(t('errors.createPartner', { error: extractErrorMessage(err) }));
      }
    },
    [selectedAccountCurrencyId, t],
  );

  const handleAssignIban = useCallback(
    async (iban: string, targetAccountId: number) => {
      const targetRow = snapshot.find((r) => r.accountId === targetAccountId);
      if (!targetRow) return;
      const normalizedIban = normalizeIban(iban);
      try {
        await updateAccount(targetAccountId, targetRow.accountName, normalizedIban);
        const entry: IbanLookupEntry = {
          accountId: targetAccountId,
          accountName: targetRow.accountName,
          accountType: 'account',
          iban: normalizedIban,
        };
        setIbanLookup((prev) => new Map(prev).set(normalizedIban, entry));
        setWizardState((prev) => ({
          ...prev,
          importRows: prev.importRows.map((r) => {
            if (!r.rawIban || normalizeIban(r.rawIban) !== normalizedIban) return r;
            return {
              ...r,
              ibanMatch: {
                type: 'ownAccount',
                accountId: targetAccountId,
                accountName: targetRow.accountName,
              },
            };
          }),
        }));
        toast.success(t('import.reviewStep.accountAssigned'));
      } catch (err) {
        toast.error(extractErrorMessage(err));
      }
    },
    [snapshot, t],
  );

  const handleImport = useCallback(async () => {
    const { selectedAccountId, importRows } = wizardState;
    if (!selectedAccountId) return;

    setWizardState((prev) => ({ ...prev, importing: true }));
    try {
      const selectedRows = importRows.filter((r) => r.isSelected);
      const entries = selectedRows.map((row) => {
        const counterpartAccountId =
          row.ibanMatch.type === 'ownAccount' || row.ibanMatch.type === 'partner'
            ? row.ibanMatch.accountId
            : undefined;

        const originalCurrencyId = row.originalCurrencyCode
          ? currencies.find((c) => c.code === row.originalCurrencyCode)?.id
          : undefined;

        return {
          accountId: selectedAccountId,
          amountMinor: row.amountMinor,
          eventDate: row.date,
          note: row.note ?? undefined,
          counterpartAccountId,
          bucketId: row.bucketId ?? undefined,
          originalCurrencyId,
          originalAmountMinor: row.originalAmountMinor ?? undefined,
          fxRateMantissa: row.fxRateMantissa ?? undefined,
          fxRateExponent: row.fxRateExponent ?? undefined,
        };
      });

      await bulkCreateCashflows({ entries });
      toast.success(t('import.success', { count: selectedRows.length }));
      await onSuccess();
      onClose();
    } catch (err) {
      toast.error(t('errors.importCashflows', { error: extractErrorMessage(err) }));
    } finally {
      setWizardState((prev) => ({ ...prev, importing: false }));
    }
  }, [wizardState, currencies, t, onSuccess, onClose]);

  const goBack = useCallback(() => {
    setWizardState((prev) => ({
      ...prev,
      step: prev.step === 'review' ? 'mapping' : 'upload',
    }));
  }, []);

  return {
    wizardState,
    ibanLookup,
    availableBuckets,
    selectedCount,
    duplicateCount,
    balanceWarningDates,
    accountsWithoutIban,
    selectedAccountCurrencyCode,
    selectedAccountCurrencyId,
    selectedAccountMinorUnits,
    handleFileSelect,
    handleAccountSelect,
    canProceedToMapping,
    goToMapping,
    handleMappingChange,
    canProceedToReview,
    goToReview,
    handleToggleRow,
    handleSelectAll,
    handleDeselectAll,
    handleBucketChange,
    handleCounterpartChange,
    handleCreatePartner,
    handleAssignIban,
    handleImport,
    goBack,
  };
}
