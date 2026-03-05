export interface NumberFormatConfig {
  currencySymbol: string;
  currencyPosition: 'left' | 'right';
  thousandsSeparator: string;
  decimalSeparator: string;
}

export const defaultNumberFormat: NumberFormatConfig = {
  currencySymbol: '€',
  currencyPosition: 'right',
  thousandsSeparator: ' ',
  decimalSeparator: '.',
};
