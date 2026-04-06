import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { listPersons } from '../../shared/api'
import { useSelectedPerson } from '../../app/useSelectedPerson'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../shared/ui/select'

const PersonSelector = () => {
  const { t } = useTranslation()
  const { data: persons } = useQuery({ queryKey: ['persons'], queryFn: listPersons })
  const { selectedPersonId, setSelectedPersonId } = useSelectedPerson()

  if (!persons || persons.length <= 1) return null

  return (
    <Select value={selectedPersonId === null ? 'all' : String(selectedPersonId)} onValueChange={(val) => setSelectedPersonId(val === 'all' ? null : Number(val))}>
      <SelectTrigger className="w-[160px]">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="all">{t('persons.all')}</SelectItem>
        {persons.map((p) => (
          <SelectItem key={p.id} value={String(p.id)}>
            {p.name}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}

export default PersonSelector
