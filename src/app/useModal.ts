import { createContext, useContext } from 'react'
import type { ModalState } from '../shared/types'

export interface ModalContextValue {
  modalState: ModalState
  setModalState: (state: ModalState) => void
  closeModal: () => void
}

export const ModalContext = createContext<ModalContextValue | null>(null)

export const useModal = (): ModalContextValue => {
  const ctx = useContext(ModalContext)
  if (!ctx) throw new Error('useModal must be used within a ModalProvider')
  return ctx
}
