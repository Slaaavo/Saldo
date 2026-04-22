import { useTranslation } from 'react-i18next'
import { Button } from '@/shared/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/shared/ui/dropdown-menu'
import { Link2, MoreVertical, Pencil, Trash2 } from 'lucide-react'

interface AccountContextMenuProps {
  accountId: number
  accountName: string
  accountType: string
  onRenameAccount: (accountId: number, accountName: string) => void
  onDeleteAccount: (accountId: number, accountName: string) => void
  onManageLinkedAssets?: (accountId: number, accountName: string) => void
}

export const AccountContextMenu = ({ accountId, accountName, accountType, onRenameAccount, onDeleteAccount, onManageLinkedAssets }: AccountContextMenuProps) => {
  const { t } = useTranslation()

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" className="h-7 w-7 -mr-2 -mt-1">
          <MoreVertical className="h-4 w-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onClick={() => onRenameAccount(accountId, accountName)}>
          <Pencil className="h-4 w-4" />
          {t('accounts.edit')}
        </DropdownMenuItem>
        {onManageLinkedAssets && accountType === 'account' && (
          <DropdownMenuItem onClick={() => onManageLinkedAssets(accountId, accountName)}>
            <Link2 className="h-4 w-4" />
            {t('accounts.manageLinkedAssets')}
          </DropdownMenuItem>
        )}
        <DropdownMenuItem className="text-destructive focus:text-destructive" onClick={() => onDeleteAccount(accountId, accountName)}>
          <Trash2 className="h-4 w-4" />
          {t('accounts.delete')}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
