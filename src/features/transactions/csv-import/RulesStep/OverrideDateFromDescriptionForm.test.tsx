import { describe, it, expect, afterEach, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { I18nextProvider } from 'react-i18next'
import i18n from '../../../../i18n'
import OverrideDateFromDescriptionForm from './OverrideDateFromDescriptionForm'
import type { ImportRule } from '../../../../shared/types'

const rule: ImportRule & { type: 'override_date_from_description' } = {
  type: 'override_date_from_description',
  sortOrder: 0,
  descriptionColumn: '',
  conditionRegex: '',
  dateRegex: '',
}

const renderForm = () =>
  render(
    <I18nextProvider i18n={i18n}>
      <OverrideDateFromDescriptionForm rule={rule} csvHeaders={[]} onChange={vi.fn()} />
    </I18nextProvider>,
  )

describe('OverrideDateFromDescriptionForm regex reference', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en')
  })

  it('shows the reference table in English', async () => {
    await i18n.changeLanguage('en')
    renderForm()

    expect(screen.getByText('Pattern')).toBeInTheDocument()
    expect(screen.getByText('Any digit (0–9)')).toBeInTheDocument()
    expect(screen.getByText('\\d\\d matches 42')).toBeInTheDocument()
    expect(screen.getByText('^1234 matches only if the text starts with 1234')).toBeInTheDocument()
  })

  it('shows the reference table in Slovak', async () => {
    await i18n.changeLanguage('sk')
    renderForm()

    expect(screen.getByText('Vzor')).toBeInTheDocument()
    expect(screen.getByText('Ľubovoľná číslica (0–9)')).toBeInTheDocument()
    expect(screen.getByText('\\d\\d zodpovedá 42')).toBeInTheDocument()
    expect(screen.queryByText('Meaning')).not.toBeInTheDocument()
  })
})
