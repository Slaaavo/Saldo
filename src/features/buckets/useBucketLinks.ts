import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { listLinksForEvent, getLatestBucketLinks } from '../../shared/api'
import { extractErrorMessage } from '../../shared/utils/errors'
import type { SnapshotRow } from '../../shared/types'

export interface LinkRow {
  tempId: string
  sourceAccountId: number | null
  isNew: boolean
}

export interface UseBucketLinksReturn {
  loadingLinks: boolean
  visibleLinks: LinkRow[]
  availableToLink: SnapshotRow[]
  handleSourceAccountSelect: (tempId: string, sourceAccountId: number) => void
  handleAddLink: () => void
  handleUnlink: (tempId: string) => void
  handleRemoveNew: (tempId: string) => void
  getLinkedAccountIds: () => number[]
}

interface UseBucketLinksParams {
  isBucket: boolean
  eventId: number | null
  bucketAccountId?: number | null
  asOfDate?: string | null
  allAccounts: SnapshotRow[]
}

export const useBucketLinks = ({ isBucket, eventId, bucketAccountId, asOfDate, allAccounts }: UseBucketLinksParams): UseBucketLinksReturn => {
  const [links, setLinks] = useState<LinkRow[]>([])
  const [loadedForEventId, setLoadedForEventId] = useState<number | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)

  useEffect(() => {
    if (!isBucket || eventId === null) return

    let cancelled = false
    listLinksForEvent(eventId)
      .then((bucketLinks) => {
        if (cancelled) return
        setLinks(
          bucketLinks.map((link) => ({
            tempId: crypto.randomUUID(),
            sourceAccountId: link.sourceAccountId,
            isNew: false,
          })),
        )
        setLoadError(null)
        setLoadedForEventId(eventId)
      })
      .catch((err) => {
        if (cancelled) return
        const message = extractErrorMessage(err)
        toast.error(message)
        setLoadError(message)
        setLoadedForEventId(eventId)
      })
    return () => {
      cancelled = true
    }
  }, [eventId, isBucket])

  // Create-mode pre-load: fetch the most recent links for the bucket account
  useEffect(() => {
    if (!isBucket || eventId !== null || !bucketAccountId || !asOfDate) return

    let cancelled = false
    getLatestBucketLinks(bucketAccountId, asOfDate)
      .then((bucketLinks) => {
        if (cancelled) return
        setLinks(
          bucketLinks.map((link) => ({
            tempId: crypto.randomUUID(),
            sourceAccountId: link.sourceAccountId,
            isNew: false,
          })),
        )
        setLoadError(null)
      })
      .catch((err) => {
        if (cancelled) return
        const message = extractErrorMessage(err)
        toast.error(message)
        setLoadError(message)
        setLinks([])
      })
    return () => {
      cancelled = true
    }
  }, [isBucket, eventId, bucketAccountId, asOfDate])

  const handleSourceAccountSelect = (tempId: string, sourceAccountId: number) => {
    setLinks((prev) => prev.map((r) => (r.tempId === tempId ? { ...r, sourceAccountId } : r)))
  }

  const handleAddLink = () => {
    setLinks((prev) => [...prev, { tempId: crypto.randomUUID(), sourceAccountId: null, isNew: true }])
  }

  const handleUnlink = (tempId: string) => {
    setLinks((prev) => prev.filter((r) => r.tempId !== tempId))
  }

  const handleRemoveNew = (tempId: string) => {
    setLinks((prev) => prev.filter((r) => r.tempId !== tempId))
  }

  // Gate all link state on isBucket so no synchronous setState in the effect is needed
  const effectiveLinks = isBucket ? links : []
  // Derived: loading while we haven't yet received data for the current event
  const loadingLinks = isBucket && eventId !== null && loadedForEventId !== eventId

  // Throwing makes the modal's submit catch toast it, instead of saving an empty set over links that never loaded
  const getLinkedAccountIds = (): number[] => {
    if (isBucket && loadError !== null) throw new Error(loadError)
    return effectiveLinks.filter((r) => r.sourceAccountId !== null).map((r) => r.sourceAccountId as number)
  }

  const currentIds = new Set(effectiveLinks.filter((r) => r.sourceAccountId !== null).map((r) => r.sourceAccountId!))
  const availableToLink = allAccounts.filter((a) => a.accountType === 'account' && !a.isBucketLinked && !currentIds.has(a.accountId))

  return {
    loadingLinks,
    visibleLinks: effectiveLinks,
    availableToLink,
    handleSourceAccountSelect,
    handleAddLink,
    handleUnlink,
    handleRemoveNew,
    getLinkedAccountIds,
  }
}
