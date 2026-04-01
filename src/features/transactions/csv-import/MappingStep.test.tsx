import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { I18nextProvider } from 'react-i18next'
import i18n from '../../../i18n'
import MappingStep from './MappingStep'
import type { ColumnMapping } from './types'

describe('MappingStep', () => {
  const mockProps = {
    csvHeaders: ['Date', 'Amount', 'no-header-column-3', 'no-header-column-4'],
    columnMapping: {
      date: null,
      amount: null,
      partner: null,
      note: null,
      currency: null,
      fxRate: null,
    } as ColumnMapping,
    onMappingChange: vi.fn(),
    onNext: vi.fn(),
    onBack: vi.fn(),
    onCancel: vi.fn(),
    canProceed: true,
  }

  const renderComponent = (props = mockProps) => {
    return render(
      <I18nextProvider i18n={i18n}>
        <MappingStep {...props} />
      </I18nextProvider>,
    )
  }

  it('renders without crashing with missing headers', () => {
    expect(() => renderComponent()).not.toThrow()
  })

  it('displays friendly labels for no-header-column tokens in dropdown', () => {
    renderComponent()
    // Open the dropdown for the date field
    const dateSelectTrigger = screen.getAllByRole('combobox')[0] // First select is date
    fireEvent.click(dateSelectTrigger)
    expect(screen.getByText('No header (column 3)')).toBeInTheDocument()
    expect(screen.getByText('No header (column 4)')).toBeInTheDocument()
  })

  it('displays normal headers as-is', () => {
    renderComponent()
    expect(screen.getByText('Date')).toBeInTheDocument()
    expect(screen.getByText('Amount')).toBeInTheDocument()
  })
})
