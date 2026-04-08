import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { listPersons } from '../../shared/api'
import { useSelectedPerson } from '../../app/useSelectedPerson'
import type { PersonRow } from '../../shared/types'

export interface ResolvedPersonState {
  personId: number | null
  resolvedPersonId: number | null
  localPersonId: number | null
  setLocalPersonId: (id: number) => void
  persons: PersonRow[]
  showPicker: boolean
}

export const useResolvedPersonId = (): ResolvedPersonState => {
  const { selectedPersonId } = useSelectedPerson()

  const personsQuery = useQuery({ queryKey: ['persons'], queryFn: listPersons })
  const persons = personsQuery.data ?? []

  // Default to the first legal person sorted by createdAt; fall back to physical if none
  const firstLegalPerson = [...persons].filter((p) => p.personType === 'legal').sort((a, b) => a.createdAt.localeCompare(b.createdAt))[0] ?? null
  const defaultPersonId = firstLegalPerson?.id ?? null
  const resolvedPersonId = selectedPersonId ?? defaultPersonId

  const [localPersonId, setLocalPersonId] = useState<number | null>(null)
  const personId = localPersonId ?? resolvedPersonId

  // Show the picker when no person is forced by the header and there are multiple persons
  const showPicker = selectedPersonId === null && persons.length > 1

  return { personId, resolvedPersonId, localPersonId, setLocalPersonId, persons, showPicker }
}
