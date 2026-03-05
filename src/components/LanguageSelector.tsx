import { useTranslation } from 'react-i18next';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select';

const LANGUAGES = [
  { code: 'en', labelKey: 'language.en' },
  { code: 'sk', labelKey: 'language.sk' },
] as const;

export default function LanguageSelector() {
  const { t, i18n } = useTranslation();

  return (
    <Select value={i18n.language} onValueChange={(lng) => i18n.changeLanguage(lng)}>
      <SelectTrigger className="w-32 h-8 text-xs">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {LANGUAGES.map(({ code, labelKey }) => (
          <SelectItem key={code} value={code}>
            {t(labelKey)}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
