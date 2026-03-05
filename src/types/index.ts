export interface Currency {
  id: number;
  code: string;
  name: string;
  minorUnits: number;
}

export interface Account {
  id: number;
  name: string;
  currencyId: number;
  createdAt: string;
}

export interface EventWithData {
  id: number;
  accountId: number;
  accountName: string;
  accountType: string;
  eventType: string;
  eventDate: string;
  amountMinor: number;
  note: string | null;
  createdAt: string;
}

export interface SnapshotRow {
  accountId: number;
  accountName: string;
  accountType: string;
  balanceMinor: number;
}

export interface AppError {
  code: string;
  message: string;
}
