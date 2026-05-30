import {
  ChevronRight,
  FileCode2,
  FileJson2,
  FileText,
  Folder,
  FolderOpen,
  Hash,
  Image,
  ScrollText,
} from 'lucide-react';
import type { TreeEntry } from '@/api/client';
import { formatFileSize } from '@/lib/code';

function iconForEntry(entry: TreeEntry, selectedFile?: string) {
  if (entry.kind === 'dir') {
    return selectedFile?.startsWith(`${entry.path}/`) ? FolderOpen : Folder;
  }

  const extension = entry.name.split('.').pop()?.toLowerCase();
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(extension ?? '')) {
    return Image;
  }
  if (['rs', 'ts', 'tsx', 'js', 'jsx', 'css', 'html', 'sh'].includes(extension ?? '')) {
    return FileCode2;
  }
  if (['json', 'yml', 'yaml', 'toml'].includes(extension ?? '')) {
    return FileJson2;
  }
  if (['md', 'txt'].includes(extension ?? '')) {
    return ScrollText;
  }
  return FileText;
}

interface FileExplorerProps {
  branchLabel: string;
  currentPath: string;
  entries: TreeEntry[];
  selectedFile?: string;
  onNavigateUp: () => void;
  onOpenDirectory: (path: string) => void;
  onSelectFile: (path: string) => void;
}

export function FileExplorer({
  branchLabel,
  currentPath,
  entries,
  selectedFile,
  onNavigateUp,
  onOpenDirectory,
  onSelectFile,
}: FileExplorerProps) {
  return (
    <section className="overflow-hidden rounded-xl border border-[#d0d7de] bg-white shadow-sm">
      <div className="flex items-center justify-between border-b border-[#d8dee4] bg-[#f6f8fa] px-4 py-3">
        <div>
          <p className="text-sm font-semibold text-[#24292f]">File Explorer</p>
          <p className="text-xs text-[#57606a]">
            Branch `{branchLabel}` {currentPath ? `in ${currentPath}` : 'at repository root'}
          </p>
        </div>
        <div className="inline-flex items-center gap-2 rounded-full border border-[#d8dee4] bg-white px-3 py-1 text-xs text-[#57606a]">
          <Hash className="h-3.5 w-3.5" />
          {entries.length} entries
        </div>
      </div>

      <div className="divide-y divide-[#d8dee4]">
        {currentPath ? (
          <button
            className="grid w-full grid-cols-[24px_1fr_auto] items-center gap-3 px-4 py-2.5 text-left text-sm text-[#24292f] transition hover:bg-[#f6f8fa]"
            onClick={onNavigateUp}
          >
            <Folder className="h-4 w-4 text-[#54aeff]" />
            <span className="font-medium">..</span>
            <ChevronRight className="h-4 w-4 text-[#8c959f]" />
          </button>
        ) : null}

        {entries.map((entry) => {
          const Icon = iconForEntry(entry, selectedFile);
          const isActive = entry.path === selectedFile;
          const isDirectory = entry.kind === 'dir';

          return (
            <button
              key={entry.path}
              className={`grid w-full grid-cols-[24px_1fr_auto] items-center gap-3 px-4 py-2.5 text-left text-sm transition ${
                isActive
                  ? 'bg-[#ddf4ff] text-[#0969da]'
                  : 'text-[#24292f] hover:bg-[#f6f8fa]'
              }`}
              onClick={() => (isDirectory ? onOpenDirectory(entry.path) : onSelectFile(entry.path))}
            >
              <Icon className={`h-4 w-4 ${isDirectory ? 'text-[#54aeff]' : isActive ? 'text-[#0969da]' : 'text-[#6e7781]'}`} />
              <div className="min-w-0">
                <div className="truncate font-medium">{entry.name}</div>
                <div className="truncate text-xs text-[#6e7781]">{entry.path}</div>
              </div>
              <div className="text-right text-xs text-[#6e7781]">
                {isDirectory ? 'Folder' : formatFileSize(entry.size)}
              </div>
            </button>
          );
        })}

        {entries.length === 0 ? (
          <div className="px-4 py-10 text-center text-sm text-[#57606a]">
            This directory is empty for the selected revision.
          </div>
        ) : null}
      </div>
    </section>
  );
}
