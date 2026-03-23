import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Trash2, ChevronUp, ChevronDown } from 'lucide-react';
import type { ImportRule } from '../../../../shared/types';
import { Button } from '../../../../shared/ui/button';
import { DialogFooter } from '../../../../shared/ui/dialog';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '../../../../shared/ui/select';
import SignFromColumnForm from './SignFromColumnForm';
import OverrideDateFromDescriptionForm from './OverrideDateFromDescriptionForm';

const ADD_NONE = '__none__';

interface ValidationErrors {
  [ruleIndex: number]: {
    conditionRegex?: string;
    dateRegex?: string;
  };
}

interface RulesStepProps {
  rules: ImportRule[];
  csvHeaders: string[];
  onRulesChange: (rules: ImportRule[]) => void;
  onNext: () => void;
  onBack: () => void;
  onCancel: () => void;
}

export default function RulesStep({
  rules,
  csvHeaders,
  onRulesChange,
  onNext,
  onBack,
  onCancel,
}: RulesStepProps) {
  const { t } = useTranslation();
  const [addType, setAddType] = useState<string>(ADD_NONE);
  const [validationErrors, setValidationErrors] = useState<ValidationErrors>({});

  const handleDelete = (index: number) => {
    onRulesChange(rules.filter((_, i) => i !== index));
  };

  const handleMoveUp = (index: number) => {
    if (index === 0) return;
    const next = [...rules];
    [next[index - 1], next[index]] = [next[index], next[index - 1]];
    onRulesChange(next);
  };

  const handleMoveDown = (index: number) => {
    if (index === rules.length - 1) return;
    const next = [...rules];
    [next[index], next[index + 1]] = [next[index + 1], next[index]];
    onRulesChange(next);
  };

  const handleRuleChange = (index: number, updated: ImportRule) => {
    const next = [...rules];
    next[index] = updated;
    onRulesChange(next);
    // clear errors for this index when rule changes
    if (validationErrors[index]) {
      setValidationErrors((prev) => {
        const copy = { ...prev };
        delete copy[index];
        return copy;
      });
    }
  };

  const handleAdd = () => {
    if (addType === ADD_NONE) return;
    let newRule: ImportRule;
    if (addType === 'sign_from_column') {
      newRule = {
        type: 'sign_from_column',
        sortOrder: rules.length,
        typeColumn: '',
        negativeType: '',
      };
    } else {
      newRule = {
        type: 'override_date_from_description',
        sortOrder: rules.length,
        descriptionColumn: '',
        conditionRegex: '',
        dateRegex: '',
      };
    }
    onRulesChange([...rules, newRule]);
  };

  const handleNext = () => {
    const errors: ValidationErrors = {};

    rules.forEach((rule, i) => {
      if (rule.type === 'override_date_from_description') {
        const ruleErrors: { conditionRegex?: string; dateRegex?: string } = {};
        if (rule.conditionRegex !== '') {
          try {
            new RegExp(rule.conditionRegex);
          } catch {
            ruleErrors.conditionRegex = t('import.rules.invalidRegex');
          }
        }
        try {
          new RegExp(rule.dateRegex);
        } catch {
          ruleErrors.dateRegex = t('import.rules.invalidRegex');
        }
        if (Object.keys(ruleErrors).length > 0) {
          errors[i] = ruleErrors;
        }
      }
    });

    if (Object.keys(errors).length > 0) {
      setValidationErrors(errors);
      return;
    }
    setValidationErrors({});
    onNext();
  };

  return (
    <>
      <div className="flex flex-col gap-4">
        <div>
          <p className="text-sm font-medium">{t('import.rulesStep.title')}</p>
          <p className="text-sm text-muted-foreground">{t('import.rulesStep.description')}</p>
        </div>

        {rules.length === 0 ? (
          <p className="text-sm text-muted-foreground italic">{t('import.rulesStep.empty')}</p>
        ) : (
          <ol className="flex flex-col gap-3">
            {rules.map((rule, index) => (
              <li key={index} className="flex flex-col gap-2 rounded-[var(--radius)] border p-3">
                <div className="flex items-center justify-between gap-2">
                  <span className="text-xs font-semibold text-muted-foreground">
                    {index + 1}.{' '}
                    {rule.type === 'sign_from_column'
                      ? t('import.rules.signFromColumnLabel')
                      : t('import.rules.overrideDateFromDescriptionLabel')}
                  </span>
                  <div className="flex items-center gap-1">
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7"
                      onClick={() => handleMoveUp(index)}
                      disabled={index === 0}
                    >
                      <ChevronUp className="h-4 w-4" />
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7"
                      onClick={() => handleMoveDown(index)}
                      disabled={index === rules.length - 1}
                    >
                      <ChevronDown className="h-4 w-4" />
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7 text-destructive hover:text-destructive"
                      onClick={() => handleDelete(index)}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                </div>

                {rule.type === 'sign_from_column' ? (
                  <SignFromColumnForm
                    rule={rule}
                    csvHeaders={csvHeaders}
                    onChange={(updated: ImportRule) => handleRuleChange(index, updated)}
                  />
                ) : (
                  <OverrideDateFromDescriptionForm
                    rule={rule}
                    csvHeaders={csvHeaders}
                    onChange={(updated: ImportRule) => handleRuleChange(index, updated)}
                    errors={validationErrors[index]}
                  />
                )}
              </li>
            ))}
          </ol>
        )}

        <div className="flex items-center gap-2">
          <Select value={addType} onValueChange={setAddType}>
            <SelectTrigger className="w-64">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={ADD_NONE}>{t('import.rulesStep.selectType')}</SelectItem>
              <SelectItem value="sign_from_column">
                {t('import.rules.signFromColumnLabel')}
              </SelectItem>
              <SelectItem value="override_date_from_description">
                {t('import.rules.overrideDateFromDescriptionLabel')}
              </SelectItem>
            </SelectContent>
          </Select>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={handleAdd}
            disabled={addType === ADD_NONE}
          >
            {t('import.rulesStep.add')}
          </Button>
        </div>
      </div>

      <DialogFooter>
        <Button type="button" variant="outline" onClick={onBack}>
          {t('import.back')}
        </Button>
        <Button type="button" variant="outline" onClick={onCancel}>
          {t('modals.confirm.cancel')}
        </Button>
        <Button type="button" onClick={handleNext}>
          {t('import.next')}
        </Button>
      </DialogFooter>
    </>
  );
}
