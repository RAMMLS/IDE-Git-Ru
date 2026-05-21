import { NavLink, Outlet } from 'react-router-dom';
import { GitBranch, GitCommit, Activity, FolderGit2 } from 'lucide-react';
import { useQuery } from '@tanstack/react-query';
import { api } from '@/api/client';
import { useRepoContext } from '@/components/providers/repo-context';
import { ScrollArea } from '@/components/ui/scroll-area';

export function AppLayout() {
  const { repos, selectedRepo, selectedRepoId, setSelectedRepoId, isLoading: isReposLoading } = useRepoContext();
  const { data: status } = useQuery({
    queryKey: ['status', selectedRepoId],
    queryFn: () => api.getStatus(selectedRepoId ?? undefined),
    enabled: !!selectedRepoId,
  });
  const { data: branches = [] } = useQuery({
    queryKey: ['branches', selectedRepoId],
    queryFn: () => api.getBranches(selectedRepoId ?? undefined),
    enabled: !!selectedRepoId,
    initialData: [],
  });

  return (
    <div className="flex h-screen flex-col bg-background text-foreground dark">
      <header className="flex h-14 items-center justify-between border-b bg-card px-4 shadow-sm">
        <div className="flex items-center space-x-2">
          <div className="flex h-8 w-8 items-center justify-center rounded-md bg-primary text-primary-foreground font-bold">
            A
          </div>
          <span className="text-lg font-semibold tracking-tight">Aura VCS</span>
        </div>
        <div className="flex items-center gap-4 text-sm">
          <div className="flex items-center gap-2">
            <FolderGit2 className="h-4 w-4 text-muted-foreground" />
            <select
              className="h-9 rounded-md border bg-background px-3 text-sm outline-none"
              value={selectedRepoId ?? ''}
              onChange={(event) => setSelectedRepoId(event.target.value || null)}
              disabled={isReposLoading || repos.length === 0}
            >
              {repos.length === 0 ? (
                <option value="">Нет репозиториев</option>
              ) : (
                repos.map((repo) => (
                  <option key={repo.id} value={repo.id}>
                    {repo.name}
                  </option>
                ))
              )}
            </select>
          </div>
          {status?.branch && (
            <div className="flex items-center space-x-1 text-muted-foreground">
              <GitBranch className="h-4 w-4" />
              <span>{status.branch}</span>
            </div>
          )}
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden">
        <aside className="w-64 border-r bg-card/50 flex flex-col">
          <div className="p-4 border-b">
            <h2 className="mb-2 text-xs font-semibold tracking-tight text-muted-foreground uppercase">
              Navigation
            </h2>
            <nav className="space-y-1">
              <NavLink
                to="/"
                className={({ isActive }) =>
                  `flex items-center space-x-2 rounded-md px-3 py-2 text-sm font-medium transition-colors ${
                    isActive ? 'bg-secondary text-secondary-foreground' : 'text-muted-foreground hover:bg-secondary/50 hover:text-foreground'
                  }`
                }
              >
                <GitCommit className="h-4 w-4" />
                <span>Commits</span>
              </NavLink>
              <NavLink
                to="/status"
                className={({ isActive }) =>
                  `flex items-center space-x-2 rounded-md px-3 py-2 text-sm font-medium transition-colors ${
                    isActive ? 'bg-secondary text-secondary-foreground' : 'text-muted-foreground hover:bg-secondary/50 hover:text-foreground'
                  }`
                }
              >
                <Activity className="h-4 w-4" />
                <span>Status</span>
              </NavLink>
            </nav>
          </div>

          <div className="p-4 flex-1 flex flex-col min-h-0">
            <div className="mb-4 rounded-md border bg-muted/30 p-3 text-sm">
              {selectedRepo ? (
                <>
                  <div className="font-medium">{selectedRepo.name}</div>
                  <div className="text-xs text-muted-foreground uppercase tracking-wide">
                    {selectedRepo.storage === 'hosted' ? 'Hosted' : 'Linked'}
                  </div>
                  <div className="mt-2 break-all text-xs text-muted-foreground">
                    {selectedRepo.path}
                  </div>
                </>
              ) : (
                <div className="text-muted-foreground">
                  Создай или зарегистрируй репозиторий на странице `Status`.
                </div>
              )}
            </div>

            <div className="flex items-center justify-between mb-2">
              <h2 className="text-xs font-semibold tracking-tight text-muted-foreground uppercase">
                Branches
              </h2>
            </div>
            <ScrollArea className="flex-1 -mx-2">
              <div className="px-2 space-y-1">
                {branches.length === 0 ? (
                  <div className="rounded-md px-3 py-2 text-sm text-muted-foreground">
                    {selectedRepo ? 'Веток пока нет.' : 'Репозиторий не выбран.'}
                  </div>
                ) : (
                  branches.map((branch) => (
                    <div
                      key={branch}
                      className={`flex w-full items-center space-x-2 rounded-md px-3 py-2 text-sm font-medium transition-colors ${
                        branch === status?.branch
                          ? 'bg-primary/10 text-primary'
                          : 'text-muted-foreground hover:bg-secondary/50 hover:text-foreground'
                      }`}
                    >
                      <GitBranch className="h-4 w-4" />
                      <span className="truncate">{branch}</span>
                    </div>
                  ))
                )}
              </div>
            </ScrollArea>
          </div>
        </aside>

        <main className="flex-1 overflow-hidden relative bg-background">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
