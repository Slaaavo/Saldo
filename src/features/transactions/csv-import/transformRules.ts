import type { ImportRule, SignFromColumnParams, OverrideDateFromDescriptionParams } from '../../../shared/types'
import type { CsvRow, ColumnMapping, TransformedCsvRow } from './types'
import { parseDateString } from './csvParser'

export const applyRules = (
  csvRows: CsvRow[],
  rules: ImportRule[],
  // columnMapping is part of the public API signature reserved for future rule types
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  _columnMapping: ColumnMapping,
): TransformedCsvRow[] => {
  const sortedRules = [...rules].sort((a, b) => a.sortOrder - b.sortOrder)

  return csvRows.map((row) => {
    const transformed = {
      ...row,
      __negateAmount: false,
      __overrideDateString: null,
    } as TransformedCsvRow

    for (const rule of sortedRules) {
      if (rule.type === 'sign_from_column') {
        applySignFromColumn(transformed, rule as typeof rule & SignFromColumnParams)
      } else if (rule.type === 'override_date_from_description') {
        applyOverrideDateFromDescription(transformed, rule as typeof rule & OverrideDateFromDescriptionParams)
      }
    }

    return transformed
  })
}

const applySignFromColumn = (row: TransformedCsvRow, rule: { typeColumn: string; negativeType: string }): void => {
  const value = row[rule.typeColumn]
  if (value === undefined) return
  if (value.trim().toLowerCase() === rule.negativeType.trim().toLowerCase()) {
    row.__negateAmount = true
  }
}

const applyOverrideDateFromDescription = (row: TransformedCsvRow, rule: { descriptionColumn: string; conditionRegex: string; dateRegex: string }): void => {
  const description = row[rule.descriptionColumn]
  if (description === undefined) return

  let conditionMatches: boolean
  if (rule.conditionRegex === '') {
    conditionMatches = true
  } else {
    let conditionRe: RegExp
    try {
      conditionRe = new RegExp(rule.conditionRegex)
    } catch {
      return
    }
    conditionMatches = conditionRe.test(description)
  }

  if (!conditionMatches) return

  let dateRe: RegExp
  try {
    dateRe = new RegExp(rule.dateRegex)
  } catch {
    return
  }

  const match = description.match(dateRe)
  if (!match || !match[1]) return

  const parsed = parseDateString(match[1])
  if (parsed !== null) {
    row.__overrideDateString = parsed
  }
}
