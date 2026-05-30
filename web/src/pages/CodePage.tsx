import { useEffect, useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ChevronRight, CircleDot, Code2, GitBranch, GitCommit, History, Layers3 } from 'lucide-react';
import { api, type TreeEntry } from '@/api/client';
import { CodeViewer } from '@/components/repository/CodeViewer';
import { FileExplorer } from '@/components/repository/FileExplorer';
import { useRepoContext } from '@/components/providers/repo-context';

function parentPath(path: string) {
  const parts = path.split('/').filter(Boolean);
  parts.pop();
  return parts.join('/');
}

export function CodePage() {
  const { selectedRepo, selectedRepoId } = useRepoContext();
  const [path, setPath] = useState('');
  const [selectedFile, setSelectedFile] = useState('');
  const [rev, setRev] = useState('');

  const statusQuery = useQuery({
    queryKey: ['status', selectedRepoId],
    queryFn: () => api.getStatus(selectedRepoId ?? undefined),
    enabled: !!selectedRepoId,
    retry: false,
  });
  const branchesQuery = useQuery({
    queryKey: ['branches', selectedRepoId],
    queryFn: () => api.getBranches(selectedRepoId ?? undefined),
    enabled: !!selectedRepoId,
    initialData: [],
  });
  const commitsQuery = useQuery({
    queryKey: ['commits', selectedRepoId],
    queryFn: () => api.getCommits(selectedRepoId ?? undefined),
    enabled: !!selectedRepoId,
    initialData: [],
  });
  const treeQuery = useQuery({
    queryKey: ['tree', selectedRepoId, path, rev],
    queryFn: () => api.getTree(selectedRepoId ?? undefined, path, rev || undefined),
    enabled: !!selectedRepoId,
    initialData: [],
  });

  useEffect(() => {
    setPath('');
    setSelectedFile('');
    setRev('');
  }, [selectedRepoId]);

  const activeRev = rev || statusQuery.data?.branch || branchesQuery.data[0] || 'main';
  const latestCommit = commitsQuery.data[0];
  const crumbs = useMemo(() => path.split('/').filter(Boolean), [path]);

  const sortedEntries = useMemo<TreeEntry[]>(() => {
    const entries = treeQuery.data ?? [];
    const folders = entries.filter((entry) => entry.kind === 'dir');
    const files = entries.filter((entry) => entry.kind === 'file');
    return [...folders, ...files];
  }, [treeQuery.data]);

  useEffect(() => {
    if (!sortedEntries.length) {
      setSelectedFile('');
      return;
    }

    if (selectedFile && sortedEntries.some((entry) => entry.path === selectedFile)) {
      return;
    }

    const preferredFile =
      sortedEntries.find((entry) => entry.kind === 'file' && entry.name.toLowerCase().startsWith('readme')) ??
      sortedEntries.find((entry) => entry.kind === 'file');

    setSelectedFile(preferredFile?.path ?? '');
  }, [selectedFile, sortedEntries]);

  if (!selectedRepoId) {
    return (
      <div className="mx-auto flex h-full max-w-6xl items-center justify-center p-6">
        <div className="w-full max-w-xl rounded-2xl border border-dashed border-[#d0d7de] bg-white p-10 text-center shadow-sm">
          <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-full bg-[#ddf4ff] text-[#0969da]">
            <Code2 className="h-6 w-6" />
          </div>
          <h1 className="mt-5 text-2xl font-semibold text-[#24292f]">Open a repository to browse code</h1>
          <p className="mt-2 text-sm text-[#57606a]">
            Create or register a repository in `Source Control`, then Aura Hub will render the file tree,
            branches, commits and source preview here.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-full bg-[#f6f8fa]">
      <div className="mx-auto flex max-w-7xl flex-col gap-6 px-6 py-6">
        <section className="rounded-2xl border border-[#d0d7de] bg-white p-6 shadow-sm">
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <h1 className="truncate text-2xl font-semibold text-[#24292f]">{selectedRepo?.name}</h1>
                <span className="rounded-full border border-[#d8dee4] bg-[#f6f8fa] px-2.5 py-1 text-xs text-[#57606a]">
                  {selectedRepo?.storage === 'hosted' ? 'Hosted on Aura Hub' : 'Linked workspace'}
                </span>
              </div>
              <p className="mt-2 text-sm text-[#57606a]">{selectedRepo?.path}</p>
            </div>

            <label className="inline-flex items-center gap-2 rounded-lg border border-[#d0d7de] bg-white px-3 py-2 text-sm text-[#24292f]">
              <GitBranch className="h-4 w-4 text-[#57606a]" />
              <select
                className="min-w-32 bg-transparent outline-none"
                value={activeRev}
                onChange={(event) => setRev(event.target.value)}
              >
                {branchesQuery.data.map((branch) => (
                  <option key={branch} value={branch}>
                    {branch}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <div className="mt-6 grid gap-4 lg:grid-cols-4">
            <div className="rounded-xl border border-[#d8dee4] bg-[#f6f8fa] p-4">
              <div className="flex items-center gap-2 text-sm font-medium text-[#24292f]">
                <GitBranch className="h-4 w-4 text-[#57606a]" />
                Active branch
              </div>
              <div className="mt-3 text-2xl font-semibold text-[#0969da]">{statusQuery.data?.branch ?? activeRev}</div>
              <p className="mt-1 text-xs text-[#57606a]">Browsing repository state from the selected ref.</p>
            </div>

            <div className="rounded-xl border border-[#d8dee4] bg-[#f6f8fa] p-4">
              <div className="flex items-center gap-2 text-sm font-medium text-[#24292f]">
                <GitCommit className="h-4 w-4 text-[#57606a]" />
                Commits
              </div>
              <div className="mt-3 text-2xl font-semibold text-[#24292f]">{commitsQuery.data.length}</div>
              <p className="mt-1 text-xs text-[#57606a]">First-parent history available from the Rust API.</p>
            </div>

            <div className="rounded-xl border border-[#d8dee4] bg-[#f6f8fa] p-4">
              <div className="flex items-center gap-2 text-sm font-medium text-[#24292f]">
                <Layers3 className="h-4 w-4 text-[#57606a]" />
                Entries in view
              </div>
              <div className="mt-3 text-2xl font-semibold text-[#24292f]">{sortedEntries.length}</div>
              <p className="mt-1 text-xs text-[#57606a]">Folders and files inside the current tree path.</p>
            </div>

            <div className="rounded-xl border border-[#d8dee4] bg-[#f6f8fa] p-4">
              <div className="flex items-center gap-2 text-sm font-medium text-[#24292f]">
                <CircleDot className="h-4 w-4 text-[#57606a]" />
                Working tree
              </div>
              <div className="mt-3 text-sm font-semibold text-[#24292f]">
                {statusQuery.data &&
                statusQuery.data.staged.length === 0 &&
                statusQuery.data.unstaged.length === 0 &&
                statusQuery.data.untracked.length === 0
                  ? 'Clean'
                  : 'Changes pending'}
              </div>
              <p className="mt-1 text-xs text-[#57606a]">
                {statusQuery.data
                  ? `${statusQuery.data.staged.length} staged, ${statusQuery.data.unstaged.length} modified, ${statusQuery.data.untracked.length} untracked`
                  : 'Status unavailable'}
              </p>
            </div>
          </div>
        </section>

        <section className="rounded-xl border border-[#d0d7de] bg-white px-4 py-3 shadow-sm">
          <div className="flex flex-wrap items-center gap-1 text-sm text-[#57606a]">
            <button className="rounded px-2 py-1 font-medium text-[#0969da] hover:bg-[#ddf4ff]" onClick={() => setPath('')}>
              {selectedRepo?.name}
            </button>
            {crumbs.map((crumb, index) => {
              const nextPath = crumbs.slice(0, index + 1).join('/');
              return (
                <span key={nextPath} className="inline-flex items-center gap-1">
                  <ChevronRight className="h-4 w-4" />
                  <button className="rounded px-2 py-1 hover:bg-[#f6f8fa]" onClick={() => setPath(nextPath)}>
                    {crumb}
                  </button>
                </span>
              );
            })}
          </div>
        </section>

        <div className="grid min-h-0 gap-6 xl:grid-cols-[360px_minmax(0,1fr)]">
          <div className="space-y-6">
            <FileExplorer
              branchLabel={activeRev}
              currentPath={path}
              entries={sortedEntries}
              selectedFile={selectedFile}
              onNavigateUp={() => setPath(parentPath(path))}
              onOpenDirectory={(nextPath) => {
                setPath(nextPath);
                setSelectedFile('');
              }}
              onSelectFile={setSelectedFile}
            />

            <section className="rounded-xl border border-[#d0d7de] bg-white p-4 shadow-sm">
              <div className="flex items-center gap-2 text-sm font-semibold text-[#24292f]">
                <History className="h-4 w-4 text-[#57606a]" />
                Latest commit
              </div>
              {latestCommit ? (
                <>
                  <div className="mt-3 text-base font-semibold text-[#24292f]">{latestCommit.message}</div>
                  <div className="mt-2 text-sm text-[#57606a]">{latestCommit.author}</div>
                  <div className="mt-1 font-mono text-xs text-[#57606a]">{latestCommit.hash}</div>
                </>
              ) : (
                <p className="mt-3 text-sm text-[#57606a]">This repository has no commits yet.</p>
              )}
            </section>
          </div>

          <CodeViewer repoId={selectedRepoId} path={selectedFile} rev={rev || undefined} />
        </div>
      </div>
    </div>
  );
}
