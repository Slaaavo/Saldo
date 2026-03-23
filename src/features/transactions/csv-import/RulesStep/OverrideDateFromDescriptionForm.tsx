import { useTranslation } from 'react-i18next';
import type { ImportRule } from '../../../../shared/types';
import { Input } from '../../../../shared/ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '../../../../shared/ui/select';

interface OverrideDateFromDescriptionFormProps {
  rule: ImportRule & { type: 'override_date_from_description' };
  csvHeaders: string[];
  onChange: (rule: ImportRule) => void;
  errors?: { conditionRegex?: string; dateRegex?: string };
}

export default function OverrideDateFromDescriptionForm({
  rule,
  csvHeaders,
  onChange,
  errors,
}: OverrideDateFromDescriptionFormProps) {
  const { t } = useTranslation();

  const descColumnMissing =
    rule.descriptionColumn !== '' && !csvHeaders.includes(rule.descriptionColumn);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">
          {t('import.rules.overrideDateFromDescription.descriptionColumn')}
        </span>
        <Select
          value={rule.descriptionColumn !== '' ? rule.descriptionColumn : '__none__'}
          onValueChange={(value) =>
            onChange({
              ...rule,
              descriptionColumn: value === '__none__' ? '' : value,
            })
          }
        >
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__none__">—</SelectItem>
            {csvHeaders.map((h) => (
              <SelectItem key={h} value={h}>
                {h}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {descColumnMissing && (
          <p className="text-xs text-amber-600 dark:text-amber-400">
            {t('import.rules.unmatchedColumn', { name: rule.descriptionColumn })}
          </p>
        )}
      </div>

      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">
          {t('import.rules.overrideDateFromDescription.conditionRegex')}
        </span>
        <Input
          value={rule.conditionRegex}
          onChange={(e) => onChange({ ...rule, conditionRegex: e.target.value })}
        />
        <p className="text-xs text-muted-foreground">
          {t('import.rules.overrideDateFromDescription.conditionExample')}
        </p>
        {errors?.conditionRegex && (
          <p className="text-xs text-destructive">{errors.conditionRegex}</p>
        )}
      </div>

      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">
          {t('import.rules.overrideDateFromDescription.dateRegex')}
        </span>
        <Input
          value={rule.dateRegex}
          onChange={(e) => onChange({ ...rule, dateRegex: e.target.value })}
        />
        <p className="text-xs text-muted-foreground">
          {t('import.rules.overrideDateFromDescription.dateExample')}
        </p>
        {errors?.dateRegex && <p className="text-xs text-destructive">{errors.dateRegex}</p>}
      </div>

      <details className="text-sm">
        <summary className="cursor-pointer select-none text-muted-foreground hover:text-foreground">
          {t('import.rules.overrideDateFromDescription.regexHint')}
        </summary>
        <table className="mt-2 w-full text-xs border-collapse">
          <thead>
            <tr className="text-left border-b">
              <th className="py-1 pr-4 font-medium">Pattern</th>
              <th className="py-1 pr-4 font-medium">Meaning</th>
              <th className="py-1 font-medium">Example</th>
            </tr>
          </thead>
          <tbody className="text-muted-foreground">
            <tr className="border-b border-border/40">
              <td className="py-1 pr-4 font-mono">\d</td>
              <td className="py-1 pr-4">Any digit (0–9)</td>
              <td className="py-1 font-mono">\d\d matches 42</td>
            </tr>
            <tr className="border-b border-border/40">
              <td className="py-1 pr-4 font-mono">\d+</td>
              <td className="py-1 pr-4">One or more digits</td>
              <td className="py-1 font-mono">\d+ matches 123</td>
            </tr>
            <tr className="border-b border-border/40">
              <td className="py-1 pr-4 font-mono">\.</td>
              <td className="py-1 pr-4">A literal dot</td>
              <td className="py-1 font-mono">
                \d{'{2}'}\.d{'{2}'} matches 15.03
              </td>
            </tr>
            <tr className="border-b border-border/40">
              <td className="py-1 pr-4 font-mono">{'{\\d{N}}'}</td>
              <td className="py-1 pr-4">Exactly N digits</td>
              <td className="py-1 font-mono">\d{'{4}'} matches 2026</td>
            </tr>
            <tr className="border-b border-border/40">
              <td className="py-1 pr-4 font-mono">{'(...)'}</td>
              <td className="py-1 pr-4">Capture group — the date to extract</td>
              <td className="py-1 font-mono">{'(\\d{2}\\.\\d{2}\\.\\d{4})'} captures 15.03.2026</td>
            </tr>
            <tr className="border-b border-border/40">
              <td className="py-1 pr-4 font-mono">.*</td>
              <td className="py-1 pr-4">Any characters</td>
              <td className="py-1 font-mono">
                .*{'(\\d{2}\\.\\d{2}\\.\\d{4})'} finds a date anywhere
              </td>
            </tr>
            <tr className="border-b border-border/40">
              <td className="py-1 pr-4 font-mono">^</td>
              <td className="py-1 pr-4">Start of text</td>
              <td className="py-1 font-mono">^1234 matches only if starts with 1234</td>
            </tr>
            <tr>
              <td className="py-1 pr-4 font-mono">\*</td>
              <td className="py-1 pr-4">A literal asterisk</td>
              <td className="py-1 font-mono">
                \d{'{4}'}\*{'{4}'}\d{'{4}'} matches 1234****5678
              </td>
            </tr>
          </tbody>
        </table>
      </details>
    </div>
  );
}
