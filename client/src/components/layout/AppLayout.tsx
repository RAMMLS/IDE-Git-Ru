import { NavLink, Outlet } from 'react-router-dom';
import { GitBranch, Plus, GitCommit, Activity } from 'lucide-react';
import { useQuery } from '@tanstack/react-query';
import { api } from '@/api/client';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';

export function AppLayout() {
  const { data: status } = useQuery({ queryKey: ['status'], queryFn: api.getStatus });
  const { data: branches } = useQuery({ queryKey: ['branches'], queryFn: api.getBranches, initialData: [] });

  return (
    <div className="flex h-screen flex-col bg-background text-foreground dark">
      {/* Header */}
      <header className="flex h-14 items-center justify-between border-b bg-card px-4 shadow-sm">
        <div className="flex items-center space-x-2">
          <div className="flex h-8 w-8 items-center justify-center rounded-md bg-primary text-primary-foreground font-bold">
            A
          </div>
          <span className="text-lg font-semibold tracking-tight">Aura VCS</span>
        </div>
        <div className="flex items-center space-x-4 text-sm">
          {status?.branch && (
            <div className="flex items-center space-x-1 text-muted-foreground">
              <GitBranch className="h-4 w-4" />
              <span>{status.branch}</span>
            </div>
          )}
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar */}
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
            <div className="flex items-center justify-between mb-2">
              <h2 className="text-xs font-semibold tracking-tight text-muted-foreground uppercase">
                Branches
              </h2>
              <Button variant="ghost" size="icon" className="h-5 w-5 rounded-full">
                <Plus className="h-3 w-3" />
              </Button>
            </div>
            <ScrollArea className="flex-1 -mx-2">
              <div className="px-2 space-y-1">
                {branches?.map((branch) => (
                  <button
                    key={branch}
                    className={`flex w-full items-center space-x-2 rounded-md px-3 py-2 text-sm font-medium transition-colors ${
                      branch === status?.branch
                        ? 'bg-primary/10 text-primary'
                        : 'text-muted-foreground hover:bg-secondary/50 hover:text-foreground'
                    }`}
                  >
                    <GitBranch className="h-4 w-4" />
                    <span className="truncate">{branch}</span>
                  </button>
                ))}
              </div>
            </ScrollArea>
          </div>
        </aside>

        {/* Main Content */}
        <main className="flex-1 overflow-hidden relative bg-background">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
