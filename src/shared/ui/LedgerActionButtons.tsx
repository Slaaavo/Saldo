import { useTranslation } from 'react-i18next'
import { Button } from './button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from './dropdown-menu'
import { Plus, Upload } from 'lucide-react'

interface Props {
  onAddEvents: () => void
  onImportCsv: () => void
  onImportEkasa: () => void
}

const LedgerActionButtons = ({ onAddEvents, onImportCsv, onImportEkasa }: Props) => {
  const { t } = useTranslation()

  return (
    <div className="flex items-center gap-2">
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button size="sm" variant="outline">
            <Upload className="h-4 w-4" />
            {t('import.dropdownTrigger')}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onClick={onImportCsv}>{t('import.csvOption')}</DropdownMenuItem>
          <DropdownMenuItem onClick={onImportEkasa}>{t('import.ekasaReceiptOption')}</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <Button onClick={onAddEvents} size="sm">
        <Plus className="h-4 w-4" />
        {t('ledger.addEvents')}
      </Button>
    </div>
  )
}

export default LedgerActionButtons
