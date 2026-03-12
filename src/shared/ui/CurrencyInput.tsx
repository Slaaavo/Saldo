import * as React from 'react';
import { Input } from './input';

interface CurrencyInputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  currencyCode?: string;
}

const CurrencyInput = React.forwardRef<HTMLInputElement, CurrencyInputProps>(
  ({ currencyCode, className, ...props }, ref) => {
    if (!currencyCode) {
      return <Input className={className} ref={ref} {...props} />;
    }
    return (
      <div className="relative">
        <Input className={`pr-14${className ? ` ${className}` : ''}`} ref={ref} {...props} />
        <span className="absolute right-3 top-1/2 -translate-y-1/2 text-sm text-muted-foreground pointer-events-none">
          {currencyCode}
        </span>
      </div>
    );
  },
);
CurrencyInput.displayName = 'CurrencyInput';

export { CurrencyInput };
