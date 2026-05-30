import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Binary, Clock3, FileCode2, GitCommit, Quote } from 'lucide-react';
import { api } from '@/api/client';
import { formatFileSize, highlightSource } from '@/lib/code';

interface CodeViewerProps {
  repoId: string;
  path: string;
  rev?: string;
}

export function CodeViewer({ repoId, path, rev }: CodeViewerProps) {
  const fileQuery = useQuery({
    queryKey: ['file', repoId, path, rev],
    queryFn: () => api.getFile(repoId, path, rev),
    enabled: !!repoId && !!path,
  });

  const rendered = useMemo(() => {
    if (!fileQuery.data) {
      return null;
    }

    const { html, language } = highlightSource(fileQuery.data.content, fileQuery.data.language, fileQuery.data.path);
    const lines = html.split('\n');

    return {
      language,
      lines,
      size: new TextEncoder().encode(fileQuery.data.content).length,
    };
  }, [fileQuery.data]);

  if (fileQuery.isLoading) {
    return (
      <div className="flex min-h-[420px] items-center justify-center rounded-xl border border-[#d0d7de] bg-white text-sm text-[#57606a] shadow-sm">
        Loading file contents...
      </div>
    );
  }

  if (!fileQuery.data || !rendered) {
    return (
      <div className="flex min-h-[420px] items-center justify-center rounded-xl border border-dashed border-[#d0d7de] bg-[#f6f8fa] text-sm text-[#57606a]">
        Select a file to preview its source.
      </div>
    );
  }

  const lineCount = rendered.lines.length;

  return (
    <section className="overflow-hidden rounded-xl border border-[#d0d7de] bg-white shadow-sm">
      <div className="border-b border-[#d8dee4] bg-[#f6f8fa] px-5 py-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <div className="flex items-center gap-2 text-sm font-semibold text-[#24292f]">
              <FileCode2 className="h-4 w-4 text-[#57606a]" />
              <span>{fileQuery.data.path}</span>
            </div>
            <div className="mt-2 flex flex-wrap items-center gap-2 text-xs text-[#57606a]">
              <span className="inline-flex items-center gap-1 rounded-full border border-[#d8dee4] bg-white px-2.5 py-1">
                <Quote className="h-3.5 w-3.5" />
                {rendered.language}
              </span>
              <span className="inline-flex items-center gap-1 rounded-full border border-[#d8dee4] bg-white px-2.5 py-1">
                <Binary className="h-3.5 w-3.5" />
                {formatFileSize(rendered.size)}
              </span>
              <span className="inline-flex items-center gap-1 rounded-full border border-[#d8dee4] bg-white px-2.5 py-1">
                <Clock3 className="h-3.5 w-3.5" />
                {lineCount} lines
              </span>
            </div>
          </div>

          <div className="inline-flex items-center gap-1 rounded-full border border-[#d8dee4] bg-white px-3 py-1.5 font-mono text-xs text-[#57606a]">
            <GitCommit className="h-3.5 w-3.5" />
            {fileQuery.data.oid.slice(0, 12)}
          </div>
        </div>
      </div>

      <div className="overflow-auto bg-[#f6f8fa]">
        <div className="min-w-full font-mono text-[12.5px] leading-6">
          {rendered.lines.map((line, index) => (
            <div key={`${fileQuery.data.oid}-${index}`} className="grid grid-cols-[72px_minmax(0,1fr)]">
              <div className="select-none border-r border-[#d8dee4] bg-[#f6f8fa] px-4 text-right text-[#6e7781]">
                {index + 1}
              </div>
              <code
                className="overflow-x-auto whitespace-pre px-4 text-[#24292f]"
                dangerouslySetInnerHTML={{ __html: line || ' ' }}
              />
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
