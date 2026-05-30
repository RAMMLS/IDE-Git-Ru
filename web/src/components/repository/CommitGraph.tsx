import { formatDistanceToNowStrict } from 'date-fns';
import { GitBranch, GitCommit, GitMerge, Tag } from 'lucide-react';
import type { Commit } from '@/api/client';

const lanePalette = ['#1f6feb', '#8250df', '#d1242f', '#2da44e'];

function laneForRefs(refs: string[]) {
  if (!refs.length) {
    return 0;
  }

  const score = refs.join(':').split('').reduce((sum, char) => sum + char.charCodeAt(0), 0);
  return score % lanePalette.length;
}

interface CommitGraphProps {
  commits: Commit[];
  onSelectCommit?: (hash: string) => void;
}

export function CommitGraph({ commits, onSelectCommit }: CommitGraphProps) {
  return (
    <div className="space-y-4">
      {commits.map((commit, index) => {
        const lane = laneForRefs(commit.refs);
        const accent = lanePalette[lane];
        const hasRefs = commit.refs.length > 0;
        const isMerge = commit.parents.length > 1;

        return (
          <button
            key={commit.hash}
            className="group grid w-full grid-cols-[88px_minmax(0,1fr)] gap-4 rounded-xl border border-[#d0d7de] bg-white p-4 text-left shadow-sm transition hover:border-[#0969da] hover:shadow-md"
            onClick={() => onSelectCommit?.(commit.hash)}
          >
            <div className="relative flex justify-center">
              <svg
                width="72"
                height="112"
                viewBox="0 0 72 112"
                className="overflow-visible"
                aria-hidden="true"
              >
                {index !== 0 ? <line x1="36" y1="0" x2="36" y2="34" stroke="#d8dee4" strokeWidth="2" /> : null}
                <line x1="36" y1="78" x2="36" y2="112" stroke="#d8dee4" strokeWidth="2" />
                {hasRefs ? (
                  <>
                    <line
                      x1="36"
                      y1="56"
                      x2={16 + lane * 12}
                      y2="24"
                      stroke={accent}
                      strokeWidth="2"
                      strokeLinecap="round"
                    />
                    <circle cx={16 + lane * 12} cy="24" r="4" fill={accent} />
                  </>
                ) : null}
                {isMerge ? (
                  <path
                    d="M56 12 C 56 24, 48 32, 36 46"
                    fill="none"
                    stroke="#8250df"
                    strokeWidth="2"
                    strokeLinecap="round"
                  />
                ) : null}
                <circle cx="36" cy="56" r="11" fill="white" stroke={accent} strokeWidth="3" />
                <circle cx="36" cy="56" r="4" fill={accent} />
              </svg>
            </div>

            <div className="min-w-0">
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div className="min-w-0">
                  <div className="flex flex-wrap items-center gap-2">
                    <h3 className="truncate text-base font-semibold text-[#24292f]">{commit.message}</h3>
                    {isMerge ? (
                      <span className="inline-flex items-center gap-1 rounded-full border border-[#ddf4ff] bg-[#ddf4ff] px-2 py-0.5 text-[11px] font-medium text-[#0969da]">
                        <GitMerge className="h-3 w-3" />
                        merge
                      </span>
                    ) : null}
                  </div>
                  <div className="mt-2 flex flex-wrap items-center gap-3 text-sm text-[#57606a]">
                    <span>{commit.author}</span>
                    <span>{formatDistanceToNowStrict(new Date(commit.date), { addSuffix: true })}</span>
                    <span className="font-mono">{commit.hash.slice(0, 12)}</span>
                  </div>
                </div>

                <div className="flex flex-wrap items-center justify-end gap-2">
                  {commit.refs.map((ref) => (
                    <span
                      key={`${commit.hash}-${ref}`}
                      className="inline-flex items-center gap-1 rounded-full border border-[#d8dee4] bg-[#f6f8fa] px-2.5 py-1 text-[11px] font-medium text-[#24292f]"
                    >
                      <GitBranch className="h-3 w-3 text-[#57606a]" />
                      {ref}
                    </span>
                  ))}
                  <span className="inline-flex items-center gap-1 rounded-full border border-[#d8dee4] bg-white px-2.5 py-1 text-[11px] font-medium text-[#57606a]">
                    <Tag className="h-3 w-3" />
                    {commit.parents.length === 0 ? 'root' : `${commit.parents.length} parent${commit.parents.length > 1 ? 's' : ''}`}
                  </span>
                </div>
              </div>

              <div className="mt-4 flex items-center gap-2 text-xs text-[#57606a]">
                <GitCommit className="h-3.5 w-3.5" />
                Click to inspect the full patch for this commit.
              </div>
            </div>
          </button>
        );
      })}
    </div>
  );
}
