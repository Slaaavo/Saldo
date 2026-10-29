export const monthName = (month: number, language: string): string => new Intl.DateTimeFormat(language, { month: 'long' }).format(new Date(2000, month - 1, 1))
