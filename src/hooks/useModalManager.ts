import { useState } from 'react';
import type { ModalState } from '../types';

export function useModalManager() {
  const [modalState, setModalState] = useState<ModalState>({ type: 'none' });

  const closeModal = () => setModalState({ type: 'none' });

  return { modalState, setModalState, closeModal };
}
