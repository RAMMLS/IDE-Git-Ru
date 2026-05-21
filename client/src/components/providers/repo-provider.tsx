import React, { useEffect, useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { api } from '@/api/client';
import { RepoContext, type RepoContextValue } from '@/components/providers/repo-context';

const STORAGE_KEY = 'aura.active-repo';

function readStoredRepoId() {
  if (typeof window === 'undefined') {
    return null;
  }

  return window.localStorage.getItem(STORAGE_KEY);
}

export function RepoProvider({ children }: { children: React.ReactNode }) {
  const [preferredRepoId, setPreferredRepoId] = useState<string | null>(() => readStoredRepoId());
  const { data: repos = [], isLoading } = useQuery({
    queryKey: ['repos'],
    queryFn: api.listRepos,
    initialData: [],
  });
  const selectedRepoId = useMemo(() => {
    if (!repos.length) {
      return null;
    }

    if (preferredRepoId && repos.some((repo) => repo.id === preferredRepoId)) {
      return preferredRepoId;
    }

    const storedRepoId = readStoredRepoId();
    return repos.find((repo) => repo.id === storedRepoId)?.id ?? repos[0].id;
  }, [preferredRepoId, repos]);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }

    if (selectedRepoId) {
      window.localStorage.setItem(STORAGE_KEY, selectedRepoId);
    } else {
      window.localStorage.removeItem(STORAGE_KEY);
    }
  }, [selectedRepoId]);

  const value = useMemo<RepoContextValue>(() => {
    const selectedRepo = repos.find((repo) => repo.id === selectedRepoId) ?? null;

    return {
      repos,
      selectedRepoId,
      selectedRepo,
      isLoading,
      setSelectedRepoId: setPreferredRepoId,
    };
  }, [isLoading, repos, selectedRepoId]);

  return <RepoContext.Provider value={value}>{children}</RepoContext.Provider>;
}
