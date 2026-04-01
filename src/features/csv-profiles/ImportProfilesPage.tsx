import { useTranslation } from 'react-i18next'
import { Trash2 } from 'lucide-react'
import { Button } from '../../shared/ui/button'
import ConfirmDialog from '../../shared/ui/ConfirmDialog'
import { useImportProfiles } from './useImportProfiles'

function countMappedColumns(columnMappingJson: string): number {
  try {
    const m = JSON.parse(columnMappingJson) as Record<string, unknown>
    return Object.values(m).filter(Boolean).length
  } catch {
    return 0
  }
}

export default function ImportProfilesPage() {
  const { t } = useTranslation()
  const { profiles, loading, deletingProfile, setDeletingProfile, handleDelete } = useImportProfiles()

  return (
    <div className="px-4 md:px-10 py-8">
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-2xl font-bold">{t('import.profiles.title')}</h2>
      </div>

      {loading ? null : profiles.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('import.profiles.empty')}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm border-collapse">
            <thead>
              <tr className="border-b border-border">
                <th className="text-left py-2 pr-4 font-semibold text-muted-foreground">{t('import.uploadStep.profile')}</th>
                <th className="text-left py-2 pr-4 font-semibold text-muted-foreground">{t('import.profiles.columns')}</th>
                <th className="text-left py-2 pr-4 font-semibold text-muted-foreground">{t('import.profiles.rules')}</th>
                <th className="text-right py-2 font-semibold text-muted-foreground" />
              </tr>
            </thead>
            <tbody>
              {profiles.map((profile) => (
                <tr key={profile.id} className="border-b border-border last:border-0 even:bg-muted/50">
                  <td className="py-2 pr-4 font-medium">{profile.name}</td>
                  <td className="py-2 pr-4">{countMappedColumns(profile.columnMappingJson)}</td>
                  <td className="py-2 pr-4">{profile.rules.length}</td>
                  <td className="py-2 text-right">
                    <Button variant="ghost" size="sm" className="text-destructive hover:text-destructive" onClick={() => setDeletingProfile(profile)}>
                      <Trash2 className="h-4 w-4 mr-1" />
                      {t('import.profiles.delete')}
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <p className="text-sm text-muted-foreground mt-4">{t('import.profiles.description')}</p>

      {deletingProfile && (
        <ConfirmDialog
          message={t('import.profiles.deleteConfirm', { name: deletingProfile.name })}
          onConfirm={() => {
            void handleDelete(deletingProfile.id)
          }}
          onCancel={() => setDeletingProfile(null)}
        />
      )}
    </div>
  )
}
