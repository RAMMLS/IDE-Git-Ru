import { NavLink, Outlet } from 'react-router-dom';
import {
  CircleDot,
  Code2,
  GitBranch,
  GitCommit,
  GitPullRequest,
  RadioTower,
  Scale,
  Settings,
} from 'lucide-react';
import { useQuery } from '@tanstack/react-query';
import { api } from '@/api/client';
import { useRepoContext } from '@/components/providers/repo-context';

const tabs = [
  { to: '/', label: 'Code', icon: Code2, end: true },
  { to: '/commits', label: 'Commits', icon: GitCommit },
  { to: '/branches', label: 'Branches', icon: GitBranch },
  { to: '/pull-requests', label: 'Pull Requests', icon: GitPullRequest },
  { to: '/issues', label: 'Issues', icon: CircleDot },
  { to: '/source-control', label: 'Source Control', icon: RadioTower },
];

export function AppLayout() {
  const { repos, selectedRepo, selectedRepoId, setSelectedRepoId, isLoading: isReposLoading } = useRepoContext();
  const { data: status } = useQuery({
    queryKey: ['status', selectedRepoId],
    queryFn: () => api.getStatus(selectedRepoId ?? undefined),
    enabled: !!selectedRepoId,
    retry: false,
  });
  const { data: commits = [] } = useQuery({
    queryKey: ['commits', selectedRepoId],
    queryFn: () => api.getCommits(selectedRepoId ?? undefined),
    enabled: !!selectedRepoId,
    initialData: [],
    retry: false,
  });

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <header className="border-b bg-[#24292f] text-white">
        <div className="flex h-14 items-center justify-between gap-4 px-5">
          <div className="flex min-w-0 items-center gap-3">
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-white/15 bg-white text-sm font-bold text-[#24292f]">
              A
            </div>
            <span className="text-base font-semibold">Aura</span>
          </div>
          <div className="flex min-w-0 items-center gap-3">
            <select
              className="h-9 min-w-48 rounded-md border border-white/15 bg-[#1f2328] px-3 text-sm text-white outline-none"
              value={selectedRepoId ?? ''}
              onChange={(event) => setSelectedRepoId(event.target.value || null)}
              disabled={isReposLoading || repos.length === 0}
            >
              {repos.length === 0 ? (
                <option value="">No repositories</option>
              ) : (
                repos.map((repo) => (
                  <option key={repo.id} value={repo.id}>
                    {repo.name}
                  </option>
                ))
              )}
            </select>
            <Settings className="hidden h-4 w-4 text-white/70 sm:block" />
          </div>
        </div>

        <div className="border-t border-white/10 bg-[#f6f8fa] text-[#24292f]">
          <div className="mx-auto max-w-7xl px-5 py-4">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="min-w-0">
                <div className="flex min-w-0 items-center gap-2">
                  <Scale className="h-4 w-4 text-muted-foreground" />
                  <span className="truncate text-xl font-semibold">
                    {selectedRepo?.name ?? 'No repository selected'}
                  </span>
                  {selectedRepo && (
                    <span className="rounded-full border bg-white px-2 py-0.5 text-xs text-muted-foreground">
                      {selectedRepo.storage}
                    </span>
                  )}
                </div>
                {selectedRepo && (
                  <div className="mt-1 truncate text-sm text-muted-foreground">{selectedRepo.path}</div>
                )}
              </div>
              {status?.branch && (
                <div className="flex items-center gap-3 text-sm">
                  <span className="inline-flex items-center gap-1 rounded-full border bg-white px-3 py-1">
                    <GitBranch className="h-4 w-4" />
                    {status.branch}
                  </span>
                  <span className="hidden font-mono text-xs text-muted-foreground sm:inline">
                    {commits[0]?.hash.slice(0, 7) ?? status.head?.slice(0, 7) ?? 'empty'}
                  </span>
                </div>
              )}
            </div>
          </div>
        </div>

        <nav className="border-t bg-white">
          <div className="mx-auto flex max-w-7xl overflow-x-auto px-5">
            {tabs.map((tab) => {
              const Icon = tab.icon;
              return (
                <NavLink
                  key={tab.to}
                  to={tab.to}
                  end={tab.end}
                  className={({ isActive }) =>
                    `inline-flex h-12 shrink-0 items-center gap-2 border-b-2 px-3 text-sm font-medium transition-colors ${
                      isActive
                        ? 'border-[#fd8c73] text-[#24292f]'
                        : 'border-transparent text-muted-foreground hover:text-[#24292f]'
                    }`
                  }
                >
                  <Icon className="h-4 w-4" />
                  {tab.label}
                </NavLink>
              );
            })}
          </div>
        </nav>
      </header>

      <main className="min-h-0 flex-1 overflow-hidden bg-white">
        <Outlet />
      </main>
    </div>
  );
}
