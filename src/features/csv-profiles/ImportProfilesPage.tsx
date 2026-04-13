import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Trash2 } from 'lucide-react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { Button } from '../../shared/ui/button'
import ConfirmDialog from '../../shared/ui/ConfirmDialog'
import { useImportProfiles } from './useImportProfiles'
import { listEkasaProfiles, deleteEkasaProfile, listPersons } from '../../shared/api'
import type { EkasaImportProfile } from '../../shared/api'
import { extractErrorMessage } from '../../shared/utils/errors'
import { bpsToPct } from '../../shared/utils/format'

const countMappedColumns = (columnMappingJson: string): number => {
  try {
    const m = JSON.parse(columnMappingJson) as Record<string, unknown>
    return Object.values(m).filter(Boolean).length
  } catch {
    return 0
  }
}

const ImportProfilesPage = () => {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const { profiles, loading, deletingProfile, setDeletingProfile, handleDelete } = useImportProfiles()
  const [deletingEkasaProfile, setDeletingEkasaProfile] = useState<EkasaImportProfile | null>(null)

  const { data: ekasaProfiles = [], isLoading: ekasaLoading } = useQuery({
    queryKey: ['ekasaProfiles'],
    queryFn: listEkasaProfiles,
  })

  const { data: persons = [] } = useQuery({
    queryKey: ['persons'],
    queryFn: listPersons,
  })

  const resolvePersonName = (personId: number): string => persons.find((p) => p.id === personId)?.name ?? String(personId)

  const handleDeleteEkasa = async (profileId: number) => {
    try {
      await deleteEkasaProfile(profileId)
      setDeletingEkasaProfile(null)
      await queryClient.invalidateQueries({ queryKey: ['ekasaProfiles'] })
    } catch (err) {
      toast.error(extractErrorMessage(err))
    }
  }

  return (
    <div className="px-4 md:px-10 py-8">
      <h2 className="text-base font-semibold mb-3">{t('import.profiles.csvSectionTitle')}</h2>

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

      <h2 className="text-base font-semibold mt-8 mb-3">{t('import.ekasaProfiles.title')}</h2>

      {ekasaLoading ? null : ekasaProfiles.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('import.ekasaProfiles.empty')}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm border-collapse">
            <thead>
              <tr className="border-b border-border">
                <th className="text-left py-2 pr-4 font-semibold text-muted-foreground">{t('import.ekasaProfiles.person')}</th>
                <th className="text-left py-2 pr-4 font-semibold text-muted-foreground">{t('import.ekasaProfiles.rules')}</th>
                <th className="text-left py-2 pr-4 font-semibold text-muted-foreground">{t('import.ekasaProfiles.defaultDeductible')}</th>
                <th className="text-left py-2 pr-4 font-semibold text-muted-foreground">{t('import.ekasaProfiles.defaultVatReclaimable')}</th>
                <th className="text-right py-2 font-semibold text-muted-foreground" />
              </tr>
            </thead>
            <tbody>
              {ekasaProfiles.map((profile) => (
                <>
                  <tr key={profile.id} className="border-b border-border last:border-0 even:bg-muted/50">
                    <td className="py-2 pr-4 font-medium">{resolvePersonName(profile.personId)}</td>
                    <td className="py-2 pr-4">{profile.rules.length}</td>
                    <td className="py-2 pr-4">{bpsToPct(profile.defaultDeductiblePctBps)}%</td>
                    <td className="py-2 pr-4">{bpsToPct(profile.defaultVatReclaimablePctBps)}%</td>
                    <td className="py-2 text-right">
                      <Button variant="ghost" size="sm" className="text-destructive hover:text-destructive" onClick={() => setDeletingEkasaProfile(profile)}>
                        <Trash2 className="h-4 w-4 mr-1" />
                        {t('import.ekasaProfiles.delete')}
                      </Button>
                    </td>
                  </tr>
                  {profile.rules.length > 0 && (
                    <tr key={`${profile.id}-rules`} className="border-b border-border last:border-0">
                      <td colSpan={5} className="py-1 pl-8 pr-4">
                        <div className="flex flex-col gap-1 text-xs text-muted-foreground">
                          {profile.rules.map((rule) => (
                            <div key={rule.id} className="flex gap-4">
                              <span className="font-mono">{rule.namePattern}</span>
                              <span>
                                {t('ekasaImport.rulesStep.deductiblePctHeader')}: {bpsToPct(rule.deductiblePctBps)}%
                              </span>
                              <span>
                                {t('ekasaImport.rulesStep.vatReclaimablePctHeader')}: {bpsToPct(rule.vatReclaimablePctBps)}%
                              </span>
                            </div>
                          ))}
                        </div>
                      </td>
                    </tr>
                  )}
                </>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {deletingProfile && (
        <ConfirmDialog
          message={t('import.profiles.deleteConfirm', { name: deletingProfile.name })}
          onConfirm={() => {
            void handleDelete(deletingProfile.id)
          }}
          onCancel={() => setDeletingProfile(null)}
        />
      )}

      {deletingEkasaProfile && (
        <ConfirmDialog
          message={t('import.ekasaProfiles.deleteConfirm', { name: resolvePersonName(deletingEkasaProfile.personId) })}
          onConfirm={() => {
            void handleDeleteEkasa(deletingEkasaProfile.id)
          }}
          onCancel={() => setDeletingEkasaProfile(null)}
        />
      )}
    </div>
  )
}

export default ImportProfilesPage
