import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Button } from '../../shared/ui/button'
import ConfirmDialog from '../../shared/ui/ConfirmDialog'
import { extractErrorMessage } from '../../shared/utils/errors'
import { usePersons } from './usePersons'
import CreatePersonDialog from './CreatePersonDialog'
import EditPersonDialog from './EditPersonDialog'

const PersonsPage = () => {
  const { t } = useTranslation()
  const {
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
  } = usePersons()

  const handleDeleteWithError = async (personId: number) => {
    try {
      await handleDeletePerson(personId)
      setDeletingPerson(null)
    } catch (err) {
      toast.error(extractErrorMessage(err))
    }
  }

  return (
    <div className="px-4 md:px-10 py-8">
      <div className="flex justify-end mb-6">
        <Button onClick={() => setCreateDialogOpen(true)}>{t('persons.addPerson')}</Button>
      </div>

      {/* Table */}
      {isLoading ? null : persons.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('persons.empty')}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm border-collapse">
            <thead>
              <tr className="border-b border-border">
                <th className="text-left py-2 pr-4 font-semibold text-muted-foreground">{t('persons.name')}</th>
                <th className="text-left py-2 pr-4 font-semibold text-muted-foreground">{t('persons.type')}</th>
                <th className="text-right py-2 font-semibold text-muted-foreground" />
              </tr>
            </thead>
            <tbody>
              {persons.map((person) => (
                <tr key={person.id} className="border-b border-border last:border-0 even:bg-muted/50">
                  <td className="py-2 pr-4 font-medium">
                    {person.name}
                    {person.isDefault && <span className="ml-2 text-xs font-normal text-muted-foreground border border-border rounded px-1">{t('persons.defaultBadge')}</span>}
                  </td>
                  <td className="py-2 pr-4 text-muted-foreground">{person.personType === 'physical' ? t('persons.typePhysical') : t('persons.typeLegal')}</td>
                  <td className="py-2 text-right">
                    <div className="flex gap-2 justify-end">
                      <Button variant="ghost" size="sm" onClick={() => setEditingPerson(person)}>
                        {t('accounts.edit')}
                      </Button>
                      <Button variant="ghost" size="sm" className="text-destructive hover:text-destructive" onClick={() => setDeletingPerson(person)} disabled={person.isDefault}>
                        {t('accounts.delete')}
                      </Button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Dialogs */}
      <CreatePersonDialog
        open={createDialogOpen}
        onOpenChange={setCreateDialogOpen}
        onSubmit={async (name, personType) => {
          await handleCreatePerson(name, personType)
          setCreateDialogOpen(false)
        }}
      />
      {editingPerson && (
        <EditPersonDialog
          open={true}
          onOpenChange={(v) => {
            if (!v) setEditingPerson(null)
          }}
          person={editingPerson}
          onSubmit={async (personId, name, personType) => {
            await handleUpdatePerson(personId, name, personType)
            setEditingPerson(null)
          }}
        />
      )}
      {deletingPerson && (
        <ConfirmDialog
          message={t('persons.deleteConfirm', { name: deletingPerson.name })}
          onConfirm={() => handleDeleteWithError(deletingPerson.id)}
          onCancel={() => setDeletingPerson(null)}
        />
      )}
    </div>
  )
}

export default PersonsPage
