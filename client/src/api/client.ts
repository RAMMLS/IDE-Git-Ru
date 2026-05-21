import axios from 'axios';

export const apiClient = axios.create({
  baseURL: 'http://localhost:3000/api/repo', // Note: User specified `http://localhost:3000/api` in prompt but `project_memory` states `/api/repo/`. Let's use `/api/repo/` or just `/api` based on memory. I'll use `http://localhost:3000/api/repo`.
  headers: {
    'Content-Type': 'application/json',
  },
});

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
}

export const api = {
  getCommits: async () => {
    const { data } = await apiClient.get<Commit[]>('/log');
    return data;
  },
  getCommitDiff: async (hash: string) => {
    const { data } = await apiClient.get<FileDiff[]>(`/diff/${hash}`);
    return data;
  },
  getStatus: async () => {
    const { data } = await apiClient.get<Status>('/status');
    return data;
  },
  commit: async (message: string) => {
    const { data } = await apiClient.post('/commit', { message });
    return data;
  },
  getBranches: async () => {
    const { data } = await apiClient.get<string[]>('/branches');
    return data;
  },
  createBranch: async (name: string) => {
    const { data } = await apiClient.post('/branches', { name });
    return data;
  }
};
