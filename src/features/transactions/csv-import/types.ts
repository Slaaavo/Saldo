export type CsvRow = Record<string, string>;

export type CashflowFieldKey = 'date' | 'amount' | 'partner' | 'currency' | 'fxRate' | 'note';

export type ColumnMapping = Record<CashflowFieldKey, string | null>;

export type IbanMatchResult =
  | { type: 'ownAccount'; accountId: number; accountName: string }
  | { type: 'partner'; accountId: number; accountName: string }
  | { type: 'unmatched'; rawIban: string }
  | { type: 'none' };

export interface SplitLeg {
  legIndex: number;
  amountMinor: number;
  note: string | null;
  rawIban: string | null;
  ibanMatch: IbanMatchResult;
  bucketId: number | null;
  counterpartAccountId: number | null;
}

export interface ImportRow {
  index: number;
  date: string;
  amountMinor: number;
  currencyCode: string;
  originalAmountMinor: number | null;
  originalCurrencyCode: string | null;
  fxRateMantissa: number | null;
  fxRateExponent: number | null;
  note: string | null;
  rawIban: string | null;
  ibanMatch: IbanMatchResult;
  isDuplicate: boolean;
  nearDateDuplicateEventId: number | null;
  isSelected: boolean;
  bucketId: number | null;
  counterpartAccountId: number | null;
  splitLegs: SplitLeg[] | null;
}

export type WizardStep = 'upload' | 'mapping' | 'review';

export interface WizardState {
  step: WizardStep;
  file: File | null;
  csvHeaders: string[];
  csvRows: CsvRow[];
  selectedAccountId: number | null;
  columnMapping: ColumnMapping;
  importRows: ImportRow[];
  importing: boolean;
}
