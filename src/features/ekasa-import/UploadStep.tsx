import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { open } from '@tauri-apps/plugin-dialog'
import { cn } from '../../shared/lib/utils'
import { Button } from '../../shared/ui/button'
import { DialogFooter } from '../../shared/ui/dialog'

interface Props {
  onFileSelect: (filePath: string) => void
}

const UploadStep = ({ onFileSelect }: Props) => {
  const { t } = useTranslation()
  const [isDragOver, setIsDragOver] = useState(false)
  const [selectedFileName, setSelectedFileName] = useState<string | null>(null)
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(null)

  const handleBrowse = async () => {
    const result = await open({
      multiple: false,
      filters: [{ name: t('ekasaImport.uploadStep.filterLabel'), extensions: ['pdf', 'png', 'jpg', 'jpeg'] }],
    })
    if (typeof result === 'string') {
      const fileName = result.split(/[\\/]/).pop() ?? result
      setSelectedFileName(fileName)
      setSelectedFilePath(result)
    }
  }

  const handleDragOver = (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault()
    setIsDragOver(true)
  }

  const handleDragLeave = () => {
    setIsDragOver(false)
  }

  const handleDrop = (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault()
    setIsDragOver(false)
    toast.info(t('ekasaImport.uploadStep.dragNotSupported'))
  }

  const handleNext = () => {
    if (selectedFilePath !== null) {
      onFileSelect(selectedFilePath)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <div
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        className={cn(
          'flex flex-col items-center justify-center rounded-[var(--radius)] border-2 border-dashed p-8 text-center transition-colors',
          isDragOver ? 'border-primary bg-primary/5' : 'border-border bg-muted/20 hover:border-primary/50',
        )}
      >
        {selectedFileName ? (
          <p className="text-sm font-medium">{t('ekasaImport.uploadStep.selected', { name: selectedFileName })}</p>
        ) : (
          <>
            <p className="text-sm text-muted-foreground">{t('ekasaImport.uploadStep.dragDrop')}</p>
            <p className="mt-1 text-xs text-muted-foreground">{t('ekasaImport.uploadStep.instructions')}</p>
          </>
        )}
        <Button type="button" variant="outline" size="sm" className="mt-3" onClick={handleBrowse}>
          {t('ekasaImport.uploadStep.browse')}
        </Button>
      </div>
      <DialogFooter>
        <Button type="button" onClick={handleNext} disabled={selectedFilePath === null}>
          {t('ekasaImport.rulesStep.next')}
        </Button>
      </DialogFooter>
    </div>
  )
}

export default UploadStep
