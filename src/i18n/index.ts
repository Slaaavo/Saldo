import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import en from './locales/en.yml';
import sk from './locales/sk.yml';

const STORAGE_KEY = 'saldo-language';

// One-time migration: move language preference from old key to new key
const OLD_STORAGE_KEY = 'our-finances-language';
const savedLang = localStorage.getItem(OLD_STORAGE_KEY);
if (savedLang) {
  localStorage.setItem(STORAGE_KEY, savedLang);
  localStorage.removeItem(OLD_STORAGE_KEY);
}

i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    sk: { translation: sk },
  },
  lng: localStorage.getItem(STORAGE_KEY) || 'en',
  fallbackLng: 'en',
  interpolation: {
    escapeValue: false, // React already escapes output
  },
});

// Persist language changes
i18n.on('languageChanged', (lng: string) => {
  localStorage.setItem(STORAGE_KEY, lng);
});

export default i18n;
