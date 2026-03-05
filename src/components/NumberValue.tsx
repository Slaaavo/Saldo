import type { HTMLAttributes } from 'react';
import { formatAmount } from '../utils/format';
import type { NumberFormatConfig } from '../config/numberFormat';
import { defaultNumberFormat } from '../config/numberFormat';

interface NumberValueProps extends HTMLAttributes<HTMLSpanElement> {
  value: number;
  minorUnits?: number;
  config?: NumberFormatConfig;
}

export default function NumberValue({
  value,
  minorUnits = 2,
  config = defaultNumberFormat,
  className,
  ...rest
}: NumberValueProps) {
  return (
    <span className={className} {...rest}>
      {formatAmount(value, minorUnits, config)}
    </span>
  );
}
