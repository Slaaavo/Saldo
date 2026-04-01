import { useState, useEffect, useCallback } from 'react'
import type { PartnerAccount } from '../../shared/types'
import { listPartnerAccounts, createPartnerAccount, updatePartnerAccount, deletePartnerAccount, getConsolidationCurrency } from '../../shared/api'
import { extractErrorMessage } from '../../shared/utils/errors'

export function usePartners() {
  const [partners, setPartners] = useState<PartnerAccount[]>([])
  const [loading, setLoading] = useState(true)
  const [searchQuery, setSearchQuery] = useState('')
  const [consolidationCurrencyId, setConsolidationCurrencyId] = useState<number | null>(null)

  // Modal state
  const [createModalOpen, setCreateModalOpen] = useState(false)
  const [editingPartner, setEditingPartner] = useState<PartnerAccount | null>(null)
  const [deletingPartner, setDeletingPartner] = useState<PartnerAccount | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)

  const loadPartners = useCallback(async () => {
    try {
      const data = await listPartnerAccounts()
      setPartners(data)
    } catch (err) {
      console.error('Failed to load partners:', extractErrorMessage(err))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    loadPartners()
    getConsolidationCurrency()
      .then((c) => setConsolidationCurrencyId(c.id))
      .catch(() => {})
  }, [loadPartners])

  const filteredPartners = partners.filter((p) => p.name.toLowerCase().includes(searchQuery.toLowerCase()) || (p.iban ?? '').toLowerCase().includes(searchQuery.toLowerCase()))

  const handleCreate = async (name: string, iban: string, currencyId: number) => {
    setActionError(null)
    await createPartnerAccount(name, currencyId, iban)
    setCreateModalOpen(false)
    await loadPartners()
  }

  const handleUpdate = async (accountId: number, name: string, iban: string) => {
    setActionError(null)
    await updatePartnerAccount(accountId, name, iban)
    setEditingPartner(null)
    await loadPartners()
  }

  const handleDelete = async (accountId: number) => {
    setActionError(null)
    await deletePartnerAccount(accountId)
    setDeletingPartner(null)
    await loadPartners()
  }

  return {
    partners,
    filteredPartners,
    loading,
    searchQuery,
    setSearchQuery,
    consolidationCurrencyId,
    createModalOpen,
    setCreateModalOpen,
    editingPartner,
    setEditingPartner,
    deletingPartner,
    setDeletingPartner,
    actionError,
    setActionError,
    handleCreate,
    handleUpdate,
    handleDelete,
  }
}
