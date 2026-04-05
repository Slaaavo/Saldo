import type { ReactNode } from 'react'
import { useModalManager } from './useModalManager'
import { ModalContext } from './useModal'

export const ModalProvider = ({ children }: { children: ReactNode }) => {
  const { modalState, setModalState, closeModal } = useModalManager()
  return <ModalContext.Provider value={{ modalState, setModalState, closeModal }}>{children}</ModalContext.Provider>
}
