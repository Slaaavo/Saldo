export interface IbanSegment {
  text: string
  weight: string
}

export function formatIbanSegments(iban: string): IbanSegment[] {
  const normalized = iban.replace(/\s+/g, '').toUpperCase()

  // Split into visual groups of 4 chars (standard IBAN display)
  const groups: string[] = []
  for (let i = 0; i < normalized.length; i += 4) {
    groups.push(normalized.slice(i, i + 4))
  }

  // SK IBAN structure: SKkk (4) + bank code (4) + prefix (6) + account number (10) = 24 chars
  // Char positions:    0-3        4-7             8-13          14-23
  // In groups of 4:    [0]        [1]             [2][3]        [3][4][5]
  // Group 3 (chars 12-15) straddles prefix (12-13) and account number (14-15)
  // → we assign weight by which structural part the majority/start of each group falls in
  if (normalized.startsWith('SK') && normalized.length === 24) {
    // Map each character position to its weight
    const charWeight = (pos: number): string => {
      if (pos < 4) return 'font-normal' // country + check
      if (pos < 8) return 'font-bold' // bank code
      if (pos < 14) return 'font-normal' // account prefix
      return 'font-bold' // account number
    }

    // Split groups that cross weight boundaries into sub-spans
    const result: IbanSegment[] = []
    let charPos = 0
    for (const group of groups) {
      let current = ''
      let currentWeight = charWeight(charPos)
      for (const ch of group) {
        const w = charWeight(charPos)
        if (w !== currentWeight) {
          if (current) result.push({ text: current, weight: currentWeight })
          current = ''
          currentWeight = w
        }
        current += ch
        charPos++
      }
      if (current) result.push({ text: current, weight: currentWeight })
      // Add a space separator marker after each group (except the last)
      if (charPos < normalized.length) {
        result.push({ text: ' ', weight: '' })
      }
    }
    return result
  }

  // Non-SK: groups of 4, uniform weight
  return groups.flatMap((text, i) =>
    i < groups.length - 1
      ? [
          { text, weight: 'font-bold' },
          { text: ' ', weight: '' },
        ]
      : [{ text, weight: 'font-bold' }],
  )
}
