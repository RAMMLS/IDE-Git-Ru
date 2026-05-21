import { createContext, useContext } from 'react';
import type { RepoRecord } from '@/api/client';

export interface RepoContextValue {
  repos: RepoRecord[];
  selectedRepoId: string | null;
  selectedRepo: RepoRecord | null;
  isLoading: boolean;
  setSelectedRepoId: (repoId: string | null) => void;
}

export const RepoContext = createContext<RepoContextValue | undefined>(undefined);

export function useRepoContext() {
  const context = useContext(RepoContext);

  if (!context) {
    throw new Error('useRepoContext must be used within RepoProvider');
  }

  return context;
}
