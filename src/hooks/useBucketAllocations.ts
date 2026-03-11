import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import type { SnapshotRow } from '../types';
import { toMinorUnits, fromMinorUnits } from '../utils/format';
import { extractErrorMessage } from '../utils/errors';
import { createBucketAllocation, getAccountAllocatedTotal, listBucketAllocations } from '../api';

export interface AllocationRow {
  tempId: string;
  sourceAccountId: number | null;
  amountStr: string;
  originalAmountMinor: number;
  isNew: boolean;
  isUnlinked: boolean;
}

interface UseBucketAllocationsParams {
  bucketId: number | null;
  date: string;
  allocationSources: SnapshotRow[];
}

export interface UseBucketAllocationsReturn {
  loadingAllocations: boolean;
  visibleAllocations: AllocationRow[];
  availableToLink: SnapshotRow[];
  displayErrors: Record<string, string>;
  hasErrors: boolean;
  getAvailableBalance: (row: AllocationRow) => number;
  handleSourceAccountSelect: (tempId: string, newSourceAccountId: number) => Promise<void>;
  handleAddAllocation: () => void;
  handleUnlink: (tempId: string) => void;
  handleRemoveNew: (tempId: string) => void;
  handleAllocationAmountChange: (tempId: string, value: string) => void;
  saveAllocations: () => Promise<boolean>;
}

export function useBucketAllocations({
  bucketId,
  date,
  allocationSources,
}: UseBucketAllocationsParams): UseBucketAllocationsReturn {
  const isBucket = bucketId !== null;
  const { t } = useTranslation();
  const tempIdCounter = useRef(0);
  const nextTempId = () => `alloc-${tempIdCounter.current++}`;

  const [allocations, setAllocations] = useState<AllocationRow[]>([]);
  const [allocatedTotals, setAllocatedTotals] = useState<Map<number, number>>(new Map());
  const [loadingAllocations, setLoadingAllocations] = useState(false);
  const [backendErrors, setBackendErrors] = useState<Record<string, string>>({});

  const loadAllocations = useCallback(async (bktId: number, asOfDate: string) => {
    setLoadingAllocations(true);
    setAllocations([]);
    setAllocatedTotals(new Map());
    setBackendErrors({});
    try {
      const existing = await listBucketAllocations(bktId, asOfDate);
      const totalsMap = new Map<number, number>();
      await Promise.all(
        existing.map(async (alloc) => {
          const total = await getAccountAllocatedTotal(alloc.sourceAccountId, asOfDate);
          totalsMap.set(alloc.sourceAccountId, total);
        }),
      );
      setAllocatedTotals(totalsMap);
      setAllocations(
        existing.map((alloc) => ({
          tempId: nextTempId(),
          sourceAccountId: alloc.sourceAccountId,
          amountStr: fromMinorUnits(alloc.amountMinor, alloc.sourceCurrencyMinorUnits),
          originalAmountMinor: alloc.amountMinor,
          isNew: false,
          isUnlinked: false,
        })),
      );
    } catch (err) {
      console.error('Failed to load bucket allocations:', err);
    } finally {
      setLoadingAllocations(false);
    }
  }, []); // nextTempId reads from a stable ref — safe with empty deps

  useEffect(() => {
    if (isBucket && bucketId !== null) {
      void loadAllocations(bucketId, date);
    } else {
      setAllocations([]);
      setAllocatedTotals(new Map());
      setBackendErrors({});
    }
  }, [bucketId, date, isBucket, loadAllocations]);

  const getAvailableBalance = (row: AllocationRow): number => {
    if (row.sourceAccountId === null) return 0;
    const sourceAccount = allocationSources.find((a) => a.accountId === row.sourceAccountId);
    if (!sourceAccount) return 0;
    const allocated = allocatedTotals.get(row.sourceAccountId) ?? 0;
    return sourceAccount.balanceMinor - allocated + row.originalAmountMinor;
  };

  const handleSourceAccountSelect = async (tempId: string, newSourceAccountId: number) => {
    setAllocations((prev) =>
      prev.map((row) =>
        row.tempId === tempId ? { ...row, sourceAccountId: newSourceAccountId } : row,
      ),
    );
    setBackendErrors((prev) => {
      const next = { ...prev };
      delete next[tempId];
      return next;
    });
    if (!allocatedTotals.has(newSourceAccountId)) {
      try {
        const total = await getAccountAllocatedTotal(newSourceAccountId, date);
        setAllocatedTotals((prev) => new Map(prev).set(newSourceAccountId, total));
      } catch (err) {
        console.error('Failed to get allocated total:', err);
      }
    }
  };

  const handleAddAllocation = () => {
    setAllocations((prev) => [
      ...prev,
      {
        tempId: nextTempId(),
        sourceAccountId: null,
        amountStr: '',
        originalAmountMinor: 0,
        isNew: true,
        isUnlinked: false,
      },
    ]);
  };

  const handleUnlink = (tempId: string) => {
    setAllocations((prev) =>
      prev.map((row) => (row.tempId === tempId ? { ...row, isUnlinked: true } : row)),
    );
    setBackendErrors((prev) => {
      const next = { ...prev };
      delete next[tempId];
      return next;
    });
  };

  const handleRemoveNew = (tempId: string) => {
    setAllocations((prev) => prev.filter((row) => row.tempId !== tempId));
    setBackendErrors((prev) => {
      const next = { ...prev };
      delete next[tempId];
      return next;
    });
  };

  const handleAllocationAmountChange = (tempId: string, value: string) => {
    setAllocations((prev) =>
      prev.map((row) => (row.tempId === tempId ? { ...row, amountStr: value } : row)),
    );
    setBackendErrors((prev) => {
      const next = { ...prev };
      delete next[tempId];
      return next;
    });
  };

  // Compute client-side validation errors live for display and to disable the submit button
  const clientErrors: Record<string, string> = {};
  if (isBucket) {
    for (const row of allocations) {
      if (row.isUnlinked || row.sourceAccountId === null) continue;
      const sourceAccount = allocationSources.find((a) => a.accountId === row.sourceAccountId);
      const sourceMinorUnits = sourceAccount?.currencyMinorUnits ?? 2;
      const parsed = parseFloat(row.amountStr);
      if (row.amountStr !== '' && (isNaN(parsed) || parsed < 0)) {
        clientErrors[row.tempId] = t('validation.invalidAmount');
        continue;
      }
      if (!isNaN(parsed) && parsed >= 0) {
        const amountMinor = toMinorUnits(row.amountStr, sourceMinorUnits);
        const available = getAvailableBalance(row);
        if (amountMinor > available) {
          const availableDecimal = fromMinorUnits(available, sourceMinorUnits);
          clientErrors[row.tempId] = t('modals.createBalanceUpdate.exceedsAvailable', {
            amount: availableDecimal,
            currency: sourceAccount?.currencyCode ?? '',
          });
        }
      }
    }
  }

  const displayErrors = { ...clientErrors, ...backendErrors };
  const hasErrors = Object.keys(displayErrors).length > 0;

  const saveAllocations = async (): Promise<boolean> => {
    if (bucketId === null) return true;
    const apiErrors: Record<string, string> = {};
    for (const row of allocations) {
      if (row.sourceAccountId === null) continue;
      const sourceAccount = allocationSources.find((a) => a.accountId === row.sourceAccountId);
      const sourceMinorUnits = sourceAccount?.currencyMinorUnits ?? 2;
      let amountMinor: number;
      if (row.isUnlinked) {
        amountMinor = 0;
      } else {
        const parsedAmount = parseFloat(row.amountStr);
        if (isNaN(parsedAmount) || parsedAmount < 0) {
          apiErrors[row.tempId] = t('validation.invalidAmount');
          continue;
        }
        amountMinor = toMinorUnits(row.amountStr, sourceMinorUnits);
        if (!row.isNew && amountMinor === row.originalAmountMinor) {
          continue; // unchanged existing allocation, skip
        }
      }
      try {
        await createBucketAllocation(bucketId, row.sourceAccountId, amountMinor, date);
      } catch (err) {
        const errStr = extractErrorMessage(err);
        if (errStr.includes('OVER_ALLOCATION')) {
          apiErrors[row.tempId] = t('errors.overAllocation');
        } else {
          throw err;
        }
      }
    }
    if (Object.keys(apiErrors).length > 0) {
      setBackendErrors(apiErrors);
      return false;
    }
    return true;
  };

  const visibleAllocations = allocations.filter((row) => !row.isUnlinked);
  const linkedAccountIds = new Set(
    visibleAllocations
      .filter((r) => r.sourceAccountId !== null)
      .map((r) => r.sourceAccountId as number),
  );
  const availableToLink = allocationSources.filter((a) => !linkedAccountIds.has(a.accountId));

  return {
    loadingAllocations,
    visibleAllocations,
    availableToLink,
    displayErrors,
    hasErrors,
    getAvailableBalance,
    handleSourceAccountSelect,
    handleAddAllocation,
    handleUnlink,
    handleRemoveNew,
    handleAllocationAmountChange,
    saveAllocations,
  };
}
