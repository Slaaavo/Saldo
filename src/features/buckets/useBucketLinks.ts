import { useEffect, useState } from 'react';
import { listLinksForEvent, getLatestBucketLinks } from '../../shared/api';
import type { SnapshotRow } from '../../shared/types';

export interface LinkRow {
  tempId: string;
  sourceAccountId: number | null;
  isNew: boolean;
}

export interface UseBucketLinksReturn {
  loadingLinks: boolean;
  visibleLinks: LinkRow[];
  availableToLink: SnapshotRow[];
  handleSourceAccountSelect: (tempId: string, sourceAccountId: number) => void;
  handleAddLink: () => void;
  handleUnlink: (tempId: string) => void;
  handleRemoveNew: (tempId: string) => void;
  getLinkedAccountIds: () => number[];
}

interface UseBucketLinksParams {
  isBucket: boolean;
  eventId: number | null;
  bucketAccountId?: number | null;
  asOfDate?: string | null;
  allAccounts: SnapshotRow[];
}

export function useBucketLinks({
  isBucket,
  eventId,
  bucketAccountId,
  asOfDate,
  allAccounts,
}: UseBucketLinksParams): UseBucketLinksReturn {
  const [links, setLinks] = useState<LinkRow[]>([]);
  const [loadedForEventId, setLoadedForEventId] = useState<number | null>(null);

  useEffect(() => {
    if (!isBucket || eventId === null) return;

    let cancelled = false;
    listLinksForEvent(eventId)
      .then((bucketLinks) => {
        if (cancelled) return;
        setLinks(
          bucketLinks.map((link) => ({
            tempId: crypto.randomUUID(),
            sourceAccountId: link.sourceAccountId,
            isNew: false,
          })),
        );
        setLoadedForEventId(eventId);
      })
      .catch(() => {
        if (!cancelled) setLoadedForEventId(eventId);
      });
    return () => {
      cancelled = true;
    };
  }, [eventId, isBucket]);

  // Create-mode pre-load: fetch the most recent links for the bucket account
  useEffect(() => {
    if (!isBucket || eventId !== null || !bucketAccountId || !asOfDate) return;

    let cancelled = false;
    getLatestBucketLinks(bucketAccountId, asOfDate)
      .then((bucketLinks) => {
        if (cancelled) return;
        setLinks(
          bucketLinks.map((link) => ({
            tempId: crypto.randomUUID(),
            sourceAccountId: link.sourceAccountId,
            isNew: false,
          })),
        );
      })
      .catch(() => {
        if (!cancelled) setLinks([]);
      });
    return () => {
      cancelled = true;
    };
  }, [isBucket, eventId, bucketAccountId, asOfDate]);

  const handleSourceAccountSelect = (tempId: string, sourceAccountId: number) => {
    setLinks((prev) => prev.map((r) => (r.tempId === tempId ? { ...r, sourceAccountId } : r)));
  };

  const handleAddLink = () => {
    setLinks((prev) => [
      ...prev,
      { tempId: crypto.randomUUID(), sourceAccountId: null, isNew: true },
    ]);
  };

  const handleUnlink = (tempId: string) => {
    setLinks((prev) => prev.filter((r) => r.tempId !== tempId));
  };

  const handleRemoveNew = (tempId: string) => {
    setLinks((prev) => prev.filter((r) => r.tempId !== tempId));
  };

  // Gate all link state on isBucket so no synchronous setState in the effect is needed
  const effectiveLinks = isBucket ? links : [];
  // Derived: loading while we haven't yet received data for the current event
  const loadingLinks = isBucket && eventId !== null && loadedForEventId !== eventId;

  const getLinkedAccountIds = (): number[] =>
    effectiveLinks
      .filter((r) => r.sourceAccountId !== null)
      .map((r) => r.sourceAccountId as number);

  const currentIds = new Set(
    effectiveLinks.filter((r) => r.sourceAccountId !== null).map((r) => r.sourceAccountId!),
  );
  const availableToLink = allAccounts.filter(
    (a) => a.accountType === 'account' && !a.isBucketLinked && !currentIds.has(a.accountId),
  );

  return {
    loadingLinks,
    visibleLinks: effectiveLinks,
    availableToLink,
    handleSourceAccountSelect,
    handleAddLink,
    handleUnlink,
    handleRemoveNew,
    getLinkedAccountIds,
  };
}
