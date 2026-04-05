import { createContext, useContext } from 'react'
import type { ReactNode } from 'react'
import { useModalManager } from './useModalManager'
import type { ModalState } from '../shared/types'

interface ModalContextValue {
  modalState: ModalState
  setModalState: (state: ModalState) => void
  closeModal: () => void
}

const ModalContext = createContext<ModalContextValue | null>(null)

export const ModalProvider = ({ children }: { children: ReactNode }) => {
  const { modalState, setModalState, closeModal } = useModalManager()
  return <ModalContext.Provider value={{ modalState, setModalState, closeModal }}>{children}</ModalContext.Provider>
}

// eslint-disable-next-line react-refresh/only-export-components
export const useModal = (): ModalContextValue => {
  const ctx = useContext(ModalContext)
  if (!ctx) throw new Error('useModal must be used within a ModalProvider')
  return ctx
}
