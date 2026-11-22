import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { I18nextProvider } from 'react-i18next'
import i18n from '../../../../i18n'
import type { ImportRule } from '../../../../shared/types'
import RulesStep from './RulesStep'

const renderRules = (rules: ImportRule[]) => {
  const props = { rules, csvHeaders: ['Stav transakcie'], onRulesChange: vi.fn(), onNext: vi.fn(), onBack: vi.fn(), onCancel: vi.fn() }
  render(
    <I18nextProvider i18n={i18n}>
      <RulesStep {...props} />
    </I18nextProvider>,
  )
  return props
}

describe('RulesStep card hold rules', () => {
  it('edits the pattern of a card hold rule', async () => {
    const props = renderRules([{ type: 'pending_from_column', sortOrder: 0, column: 'Stav transakcie', pattern: '' }])

    expect(screen.getByText('1. Card hold from column')).toBeVisible()
    await userEvent.type(screen.getByRole('textbox', { name: 'Pattern (regex, ignores case)' }), 'A')

    expect(props.onRulesChange).toHaveBeenCalledWith([{ type: 'pending_from_column', sortOrder: 0, column: 'Stav transakcie', pattern: 'A' }])
  })

  it('stops at an invalid pattern of a declined payment rule', async () => {
    const props = renderRules([{ type: 'declined_from_column', sortOrder: 0, column: 'Stav transakcie', pattern: '[Zamietnut' }])

    await userEvent.click(screen.getByRole('button', { name: 'Next' }))

    expect(screen.getByText('Invalid regex pattern')).toBeVisible()
    expect(props.onNext).not.toHaveBeenCalled()
  })
})
