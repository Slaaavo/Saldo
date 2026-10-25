export const translate = (key: string, opts?: Record<string, unknown>): string => {
  if (!opts || Object.keys(opts).length === 0) return key
  const values = Object.entries(opts).map(([name, value]) => `${name}=${String(value)}`)
  return `${key} (${values.join(', ')})`
}

export const i18nMock = {
  useTranslation: () => ({ t: translate, i18n: { language: 'en' } }),
}
