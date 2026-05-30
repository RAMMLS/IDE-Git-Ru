import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { GitBranch, Plus } from 'lucide-react';
import { api } from '@/api/client';
import { useRepoContext } from '@/components/providers/repo-context';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

export function BranchesPage() {
  const queryClient = useQueryClient();
  const { selectedRepoId } = useRepoContext();
  const [branchName, setBranchName] = useState('');

  const statusQuery = useQuery({
    queryKey: ['status', selectedRepoId],
    queryFn: () => api.getStatus(selectedRepoId ?? undefined),
    enabled: !!selectedRepoId,
  });
  const branchesQuery = useQuery({
    queryKey: ['branches', selectedRepoId],
    queryFn: () => api.getBranches(selectedRepoId ?? undefined),
    enabled: !!selectedRepoId,
    initialData: [],
  });
  const createBranch = useMutation({
    mutationFn: (name: string) => api.createBranch(name, selectedRepoId ?? undefined),
    onSuccess: () => {
      setBranchName('');
      queryClient.invalidateQueries({ queryKey: ['branches', selectedRepoId] });
    },
  });

  if (!selectedRepoId) {
    return <div className="p-8 text-muted-foreground">Create or select a repository in Source Control.</div>;
  }

  const activeBranch = statusQuery.data?.branch;

  return (
    <div className="mx-auto flex h-full max-w-5xl flex-col gap-4 p-6">
      <div>
        <h1 className="text-xl font-semibold">Branches</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {branchesQuery.data.length} branch{branchesQuery.data.length === 1 ? '' : 'es'} in this repository
        </p>
      </div>

      <form
        className="flex max-w-xl gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          const name = branchName.trim();
          if (name) {
            createBranch.mutate(name);
          }
        }}
      >
        <Input
          value={branchName}
          onChange={(event) => setBranchName(event.target.value)}
          placeholder="new-branch"
        />
        <Button type="submit" disabled={!branchName.trim() || createBranch.isPending}>
          <Plus className="mr-2 h-4 w-4" />
          New branch
        </Button>
      </form>

      <div className="overflow-hidden rounded-md border bg-card">
        <div className="border-b bg-[#f6f8fa] px-4 py-2 text-sm font-medium">Branch list</div>
        {branchesQuery.data.length === 0 ? (
          <div className="p-6 text-sm text-muted-foreground">No branches yet.</div>
        ) : (
          branchesQuery.data.map((branch) => (
            <div
              key={branch}
              className="grid grid-cols-[1fr_auto] items-center gap-4 border-b px-4 py-3 last:border-b-0"
            >
              <div className="flex min-w-0 items-center gap-3">
                <GitBranch className="h-4 w-4 shrink-0 text-muted-foreground" />
                <span className="truncate font-mono text-sm font-medium">{branch}</span>
                {branch === activeBranch && (
                  <span className="rounded-full border border-[#1a7f37]/30 bg-[#dafbe1] px-2 py-0.5 text-xs font-medium text-[#1a7f37]">
                    default
                  </span>
                )}
              </div>
              <span className="font-mono text-xs text-muted-foreground">
                {branch === activeBranch ? statusQuery.data?.head?.slice(0, 7) : 'remote/local'}
              </span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
