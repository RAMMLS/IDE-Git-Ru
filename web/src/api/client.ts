import axios from 'axios';

const explicitBaseUrl = import.meta.env.VITE_AURA_API_URL?.trim();
const normalizedBaseUrl = explicitBaseUrl
  ? explicitBaseUrl.replace(/\/+$/, '').replace(/\/repo$/, '')
  : '/api';

export const apiClient = axios.create({
  baseURL: normalizedBaseUrl,
  headers: {
    'Content-Type': 'application/json',
  },
});

function repoParams(repoId?: string) {
  return repoId ? { params: { repo: repoId } } : undefined;
}

function withRepoParams(repoId?: string, params: Record<string, string | undefined> = {}) {
  return {
    params: {
      ...(repoId ? { repo: repoId } : {}),
      ...Object.fromEntries(Object.entries(params).filter(([, value]) => value !== undefined && value !== '')),
    },
  };
}

export interface Commit {
  hash: string;
  author: string;
  message: string;
  date: string;
}

export interface FileDiff {
  path: string;
  status: 'added' | 'modified' | 'deleted' | 'renamed';
  diff: string;
}

export interface Status {
  staged: string[];
  unstaged: string[];
  untracked: string[];
  branch: string;
  head?: string | null;
}

export interface TreeEntry {
  name: string;
  path: string;
  kind: 'dir' | 'file';
  oid: string;
  size?: number | null;
}

export interface RepoFile {
  path: string;
  oid: string;
  language: string;
  content: string;
}

export interface RemoteConfig {
  name: string;
  target: string;
}

export interface PushSummary {
  remote: string;
  branch: string;
  hash: string;
  target: string;
}

export interface FileStat {
  path: string;
  kind: 'added' | 'modified' | 'deleted';
  insertions: number;
  deletions: number;
}

export interface ChangeStats {
  files_changed: number;
  insertions: number;
  deletions: number;
  files: FileStat[];
}

export interface PullSummary {
  remote: string;
  source_branch: string;
  local_branch: string;
  hash: string;
  previous_hash?: string | null;
  status: 'already_up_to_date' | 'fast_forward';
  target: string;
  stats?: ChangeStats | null;
}

export interface RepoRecord {
  id: string;
  name: string;
  path: string;
  storage: 'hosted' | 'linked';
}

export interface RepoUpsertPayload {
  name: string;
  path?: string;
}

export interface HubItem {
  id: number;
  title: string;
  body: string;
  status: 'open' | 'closed';
  author: string;
  created_at: number;
}

export interface HubItemPayload {
  title: string;
  body?: string;
}

export const api = {
  listRepos: async () => {
    const { data } = await apiClient.get<RepoRecord[]>('/repos');
    return data;
  },
  upsertRepo: async (payload: RepoUpsertPayload) => {
    const { data } = await apiClient.post<RepoRecord>('/repos', payload);
    return data;
  },
  getCommits: async (repoId?: string) => {
    const { data } = await apiClient.get<Commit[]>('/repo/log', repoParams(repoId));
    return data;
  },
  getCommitDiff: async (hash: string, repoId?: string) => {
    const { data } = await apiClient.get<FileDiff[]>(`/repo/diff/${hash}`, repoParams(repoId));
    return data;
  },
  getTree: async (repoId?: string, path?: string, rev?: string) => {
    const { data } = await apiClient.get<TreeEntry[]>('/repo/tree', withRepoParams(repoId, { path, rev }));
    return data;
  },
  getFile: async (repoId: string | undefined, path: string, rev?: string) => {
    const { data } = await apiClient.get<RepoFile>('/repo/file', withRepoParams(repoId, { path, rev }));
    return data;
  },
  getStatus: async (repoId?: string) => {
    const { data } = await apiClient.get<Status>('/repo/status', repoParams(repoId));
    return data;
  },
  commit: async (message: string, repoId?: string) => {
    const { data } = await apiClient.post('/repo/commit', { message }, repoParams(repoId));
    return data;
  },
  getBranches: async (repoId?: string) => {
    const { data } = await apiClient.get<string[]>('/repo/branches', repoParams(repoId));
    return data;
  },
  createBranch: async (name: string, repoId?: string) => {
    const { data } = await apiClient.post('/repo/branches', { name }, repoParams(repoId));
    return data;
  },
  getRemotes: async (repoId?: string) => {
    const { data } = await apiClient.get<RemoteConfig[]>('/repo/remotes', repoParams(repoId));
    return data;
  },
  saveRemote: async (payload: RemoteConfig, repoId?: string) => {
    const { data } = await apiClient.post<RemoteConfig>('/repo/remotes', payload, repoParams(repoId));
    return data;
  },
  push: async (payload: { remote?: string; branch?: string }, repoId?: string) => {
    const { data } = await apiClient.post<PushSummary>('/repo/push', payload, repoParams(repoId));
    return data;
  },
  pull: async (payload: { remote?: string; branch?: string }, repoId?: string) => {
    const { data } = await apiClient.post<PullSummary>('/repo/pull', payload, repoParams(repoId));
    return data;
  },
  getIssues: async (repoId?: string) => {
    const { data } = await apiClient.get<HubItem[]>('/repo/issues', repoParams(repoId));
    return data;
  },
  createIssue: async (payload: HubItemPayload, repoId?: string) => {
    const { data } = await apiClient.post<HubItem>('/repo/issues', payload, repoParams(repoId));
    return data;
  },
  getPullRequests: async (repoId?: string) => {
    const { data } = await apiClient.get<HubItem[]>('/repo/pull-requests', repoParams(repoId));
    return data;
  },
  createPullRequest: async (payload: HubItemPayload, repoId?: string) => {
    const { data } = await apiClient.post<HubItem>('/repo/pull-requests', payload, repoParams(repoId));
    return data;
  },
};
