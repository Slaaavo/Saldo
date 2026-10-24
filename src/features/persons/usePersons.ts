import { useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { listPersons, createPerson, updatePerson, deletePerson } from '../../shared/api'
import type { PersonRow } from '../../shared/types'
import { useSelectedPerson } from '../../app/useSelectedPerson'

export const usePersons = () => {
  const queryClient = useQueryClient()
  const { selectedPersonId, setSelectedPersonId } = useSelectedPerson()
  const { data: persons = [], isLoading } = useQuery({ queryKey: ['persons'], queryFn: listPersons })

  const [createDialogOpen, setCreateDialogOpen] = useState(false)
  const [editingPerson, setEditingPerson] = useState<PersonRow | null>(null)
  const [deletingPerson, setDeletingPerson] = useState<PersonRow | null>(null)

  const handleCreatePerson = async (name: string, personType: string, vatPayer: boolean) => {
    await createPerson(name, personType, vatPayer)
    await queryClient.invalidateQueries({ queryKey: ['persons'] })
  }

  const handleUpdatePerson = async (personId: number, name: string, personType: string, vatPayer: boolean) => {
    await updatePerson(personId, name, personType, vatPayer)
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ['persons'] }),
      queryClient.invalidateQueries({ queryKey: ['events'] }),
      queryClient.invalidateQueries({ queryKey: ['snapshot'] }),
    ])
  }

  const handleDeletePerson = async (personId: number) => {
    await deletePerson(personId)
    if (selectedPersonId === personId) setSelectedPersonId(null)
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ['persons'] }),
      queryClient.invalidateQueries({ queryKey: ['events'] }),
      queryClient.invalidateQueries({ queryKey: ['snapshot'] }),
    ])
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
