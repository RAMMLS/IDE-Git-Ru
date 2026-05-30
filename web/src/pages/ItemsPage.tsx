import { useState } from 'react';
import { formatDistanceToNow } from 'date-fns';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { GitPullRequest, Plus, CircleDot } from 'lucide-react';
import { api, type HubItem } from '@/api/client';
import { useRepoContext } from '@/components/providers/repo-context';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';

type ItemKind = 'issues' | 'pull-requests';

interface ItemsPageProps {
  kind: ItemKind;
}

const config = {
  issues: {
    title: 'Issues',
    empty: 'No issues are open.',
    create: 'New issue',
    icon: CircleDot,
    queryKey: 'issues',
  },
  'pull-requests': {
    title: 'Pull Requests',
    empty: 'No pull requests are open.',
    create: 'New pull request',
    icon: GitPullRequest,
    queryKey: 'pull-requests',
  },
};

function createdAgo(item: HubItem) {
  return formatDistanceToNow(new Date(item.created_at * 1000), { addSuffix: true });
}

export function ItemsPage({ kind }: ItemsPageProps) {
  const queryClient = useQueryClient();
  const { selectedRepoId } = useRepoContext();
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');
  const itemConfig = config[kind];
  const Icon = itemConfig.icon;

  const itemsQuery = useQuery({
    queryKey: [itemConfig.queryKey, selectedRepoId],
    queryFn: () =>
      kind === 'issues'
        ? api.getIssues(selectedRepoId ?? undefined)
        : api.getPullRequests(selectedRepoId ?? undefined),
    enabled: !!selectedRepoId,
    initialData: [],
  });
  const createItem = useMutation({
    mutationFn: () =>
      kind === 'issues'
        ? api.createIssue({ title: title.trim(), body: body.trim() }, selectedRepoId ?? undefined)
        : api.createPullRequest({ title: title.trim(), body: body.trim() }, selectedRepoId ?? undefined),
    onSuccess: () => {
      setTitle('');
      setBody('');
      queryClient.invalidateQueries({ queryKey: [itemConfig.queryKey, selectedRepoId] });
    },
  });

  if (!selectedRepoId) {
    return <div className="p-8 text-muted-foreground">Create or select a repository in Source Control.</div>;
  }

  return (
    <div className="mx-auto grid h-full max-w-6xl grid-cols-1 gap-6 p-6 lg:grid-cols-[1fr_360px]">
      <div className="min-w-0">
        <div className="mb-4 flex items-center justify-between gap-3">
          <div>
            <h1 className="text-xl font-semibold">{itemConfig.title}</h1>
            <p className="mt-1 text-sm text-muted-foreground">{itemsQuery.data.length} open</p>
          </div>
        </div>

        <div className="overflow-hidden rounded-md border bg-card">
          <div className="border-b bg-[#f6f8fa] px-4 py-2 text-sm font-medium">Open</div>
          {itemsQuery.data.length === 0 ? (
            <div className="p-8 text-center text-sm text-muted-foreground">{itemConfig.empty}</div>
          ) : (
            itemsQuery.data.map((item) => (
              <div key={item.id} className="flex gap-3 border-b px-4 py-4 last:border-b-0">
                <Icon className="mt-0.5 h-5 w-5 shrink-0 text-[#1a7f37]" />
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-medium">{item.title}</span>
                    <span className="rounded-full border px-2 py-0.5 text-xs text-muted-foreground">
                      #{item.id}
                    </span>
                  </div>
                  {item.body && <p className="mt-1 line-clamp-2 text-sm text-muted-foreground">{item.body}</p>}
                  <p className="mt-2 text-xs text-muted-foreground">
                    opened {createdAgo(item)} by {item.author}
                  </p>
                </div>
              </div>
            ))
          )}
        </div>
      </div>

      <form
        className="h-fit rounded-md border bg-card p-4"
        onSubmit={(event) => {
          event.preventDefault();
          if (title.trim()) {
            createItem.mutate();
          }
        }}
      >
        <h2 className="mb-3 text-sm font-semibold">{itemConfig.create}</h2>
        <div className="space-y-3">
          <Input value={title} onChange={(event) => setTitle(event.target.value)} placeholder="Title" />
          <Textarea
            value={body}
            onChange={(event) => setBody(event.target.value)}
            placeholder="Description"
            className="min-h-32"
          />
          <Button type="submit" className="w-full" disabled={!title.trim() || createItem.isPending}>
            <Plus className="mr-2 h-4 w-4" />
            Create
          </Button>
        </div>
      </form>
    </div>
  );
}
