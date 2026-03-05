import type { SnapshotRow } from '../types';
import { formatEur } from '../utils/format';
import { cn } from '@/lib/utils';
import { Button } from './ui/button';
import { Card, CardContent } from './ui/card';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from './ui/dropdown-menu';
import { MoreVertical, Plus, Pencil, Trash2 } from 'lucide-react';

interface Props {
  snapshot: SnapshotRow[];
  onUpdateBalance: (accountId: number) => void;
  onRenameAccount: (accountId: number, currentName: string) => void;
  onDeleteAccount: (accountId: number, name: string) => void;
  onCreateAccount: () => void;
}

export default function AccountCards({
  snapshot,
  onUpdateBalance,
  onRenameAccount,
  onDeleteAccount,
  onCreateAccount,
}: Props) {
  return (
    <section>
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-semibold">Accounts</h2>
        <Button onClick={onCreateAccount} size="sm">
          <Plus className="h-4 w-4" />
          Add Account
        </Button>
      </div>

      {snapshot.length === 0 ? (
        <p className="text-sm text-muted-foreground">No accounts yet. Create one to get started.</p>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-4">
          {snapshot.map((row) => (
            <Card key={row.accountId} className="relative">
              <CardContent className="flex flex-col gap-1 p-4">
                <div className="flex items-start justify-between">
                  <span className="text-sm text-muted-foreground">{row.accountName}</span>
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button variant="ghost" size="icon" className="h-7 w-7 -mr-2 -mt-1">
                        <MoreVertical className="h-4 w-4" />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                      <DropdownMenuItem
                        onClick={() => onRenameAccount(row.accountId, row.accountName)}
                      >
                        <Pencil className="h-4 w-4" />
                        Rename
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        className="text-destructive focus:text-destructive"
                        onClick={() => onDeleteAccount(row.accountId, row.accountName)}
                      >
                        <Trash2 className="h-4 w-4" />
                        Delete
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
                <span
                  className={cn('text-2xl font-bold', row.balanceMinor < 0 && 'text-destructive')}
                >
                  {formatEur(row.balanceMinor)}
                </span>
                <button
                  onClick={() => onUpdateBalance(row.accountId)}
                  className="mt-2 text-xs font-bold uppercase tracking-wider text-muted-foreground hover:text-primary transition-colors flex items-center gap-1"
                >
                  <Pencil className="h-3 w-3" />
                  Update Balance
                </button>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </section>
  );
}
