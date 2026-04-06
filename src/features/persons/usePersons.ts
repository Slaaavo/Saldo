import { useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { listPersons, createPerson, updatePerson, deletePerson } from '../../shared/api'
import type { PersonRow } from '../../shared/types'

export const usePersons = () => {
  const queryClient = useQueryClient()
  const { data: persons = [], isLoading } = useQuery({ queryKey: ['persons'], queryFn: listPersons })

  const [createDialogOpen, setCreateDialogOpen] = useState(false)
  const [editingPerson, setEditingPerson] = useState<PersonRow | null>(null)
  const [deletingPerson, setDeletingPerson] = useState<PersonRow | null>(null)

  const handleCreatePerson = async (name: string, personType: string) => {
    await createPerson(name, personType)
    await queryClient.invalidateQueries({ queryKey: ['persons'] })
  }

  const handleUpdatePerson = async (personId: number, name: string, personType: string) => {
    await updatePerson(personId, name, personType)
    await queryClient.invalidateQueries({ queryKey: ['persons'] })
  }

  const handleDeletePerson = async (personId: number) => {
    await deletePerson(personId)
    await queryClient.invalidateQueries({ queryKey: ['persons'] })
  }

  return {
    persons,
    isLoading,
    createDialogOpen,
    setCreateDialogOpen,
    editingPerson,
    setEditingPerson,
    deletingPerson,
    setDeletingPerson,
    handleCreatePerson,
    handleUpdatePerson,
    handleDeletePerson,
  }
}
