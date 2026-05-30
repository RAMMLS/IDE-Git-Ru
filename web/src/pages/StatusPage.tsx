import React, { useState } from 'react';
import axios from 'axios';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/api/client';
import { useRepoContext } from '@/components/providers/repo-context';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card';
import { ScrollArea } from '@/components/ui/scroll-area';
import {
  CheckCircle2,
  Download,
  FileEdit,
  FileMinus,
  FilePlus,
  FolderGit2,
  Link2,
  Upload,
} from 'lucide-react';

function getErrorMessage(error: unknown) {
  if (axios.isAxiosError(error)) {
    return error.response?.data?.error ?? error.message ?? 'Request failed.';
  }

  if (error instanceof Error) {
    return error.message;
  }

  return 'Unknown error.';
}

export function StatusPage() {
  const queryClient = useQueryClient();
  const { repos, selectedRepo, selectedRepoId, setSelectedRepoId, isLoading: isReposLoading } = useRepoContext();
  const [repoName, setRepoName] = useState('');
  const [repoPath, setRepoPath] = useState('');
  const [message, setMessage] = useState('');
  const [remoteName, setRemoteName] = useState('origin');
  const [remoteTarget, setRemoteTarget] = useState('');
  const [syncBranch, setSyncBranch] = useState('');
  const [syncMessage, setSyncMessage] = useState<string | null>(null);

  const statusQuery = useQuery({
    queryKey: ['status', selectedRepoId],
    queryFn: () => api.getStatus(selectedRepoId ?? undefined),
    enabled: !!selectedRepoId,
    retry: false,
  });
  const remotesQuery = useQuery({
    queryKey: ['remotes', selectedRepoId],
    queryFn: () => api.getRemotes(selectedRepoId ?? undefined),
    enabled: !!selectedRepoId,
    initialData: [],
    retry: false,
  });

  const status = statusQuery.data;
  const remotes = remotesQuery.data ?? [];
  const activeRemote = remotes.find((remote) => remote.name === remoteName) ?? remotes[0];

  const repoMutation = useMutation({
    mutationFn: api.upsertRepo,
    onSuccess: (repo) => {
      setSelectedRepoId(repo.id);
      setRepoName('');
      setRepoPath('');
      setSyncMessage(
        repo.storage === 'hosted'
          ? `Hosted repository ${repo.name} created.`
          : `Repository ${repo.name} registered.`,
      );
      queryClient.invalidateQueries({ queryKey: ['repos'] });
    },
  });

  const commitMutation = useMutation({
    mutationFn: (commitMessage: string) => api.commit(commitMessage, selectedRepoId ?? undefined),
    onSuccess: () => {
      setMessage('');
      setSyncMessage('Commit created successfully.');
      queryClient.invalidateQueries({ queryKey: ['status', selectedRepoId] });
      queryClient.invalidateQueries({ queryKey: ['commits', selectedRepoId] });
    },
  });

  const remoteMutation = useMutation({
    mutationFn: (payload: { name: string; target: string }) =>
      api.saveRemote(payload, selectedRepoId ?? undefined),
    onSuccess: (remote) => {
      setRemoteName(remote.name);
      setRemoteTarget(remote.target);
      setSyncMessage(`Remote ${remote.name} saved.`);
      queryClient.invalidateQueries({ queryKey: ['remotes', selectedRepoId] });
    },
  });

  const pushMutation = useMutation({
    mutationFn: (payload: { remote?: string; branch?: string }) =>
      api.push(payload, selectedRepoId ?? undefined),
    onSuccess: (result) => {
      setSyncMessage(`Pushed ${result.branch} to ${result.remote}.`);
      queryClient.invalidateQueries({ queryKey: ['commits', selectedRepoId] });
      queryClient.invalidateQueries({ queryKey: ['status', selectedRepoId] });
      queryClient.invalidateQueries({ queryKey: ['remotes', selectedRepoId] });
    },
  });

  const pullMutation = useMutation({
    mutationFn: (payload: { remote?: string; branch?: string }) =>
      api.pull(payload, selectedRepoId ?? undefined),
    onSuccess: (result) => {
      setSyncMessage(
        result.status === 'already_up_to_date'
          ? `${result.local_branch} is already up to date.`
          : `Pulled ${result.source_branch} from ${result.remote}.`,
      );
      queryClient.invalidateQueries({ queryKey: ['commits', selectedRepoId] });
      queryClient.invalidateQueries({ queryKey: ['status', selectedRepoId] });
      queryClient.invalidateQueries({ queryKey: ['branches', selectedRepoId] });
      queryClient.invalidateQueries({ queryKey: ['remotes', selectedRepoId] });
    },
  });

  const handleRepoSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    if (!repoName.trim()) {
      return;
    }

    setSyncMessage(null);
    repoMutation.mutate({
      name: repoName.trim(),
      path: repoPath.trim() || undefined,
    });
  };

  const handleCommit = (event: React.FormEvent) => {
    event.preventDefault();
    if (!selectedRepoId || !message.trim()) {
      return;
    }

    setSyncMessage(null);
    commitMutation.mutate(message.trim());
  };

  const handleSaveRemote = (event: React.FormEvent) => {
    event.preventDefault();
    if (!selectedRepoId) {
      return;
    }

    const target = remoteTarget.trim() || activeRemote?.target || '';
    if (!remoteName.trim() || !target) {
      return;
    }

    setSyncMessage(null);
    remoteMutation.mutate({
      name: remoteName.trim(),
      target,
    });
  };

  const handlePush = () => {
    if (!selectedRepoId || !remoteName.trim()) {
      return;
    }

    setSyncMessage(null);
    pushMutation.mutate({
      remote: remoteName.trim(),
      branch: syncBranch.trim() || status?.branch,
    });
  };

  const handlePull = () => {
    if (!selectedRepoId || !remoteName.trim()) {
      return;
    }

    setSyncMessage(null);
    pullMutation.mutate({
      remote: remoteName.trim(),
      branch: syncBranch.trim() || status?.branch,
    });
  };

  if (isReposLoading && repos.length === 0) {
    return <div className="p-8 text-muted-foreground">Loading repositories...</div>;
  }

  const hasChanges = !!status && (status.staged.length > 0 || status.unstaged.length > 0 || status.untracked.length > 0);
  const activeMutationError =
    repoMutation.error ??
    commitMutation.error ??
    remoteMutation.error ??
    pushMutation.error ??
    pullMutation.error;

  return (
    <div className="h-full flex flex-col p-6">
      <div className="mb-6">
        <h1 className="text-2xl font-bold tracking-tight">Repository Status</h1>
        <p className="text-muted-foreground">
          Manage hosted repositories, register linked repos and sync remotes from the UI.
        </p>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-3 gap-6 flex-1 min-h-0">
        <div className="xl:col-span-2 flex flex-col gap-6 overflow-hidden">
          <Card>
            <form onSubmit={handleRepoSubmit}>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <FolderGit2 className="h-4 w-4" />
                  Server Repositories
                </CardTitle>
                <CardDescription>
                  Leave path empty to create a hosted Aura repository on the server, or provide a local path to register an existing repo.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="grid gap-4 md:grid-cols-2">
                  <div className="space-y-2">
                    <label className="text-sm font-medium">Repository Name</label>
                    <Input
                      placeholder="demo"
                      value={repoName}
                      onChange={(event) => setRepoName(event.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">Existing Path (Optional)</label>
                    <Input
                      placeholder="/absolute/path/to/aura-repo"
                      value={repoPath}
                      onChange={(event) => setRepoPath(event.target.value)}
                    />
                  </div>
                </div>
                {repos.length > 0 && (
                  <div className="rounded-md border bg-muted/30 p-3 text-sm text-muted-foreground">
                    Available repositories: {repos.map((repo) => `${repo.name} (${repo.storage})`).join(', ')}
                  </div>
                )}
              </CardContent>
              <CardFooter>
                <Button type="submit" disabled={!repoName.trim() || repoMutation.isPending}>
                  {repoMutation.isPending
                    ? 'Saving...'
                    : repoPath.trim()
                      ? 'Register Existing Repo'
                      : 'Create Hosted Repo'}
                </Button>
              </CardFooter>
            </form>
          </Card>

          {selectedRepo ? (
            <Card>
              <CardHeader>
                <CardTitle>Active Repository</CardTitle>
                <CardDescription>
                  {selectedRepo.name} is currently selected in the server registry.
                </CardDescription>
              </CardHeader>
              <CardContent className="grid gap-3 text-sm md:grid-cols-3">
                <div className="rounded-md border bg-muted/20 p-3">
                  <div className="text-xs uppercase tracking-wide text-muted-foreground">Id</div>
                  <div className="mt-1 font-mono">{selectedRepo.id}</div>
                </div>
                <div className="rounded-md border bg-muted/20 p-3">
                  <div className="text-xs uppercase tracking-wide text-muted-foreground">Storage</div>
                  <div className="mt-1">{selectedRepo.storage}</div>
                </div>
                <div className="rounded-md border bg-muted/20 p-3 md:col-span-1">
                  <div className="text-xs uppercase tracking-wide text-muted-foreground">Path</div>
                  <div className="mt-1 break-all font-mono text-xs">{selectedRepo.path}</div>
                </div>
              </CardContent>
            </Card>
          ) : (
            <Card className="border-dashed bg-muted/20">
              <CardHeader>
                <CardTitle>No Repository Selected</CardTitle>
                <CardDescription>
                  Create a hosted repo or register an existing Aura repository to continue.
                </CardDescription>
              </CardHeader>
            </Card>
          )}

          {selectedRepoId && statusQuery.isError ? (
            <Card className="border-destructive/40 bg-destructive/5">
              <CardHeader>
                <CardTitle>Server connection failed</CardTitle>
                <CardDescription>{getErrorMessage(statusQuery.error)}</CardDescription>
              </CardHeader>
              <CardContent className="text-sm text-muted-foreground">
                Start the Rust server on port `3000` or use the Vite proxy from `web/vite.config.ts`.
              </CardContent>
            </Card>
          ) : selectedRepoId && statusQuery.isLoading ? (
            <div className="p-6 text-muted-foreground">Loading status...</div>
          ) : selectedRepoId ? (
            <>
              {hasChanges ? (
                <Card className="flex-1 flex flex-col min-h-0">
                  <CardHeader className="py-4">
                    <CardTitle className="text-sm font-semibold">Changes</CardTitle>
                    <CardDescription>
                      Current branch: {status?.branch || 'main'}
                    </CardDescription>
                  </CardHeader>
                  <CardContent className="flex-1 p-0 overflow-hidden">
                    <ScrollArea className="h-full px-6 pb-6">
                      <div className="space-y-6">
                        {status?.staged.length ? (
                          <div>
                            <h4 className="text-xs font-semibold text-green-500 mb-2 uppercase tracking-wider">Staged</h4>
                            <ul className="space-y-1">
                              {status.staged.map((file) => (
                                <li key={file} className="text-sm flex items-center gap-2 text-foreground/80 bg-green-500/10 px-2 py-1 rounded">
                                  <FileEdit className="h-3 w-3 text-green-500" /> {file}
                                </li>
                              ))}
                            </ul>
                          </div>
                        ) : null}

                        {status?.unstaged.length ? (
                          <div>
                            <h4 className="text-xs font-semibold text-blue-500 mb-2 uppercase tracking-wider">Modified</h4>
                            <ul className="space-y-1">
                              {status.unstaged.map((file) => (
                                <li key={file} className="text-sm flex items-center gap-2 text-foreground/80 bg-blue-500/10 px-2 py-1 rounded">
                                  <FileMinus className="h-3 w-3 text-blue-500" /> {file}
                                </li>
                              ))}
                            </ul>
                          </div>
                        ) : null}

                        {status?.untracked.length ? (
                          <div>
                            <h4 className="text-xs font-semibold text-red-500 mb-2 uppercase tracking-wider">Untracked</h4>
                            <ul className="space-y-1">
                              {status.untracked.map((file) => (
                                <li key={file} className="text-sm flex items-center gap-2 text-foreground/80 bg-red-500/10 px-2 py-1 rounded">
                                  <FilePlus className="h-3 w-3 text-red-500" /> {file}
                                </li>
                              ))}
                            </ul>
                          </div>
                        ) : null}
                      </div>
                    </ScrollArea>
                  </CardContent>
                </Card>
              ) : (
                <Card className="flex flex-col items-center justify-center p-12 text-center bg-muted/20 border-dashed">
                  <CheckCircle2 className="h-12 w-12 text-green-500 mb-4" />
                  <CardTitle className="mb-2">Working tree clean</CardTitle>
                  <p className="text-muted-foreground">Nothing to commit right now.</p>
                </Card>
              )}

              <Card>
                <form onSubmit={handleSaveRemote}>
                  <CardHeader>
                    <CardTitle className="flex items-center gap-2">
                      <Link2 className="h-4 w-4" />
                      Remote Repository
                    </CardTitle>
                    <CardDescription>
                      Configure an Aura remote path or hosted transport URL and sync the current branch.
                    </CardDescription>
                  </CardHeader>
                  <CardContent className="space-y-4">
                    <div className="space-y-2">
                      <label className="text-sm font-medium">Remote Name</label>
                      <Input
                        placeholder="origin"
                        value={remoteName}
                        onChange={(event) => setRemoteName(event.target.value)}
                      />
                    </div>
                    <div className="space-y-2">
                      <label className="text-sm font-medium">Remote Target</label>
                      <Input
                        placeholder="http://localhost:3000/api/transport/repos/demo or /absolute/path"
                        value={remoteTarget || activeRemote?.target || ''}
                        onChange={(event) => setRemoteTarget(event.target.value)}
                      />
                    </div>
                    <div className="space-y-2">
                      <label className="text-sm font-medium">Branch For Push/Pull</label>
                      <Input
                        placeholder={status?.branch || 'main'}
                        value={syncBranch}
                        onChange={(event) => setSyncBranch(event.target.value)}
                      />
                    </div>
                    {remotes.length > 0 && (
                      <div className="rounded-md border bg-muted/30 p-3 text-sm text-muted-foreground">
                        Known remotes: {remotes.map((remote) => `${remote.name} -> ${remote.target}`).join(', ')}
                      </div>
                    )}
                    {syncMessage && (
                      <div className="rounded-md border border-green-500/30 bg-green-500/10 px-3 py-2 text-sm text-green-400">
                        {syncMessage}
                      </div>
                    )}
                    {activeMutationError && (
                      <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
                        {getErrorMessage(activeMutationError)}
                      </div>
                    )}
                  </CardContent>
                  <CardFooter className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                    <Button
                      type="submit"
                      variant="outline"
                      disabled={!remoteName.trim() || !(remoteTarget.trim() || activeRemote?.target) || remoteMutation.isPending}
                    >
                      {remoteMutation.isPending ? 'Saving...' : 'Save Remote'}
                    </Button>
                    <Button
                      type="button"
                      onClick={handlePush}
                      disabled={!remoteName.trim() || pushMutation.isPending}
                    >
                      <Upload className="mr-2 h-4 w-4" />
                      {pushMutation.isPending ? 'Pushing...' : 'Push'}
                    </Button>
                    <Button
                      type="button"
                      variant="secondary"
                      onClick={handlePull}
                      disabled={!remoteName.trim() || pullMutation.isPending}
                    >
                      <Download className="mr-2 h-4 w-4" />
                      {pullMutation.isPending ? 'Pulling...' : 'Pull'}
                    </Button>
                  </CardFooter>
                </form>
              </Card>
            </>
          ) : null}
        </div>

        <div className="flex flex-col gap-6">
          <Card className="flex flex-col h-fit">
            <form onSubmit={handleCommit}>
              <CardHeader>
                <CardTitle>Create Commit</CardTitle>
                <CardDescription>
                  Aura stages the current worktree and creates a commit in one action.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Commit Message</label>
                  <Textarea
                    placeholder={selectedRepoId ? 'Enter commit message...' : 'Select a repository first'}
                    value={message}
                    onChange={(event) => setMessage(event.target.value)}
                    rows={6}
                    disabled={!hasChanges}
                  />
                </div>
              </CardContent>
              <CardFooter>
                <Button
                  type="submit"
                  className="w-full"
                  disabled={!selectedRepoId || !message.trim() || !hasChanges || commitMutation.isPending}
                >
                  {commitMutation.isPending ? 'Committing...' : 'Commit Changes'}
                </Button>
              </CardFooter>
            </form>
          </Card>
        </div>
      </div>
    </div>
  );
}
