import { create } from 'zustand';
import type { EngineContext, Selection } from './types';

interface UiState {
  context?: EngineContext;
  selection?: Selection;
  commandStatus: 'idle' | 'working' | 'applied' | 'rejected';
  error?: string;
  setContext: (context: EngineContext) => void;
  select: (selection?: Selection) => void;
  setCommand: (status: UiState['commandStatus'], error?: string) => void;
}

export const useUiStore = create<UiState>((set) => ({
  commandStatus: 'idle',
  setContext: (context) => set({ context }),
  select: (selection) => set({ selection }),
  setCommand: (commandStatus, error) => set({ commandStatus, error }),
}));
