import { useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import type { EventWithData, SnapshotRow, Currency } from '../types';
import {
  getAccountsSnapshot,
  listEvents,
  createBalanceUpdate,
  createAccount,
  updateAccount,
  deleteAccount,
  updateEvent,
  deleteEvent,
  bulkCreateBalanceUpdates,
  getConsolidationCurrency,
  listFxRates,
  updateSortOrder,
  updateAssetValue,
  setAccountAssetLinks,
} from '../api';
import { toEndOfDay, todayIso } from '../utils/format';
import { extractErrorMessage } from '../utils/errors';

interface UseFinanceDataOptions {
  closeModal: () => void;
  onFxRatePrompt: (date: string) => void;
}

export function useFinanceData({ closeModal, onFxRatePrompt }: UseFinanceDataOptions) {
  const { t } = useTranslation();

  const [selectedDate, setSelectedDate] = useState(todayIso());
  const [snapshot, setSnapshot] = useState<SnapshotRow[]>([]);
  const [events, setEvents] = useState<EventWithData[]>([]);
  const [consolidationCurrency, setConsolidationCurrency] = useState<Currency | null>(null);

  useEffect(() => {
    getConsolidationCurrency()
      .then(setConsolidationCurrency)
      .catch((err) => console.error('Failed to load consolidation currency:', err));
  }, []);

  const refresh = useCallback(async () => {
    try {
      const endOfDay = toEndOfDay(selectedDate);
      const [snap, evts] = await Promise.all([
        getAccountsSnapshot(endOfDay),
        listEvents(undefined, endOfDay),
      ]);
      setSnapshot(snap);
      setEvents(evts);
    } catch (err) {
      toast.error(t('errors.loadData', { error: extractErrorMessage(err) }));
    }
  }, [selectedDate, t]);

  useEffect(() => {
    const load = async () => {
      await refresh();
    };
    load();
  }, [refresh]);

  const handleCreateBalanceUpdate = async (
    accountId: number,
    amountMinor: number,
    eventDate: string,
    note: string,
  ) => {
    const account = snapshot.find((r) => r.accountId === accountId);
    try {
      await createBalanceUpdate(accountId, amountMinor, eventDate, note || undefined);
      closeModal();
      await refresh();
      // Step 3.4: prompt to fetch FX rates when saving a non-consolidation-currency balance update
      if (account && consolidationCurrency && account.currencyCode !== consolidationCurrency.code) {
        const rates = await listFxRates(eventDate);
        const hasRate = rates.some((r) => r.toCurrencyCode === account.currencyCode);
        if (!hasRate) {
          onFxRatePrompt(eventDate);
        }
      }
    } catch (err) {
      toast.error(t('errors.createBalanceUpdate', { error: extractErrorMessage(err) }));
    }
  };

  const handleEditBalanceUpdate = async (
    eventId: number,
    amountMinor: number,
    eventDate: string,
    note: string,
  ) => {
    try {
      await updateEvent(eventId, amountMinor, eventDate, note || undefined);
      closeModal();
      await refresh();
    } catch (err) {
      toast.error(t('errors.updateEvent', { error: extractErrorMessage(err) }));
    }
  };

  const handleDeleteEvent = async (eventId: number) => {
    try {
      await deleteEvent(eventId);
      closeModal();
      await refresh();
    } catch (err) {
      toast.error(t('errors.deleteEvent', { error: extractErrorMessage(err) }));
    }
  };

  const handleCreateAccount = async (
    name: string,
    currencyId: number,
    initialBalanceMinor?: number,
    accountType?: string,
    linkedAssetIds?: number[],
  ) => {
    try {
      await createAccount(
        name,
        currencyId,
        initialBalanceMinor,
        accountType,
        undefined,
        linkedAssetIds,
      );
      closeModal();
      await refresh();
    } catch (err) {
      toast.error(t('errors.createAccount', { error: extractErrorMessage(err) }));
    }
  };

  const handleRenameAccount = async (accountId: number, name: string) => {
    try {
      await updateAccount(accountId, name);
      closeModal();
      await refresh();
    } catch (err) {
      toast.error(t('errors.renameAccount', { error: extractErrorMessage(err) }));
    }
  };

  const handleDeleteAccount = async (accountId: number) => {
    try {
      await deleteAccount(accountId);
      closeModal();
      await refresh();
    } catch (err) {
      const msg = extractErrorMessage(err);
      if (msg.includes('active allocations in buckets')) {
        toast.error(t('errors.deleteAccountLinked'));
      } else {
        toast.error(t('errors.deleteAccount', { error: msg }));
      }
    }
  };

  const handleBulkUpdateSubmit = async (
    updates: { accountId: number; amountMinor: number }[],
    eventDate: string,
    note: string,
  ) => {
    await bulkCreateBalanceUpdates(updates, eventDate, note || undefined);
    closeModal();
    await refresh();
  };

  const handleSaveOrder = async (orderedIds: number[]) => {
    try {
      const entries = orderedIds.map((accountId, index) => ({ accountId, sortOrder: index }));
      await updateSortOrder(entries);
      closeModal();
      await refresh();
    } catch (err) {
      toast.error(extractErrorMessage(err));
    }
  };

  const handleConsolidationCurrencyChange = useCallback(async () => {
    try {
      const currency = await getConsolidationCurrency();
      setConsolidationCurrency(currency);
      await refresh();
    } catch (err) {
      console.error('Failed to reload after currency change:', err);
    }
  }, [refresh]);

  const missingFxCurrencies = [
    ...new Set(snapshot.filter((r) => r.fxRateMissing).map((r) => r.currencyCode)),
  ];

  const handleUpdateAssetValue = async (
    accountId: number,
    amountMinor: number | null,
    pricePerUnit: string | null,
    eventDate: string,
    note: string | null,
  ) => {
    try {
      await updateAssetValue(accountId, amountMinor, pricePerUnit, eventDate, note);
      closeModal();
      await refresh();
    } catch (err) {
      toast.error(t('errors.updateAssetValue', { error: extractErrorMessage(err) }));
    }
  };

  const handleSetAccountAssetLinks = async (accountId: number, assetIds: number[]) => {
    try {
      await setAccountAssetLinks(accountId, assetIds);
      closeModal();
      await refresh();
    } catch (err) {
      toast.error(extractErrorMessage(err));
    }
  };

  return {
    selectedDate,
    setSelectedDate,
    snapshot,
    events,
    consolidationCurrency,
    refresh,
    handleCreateBalanceUpdate,
    handleEditBalanceUpdate,
    handleDeleteEvent,
    handleCreateAccount,
    handleRenameAccount,
    handleDeleteAccount,
    handleBulkUpdateSubmit,
    handleSaveOrder,
    handleConsolidationCurrencyChange,
    handleUpdateAssetValue,
    handleSetAccountAssetLinks,
    missingFxCurrencies,
  };
}
