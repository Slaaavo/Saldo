import { describe, it, expect, beforeAll } from 'vitest'
import { render, screen } from '@testing-library/react'
import { I18nextProvider } from 'react-i18next'
import i18n from '../../../i18n'
import SaveProfileStep from './SaveProfileStep'
import { Dialog, DialogContent } from '../../../shared/ui/dialog'

const renderStep = (counts: { importedCount: number; removedHoldCount: number; repricedHoldCount: number }) =>
  render(
    <I18nextProvider i18n={i18n}>
      <Dialog open>
        <DialogContent aria-describedby={undefined}>
          <SaveProfileStep {...counts} loadedProfileName={null} onSaveNew={async () => {}} onUpdate={async () => {}} onClose={() => {}} />
        </DialogContent>
      </Dialog>
    </I18nextProvider>,
  )

describe('SaveProfileStep', () => {
  beforeAll(async () => {
    await i18n.changeLanguage('en')
  })

  it('leaves out a zero imported line when a hold count shows', () => {
    renderStep({ importedCount: 0, removedHoldCount: 0, repricedHoldCount: 1 })

    expect(screen.queryByText('Imported 0 transactions')).not.toBeInTheDocument()
    expect(screen.getByText('Updated 1 card hold to its booked amount')).toBeInTheDocument()
  })

  it('names every count that is not zero', () => {
    renderStep({ importedCount: 2, removedHoldCount: 1, repricedHoldCount: 1 })

    expect(screen.getByText('Imported 2 transactions')).toBeInTheDocument()
    expect(screen.getByText('Updated 1 card hold to its booked amount')).toBeInTheDocument()
    expect(screen.getByText('Removed 1 cancelled card hold')).toBeInTheDocument()
  })

  it('says that nothing was imported when every count is zero', () => {
    renderStep({ importedCount: 0, removedHoldCount: 0, repricedHoldCount: 0 })

    expect(screen.getByText('Imported 0 transactions')).toBeInTheDocument()
  })
})
