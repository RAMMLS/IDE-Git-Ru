import { useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ChevronRight, Code2, File, Folder, GitBranch, History } from 'lucide-react';
import { api, type TreeEntry } from '@/api/client';
import { useRepoContext } from '@/components/providers/repo-context';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';

function parentPath(path: string) {
  const parts = path.split('/').filter(Boolean);
  parts.pop();
  return parts.join('/');
}

function formatBytes(size?: number | null) {
  if (!size) {
    return '';
  }
  if (size < 1024) {
    return `${size} B`;
  }
  return `${(size / 1024).toFixed(1)} KB`;
}

function lineTokens(line: string, language: string) {
  const keywords = new Set([
    'async',
    'await',
    'const',
    'enum',
    'fn',
    'function',
    'if',
    'impl',
    'import',
    'interface',
    'let',
    'match',
    'mod',
    'pub',
    'return',
    'struct',
    'type',
    'use',
  ]);
  const pattern = /(\/\/.*|#.*|"(?:\\.|[^"])*"|'(?:\\.|[^'])*'|\b\d+(?:\.\d+)?\b|\b[A-Za-z_][A-Za-z0-9_]*\b)/g;
  const nodes: ReactNode[] = [];
  let lastIndex = 0;

  for (const match of line.matchAll(pattern)) {
    const token = match[0];
    const index = match.index ?? 0;
    if (index > lastIndex) {
      nodes.push(line.slice(lastIndex, index));
    }

    let className = '';
    if (token.startsWith('//') || (language === 'python' && token.startsWith('#'))) {
      className = 'text-[#6e7781]';
    } else if (token.startsWith('"') || token.startsWith("'")) {
      className = 'text-[#0a3069]';
    } else if (/^\d/.test(token)) {
      className = 'text-[#0550ae]';
    } else if (keywords.has(token)) {
      className = 'text-[#cf222e] font-medium';
    } else if (/^[A-Z]/.test(token)) {
      className = 'text-[#8250df]';
    }

    nodes.push(
      className ? (
        <span key={`${index}-${token}`} className={className}>
          {token}
        </span>
      ) : (
        token
      ),
    );
    lastIndex = index + token.length;
  }

  if (lastIndex < line.length) {
    nodes.push(line.slice(lastIndex));
  }
  return nodes.length ? nodes : ' ';
}

function FileViewer({ repoId, path, rev }: { repoId: string; path: string; rev?: string }) {
  const fileQuery = useQuery({
    queryKey: ['file', repoId, path, rev],
    queryFn: () => api.getFile(repoId, path, rev),
    enabled: !!repoId && !!path,
  });

  if (fileQuery.isLoading) {
    return <div className="p-6 text-sm text-muted-foreground">Loading file...</div>;
  }

  if (!fileQuery.data) {
    return <div className="p-6 text-sm text-muted-foreground">Select a file to preview.</div>;
  }

  const lines = fileQuery.data.content.split('\n');

  return (
    <div className="overflow-hidden rounded-md border bg-card">
      <div className="flex items-center justify-between border-b bg-muted/40 px-4 py-2">
        <div className="flex min-w-0 items-center gap-2 text-sm font-medium">
          <File className="h-4 w-4 shrink-0 text-muted-foreground" />
          <span className="truncate">{fileQuery.data.path}</span>
        </div>
        <span className="font-mono text-xs text-muted-foreground">{fileQuery.data.oid.slice(0, 7)}</span>
      </div>
      <ScrollArea className="max-h-[58vh]">
        <pre className="min-w-full bg-white text-[13px] leading-5 text-[#24292f]">
          {lines.map((line, index) => (
            <div key={index} className="grid grid-cols-[64px_1fr] hover:bg-[#f6f8fa]">
              <span className="select-none border-r bg-[#f6f8fa] px-4 text-right text-[#6e7781]">
                {index + 1}
              </span>
              <code className="whitespace-pre px-4">{lineTokens(line, fileQuery.data.language)}</code>
            </div>
          ))}
        </pre>
      </ScrollArea>
    </div>
  );
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

  const activeRev = rev || statusQuery.data?.branch || 'main';
  const latestCommit = commitsQuery.data[0];
  const crumbs = useMemo(() => path.split('/').filter(Boolean), [path]);

  if (!selectedRepoId) {
    return <div className="p-8 text-muted-foreground">Create or select a repository in Source Control.</div>;
  }

  const entries = treeQuery.data ?? [];
  const folders = entries.filter((entry) => entry.kind === 'dir');
  const files = entries.filter((entry) => entry.kind === 'file');
  const sortedEntries: TreeEntry[] = [...folders, ...files];

  return (
    <div className="mx-auto flex h-full max-w-6xl flex-col gap-4 p-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold">{selectedRepo?.name}</h1>
          <div className="mt-1 flex flex-wrap items-center gap-3 text-sm text-muted-foreground">
            <span>{selectedRepo?.storage === 'hosted' ? 'Hosted Aura repository' : 'Linked repository'}</span>
            {latestCommit && (
              <span className="inline-flex items-center gap-1">
                <History className="h-4 w-4" />
                {latestCommit.message}
              </span>
            )}
          </div>
        </div>
        <label className="inline-flex items-center gap-2 rounded-md border bg-background px-3 py-2 text-sm">
          <GitBranch className="h-4 w-4 text-muted-foreground" />
          <select
            className="bg-transparent outline-none"
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

      <div className="flex flex-wrap items-center gap-1 rounded-md border bg-[#f6f8fa] px-3 py-2 text-sm">
        <Button variant="ghost" size="sm" className="h-7 px-2" onClick={() => setPath('')}>
          {selectedRepo?.name}
        </Button>
        {crumbs.map((crumb, index) => {
          const nextPath = crumbs.slice(0, index + 1).join('/');
          return (
            <span key={nextPath} className="inline-flex items-center gap-1">
              <ChevronRight className="h-4 w-4 text-muted-foreground" />
              <Button variant="ghost" size="sm" className="h-7 px-2" onClick={() => setPath(nextPath)}>
                {crumb}
              </Button>
            </span>
          );
        })}
      </div>

      <div className="grid min-h-0 flex-1 gap-4 lg:grid-cols-[minmax(320px,420px)_1fr]">
        <div className="overflow-hidden rounded-md border bg-card">
          <div className="border-b bg-[#f6f8fa] px-4 py-2 text-sm font-medium">
            {latestCommit ? (
              <span className="line-clamp-1">{latestCommit.message}</span>
            ) : (
              <span>No commits yet</span>
            )}
          </div>
          <ScrollArea className="h-[calc(100vh-290px)] min-h-[360px]">
            {path && (
              <button
                className="grid w-full grid-cols-[24px_1fr_auto] items-center gap-3 border-b px-4 py-2 text-left text-sm hover:bg-muted/40"
                onClick={() => setPath(parentPath(path))}
              >
                <Folder className="h-4 w-4 text-[#54aeff]" />
                <span>..</span>
                <span />
              </button>
            )}
            {sortedEntries.map((entry) => (
              <button
                key={entry.path}
                className="grid w-full grid-cols-[24px_1fr_auto] items-center gap-3 border-b px-4 py-2 text-left text-sm hover:bg-muted/40"
                onClick={() => {
                  if (entry.kind === 'dir') {
                    setPath(entry.path);
                    setSelectedFile('');
                  } else {
                    setSelectedFile(entry.path);
                  }
                }}
              >
                {entry.kind === 'dir' ? (
                  <Folder className="h-4 w-4 text-[#54aeff]" />
                ) : (
                  <File className="h-4 w-4 text-muted-foreground" />
                )}
                <span className="truncate font-medium">{entry.name}</span>
                <span className="text-xs text-muted-foreground">{formatBytes(entry.size)}</span>
              </button>
            ))}
            {sortedEntries.length === 0 && (
              <div className="p-6 text-sm text-muted-foreground">This directory is empty.</div>
            )}
          </ScrollArea>
        </div>

        {selectedFile ? (
          <FileViewer repoId={selectedRepoId} path={selectedFile} rev={rev || undefined} />
        ) : (
          <div className="flex min-h-[360px] items-center justify-center rounded-md border bg-[#f6f8fa] text-sm text-muted-foreground">
            <div className="flex items-center gap-2">
              <Code2 className="h-4 w-4" />
              Select a file to open it.
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
