import { useState, useEffect, useCallback } from 'react'
import { toast } from 'sonner'
import type { ImportProfileRow } from '../../shared/types'
import { listImportProfiles, deleteImportProfile } from '../../shared/api'
import { extractErrorMessage } from '../../shared/utils/errors'

export function useImportProfiles() {
  const [profiles, setProfiles] = useState<ImportProfileRow[]>([])
  const [loading, setLoading] = useState(true)
  const [deletingProfile, setDeletingProfile] = useState<ImportProfileRow | null>(null)

  const load = useCallback(async () => {
    try {
      const data = await listImportProfiles()
      setProfiles(data)
    } catch (err) {
      toast.error(String(err))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    load()
  }, [load])

  const handleDelete = async (profileId: number) => {
    try {
      await deleteImportProfile(profileId)
      setDeletingProfile(null)
      await load()
    } catch (err) {
      toast.error(extractErrorMessage(err))
    }
  }

  return {
    profiles,
    loading,
    deletingProfile,
    setDeletingProfile,
    load,
    handleDelete,
  }
}
