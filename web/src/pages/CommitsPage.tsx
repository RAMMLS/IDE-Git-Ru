import { useQuery } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { GitBranch, History, Workflow } from 'lucide-react';
import { api } from '@/api/client';
import { CommitGraph } from '@/components/repository/CommitGraph';
import { useRepoContext } from '@/components/providers/repo-context';

export function CommitsPage() {
  const navigate = useNavigate();
  const { selectedRepo, selectedRepoId } = useRepoContext();
  const { data: commits, isLoading } = useQuery({
    queryKey: ['commits', selectedRepoId],
    queryFn: () => api.getCommits(selectedRepoId ?? undefined),
    enabled: !!selectedRepoId,
  });

  if (!selectedRepoId) {
    return (
      <div className="mx-auto flex h-full max-w-5xl items-center justify-center p-6">
        <div className="w-full max-w-xl rounded-2xl border border-dashed border-[#d0d7de] bg-white p-10 text-center shadow-sm">
          <History className="mx-auto h-12 w-12 text-[#0969da]" />
          <h1 className="mt-4 text-2xl font-semibold text-[#24292f]">No repository selected</h1>
          <p className="mt-2 text-sm text-[#57606a]">
            Create or register a repository in `Source Control` to inspect commit history and branch graph.
          </p>
        </div>
      </div>
    );
  }

  if (isLoading) {
    return <div className="p-8 text-[#57606a]">Loading commits...</div>;
  }

  if (!commits || commits.length === 0) {
    return <div className="p-8 text-[#57606a]">No commits found.</div>;
  }

  const refsCount = commits.reduce((sum, commit) => sum + commit.refs.length, 0);

  return (
    <div className="min-h-full bg-[#f6f8fa]">
      <div className="mx-auto flex max-w-6xl flex-col gap-6 px-6 py-6">
        <section className="rounded-2xl border border-[#d0d7de] bg-white p-6 shadow-sm">
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div>
              <h1 className="text-2xl font-semibold text-[#24292f]">Commit history</h1>
              <p className="mt-2 text-sm text-[#57606a]">
                Visual timeline for `{selectedRepo?.name ?? selectedRepoId}` backed by Aura server data.
              </p>
            </div>
            <div className="inline-flex rounded-full border border-[#d8dee4] bg-[#f6f8fa] px-3 py-1.5 text-xs font-medium text-[#57606a]">
              First-parent traversal
            </div>
          </div>

          <div className="mt-6 grid gap-4 md:grid-cols-3">
            <div className="rounded-xl border border-[#d8dee4] bg-[#f6f8fa] p-4">
              <div className="flex items-center gap-2 text-sm font-medium text-[#24292f]">
                <History className="h-4 w-4 text-[#57606a]" />
                Loaded commits
              </div>
              <div className="mt-3 text-2xl font-semibold text-[#24292f]">{commits.length}</div>
            </div>
            <div className="rounded-xl border border-[#d8dee4] bg-[#f6f8fa] p-4">
              <div className="flex items-center gap-2 text-sm font-medium text-[#24292f]">
                <GitBranch className="h-4 w-4 text-[#57606a]" />
                Branch labels
              </div>
              <div className="mt-3 text-2xl font-semibold text-[#24292f]">{refsCount}</div>
            </div>
            <div className="rounded-xl border border-[#d8dee4] bg-[#f6f8fa] p-4">
              <div className="flex items-center gap-2 text-sm font-medium text-[#24292f]">
                <Workflow className="h-4 w-4 text-[#57606a]" />
                Merge commits
              </div>
              <div className="mt-3 text-2xl font-semibold text-[#24292f]">
                {commits.filter((commit) => commit.parents.length > 1).length}
              </div>
            </div>
          </div>
        </section>

        <section className="rounded-2xl border border-[#d0d7de] bg-[#f6f8fa] p-2 shadow-sm">
          <CommitGraph commits={commits} onSelectCommit={(hash) => navigate(`/diff/${hash}`)} />
        </section>
      </div>
    </div>
  );
}
