import Papa from 'papaparse'
import type { CsvRow, ColumnMapping, CashflowFieldKey } from './types'

export const parseCsvFile = async (file: File): Promise<{ headers: string[]; rows: CsvRow[] }> => {
  const text = await readFileAsText(file)
  const lines = text.split('\n')
  if (lines.length < 2) {
    throw new Error('The CSV file must have at least a header row and one data row.')
  }
  const firstLine = lines[0]
  return new Promise((resolve, reject) => {
    Papa.parse<CsvRow>(text, {
      header: true,
      skipEmptyLines: true,
      dynamicTyping: false,
      complete: (results) => {
        if (results.errors.length > 0 && results.data.length === 0) {
          reject(new Error(`CSV parsing failed: ${results.errors[0].message}`))
          return
        }
        if (results.data.length === 0) {
          reject(new Error('The CSV file contains no data rows.'))
          return
        }
        const rawParse = Papa.parse(firstLine, { header: false, skipEmptyLines: false, delimiter: results.meta.delimiter })
        const rawHeaders = rawParse.data[0] as string[]
        const originalHeaders = results.meta.fields ?? []
        const normalizedHeaders = originalHeaders.map((h, i) => {
          const raw = rawHeaders[i] || ''
          if (raw.trim() === '') {
            return `no-header-column-${i + 1}`
          }
          return h
        })
        const transformedRows = results.data.map((row) => {
          const newRow: CsvRow = {}
          originalHeaders.forEach((orig, i) => {
            newRow[normalizedHeaders[i]] = row[orig]
          })
          return newRow
        })
        resolve({ headers: normalizedHeaders, rows: transformedRows })
      },
      error: (err: { message: string }) => {
        reject(new Error(`CSV parsing failed: ${err.message}`))
      },
    })
  })
}

const readFileAsText = async (file: File): Promise<string> => {
  // Try UTF-8 first
  let text = await file.text()
  // If result contains replacement characters, try Windows-1250 (common for Slovak/Czech bank exports)
  if (text.includes('\uFFFD')) {
    const buffer = await file.arrayBuffer()
    const decoder = new TextDecoder('windows-1250')
    text = decoder.decode(buffer)
  }
  return text
}

const FIELD_PATTERNS: Array<{ field: CashflowFieldKey; patterns: string[] }> = [
  { field: 'date', patterns: ['date', 'datum', 'dátum'] },
  { field: 'amount', patterns: ['amount', 'suma', 'čiastka', 'castka'] },
  { field: 'partner', patterns: ['iban'] },
  { field: 'note', patterns: ['note', 'poznámka', 'poznamka', 'popis', 'description'] },
  { field: 'currency', patterns: ['currency', 'mena'] },
  { field: 'fxRate', patterns: ['rate', 'kurz'] },
]

export const autoDetectMapping = (headers: string[]): ColumnMapping => {
  const mapping: ColumnMapping = {
    date: null,
    amount: null,
    partner: null,
    note: null,
    currency: null,
    fxRate: null,
  }

  for (const header of headers) {
    const lower = header.toLowerCase()
    for (const { field, patterns } of FIELD_PATTERNS) {
      if (mapping[field] !== null) continue
      if (patterns.some((p) => lower.includes(p))) {
        mapping[field] = header
        break
      }
    }
  }

  return mapping
}

export const parseAmount = (raw: string): number | null => {
  // Strip currency symbols, spaces, and leading plus signs; normalise the Unicode minus sign
  let cleaned = raw.replace(/[€$£¥₹\s+]/g, '').replace(/−/g, '-')

  // Accounting notations for negatives: "(45.00)", "45,00-" and the usual "-45.00"
  let negative = false
  if (cleaned.startsWith('(') && cleaned.endsWith(')')) {
    negative = true
    cleaned = cleaned.slice(1, -1)
  }
  if (cleaned.endsWith('-')) {
    negative = true
    cleaned = cleaned.slice(0, -1)
  } else if (cleaned.startsWith('-')) {
    negative = true
    cleaned = cleaned.slice(1)
  }

  if (cleaned === '') return null

  const lastComma = cleaned.lastIndexOf(',')
  const lastDot = cleaned.lastIndexOf('.')

  let normalized: string

  if (lastComma > lastDot) {
    // European format: 1.234,56 — comma is decimal separator
    normalized = cleaned.replace(/\./g, '').replace(',', '.')
  } else if (lastDot > lastComma) {
    // US format: 1,234.56 — dot is decimal separator
    normalized = cleaned.replace(/,/g, '')
  } else {
    // No separators at all (e.g., "1234")
    normalized = cleaned
  }

  const value = parseFloat(normalized)
  if (isNaN(value)) return null
  return negative ? -value : value
}

const DATE_PATTERNS: Array<{
  regex: RegExp
  parse: (m: RegExpMatchArray) => [number, number, number]
}> = [
  {
    // YYYYMMDD (exactly 8 digits, no separators) — must precede YYYY-MM-DD to avoid ambiguity
    regex: /^(\d{4})(\d{2})(\d{2})$/,
    parse: (m) => [parseInt(m[1]), parseInt(m[2]), parseInt(m[3])],
  },
  {
    // YYYY-MM-DD
    regex: /^(\d{4})-(\d{1,2})-(\d{1,2})$/,
    parse: (m) => [parseInt(m[1]), parseInt(m[2]), parseInt(m[3])],
  },
  {
    // DD.MM.YYYY or D.M.YYYY
    regex: /^(\d{1,2})\.(\d{1,2})\.(\d{4})$/,
    parse: (m) => [parseInt(m[3]), parseInt(m[2]), parseInt(m[1])],
  },
  {
    // DD/MM/YYYY
    regex: /^(\d{1,2})\/(\d{1,2})\/(\d{4})$/,
    parse: (m) => [parseInt(m[3]), parseInt(m[2]), parseInt(m[1])],
  },
  {
    // DD-MM-YYYY
    regex: /^(\d{2})-(\d{2})-(\d{4})$/,
    parse: (m) => [parseInt(m[3]), parseInt(m[2]), parseInt(m[1])],
  },
]

export const parseDateString = (raw: string): string | null => {
  if (!raw || raw.trim() === '') return null

  // Bank exports often append a time ("2024-03-15 10:22:33"); only the date part is used
  const datePart = raw.trim().replace(/[ T]\d{1,2}:\d{2}(:\d{2}(\.\d+)?)?$/, '')

  for (const { regex, parse } of DATE_PATTERNS) {
    const match = datePart.match(regex)
    if (!match) continue

    const [year, month, day] = parse(match)

    if (month < 1 || month > 12) return null
    const daysInMonth = new Date(Date.UTC(year, month, 0)).getUTCDate()
    if (day < 1 || day > daysInMonth) return null

    const yyyy = String(year).padStart(4, '0')
    const mm = String(month).padStart(2, '0')
    const dd = String(day).padStart(2, '0')

    return `${yyyy}-${mm}-${dd}T12:00:00`
  }

  return null
}
