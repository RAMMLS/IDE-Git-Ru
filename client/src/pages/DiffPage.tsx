import { useParams, useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { ArrowLeft, File as FileIcon } from 'lucide-react';
import { api } from '@/api/client';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card';

export function DiffPage() {
  const { hash } = useParams<{ hash: string }>();
  const navigate = useNavigate();

  const { data: diffs, isLoading } = useQuery({
    queryKey: ['diff', hash],
    queryFn: () => api.getCommitDiff(hash!),
    enabled: !!hash,
  });

  if (isLoading) {
    return <div className="p-8 text-muted-foreground">Loading diff...</div>;
  }

  return (
    <div className="h-full flex flex-col p-6">
      <div className="mb-6 flex items-center gap-4">
        <Button variant="outline" size="icon" onClick={() => navigate(-1)}>
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Commit Diff</h1>
          <p className="text-muted-foreground font-mono text-sm">{hash}</p>
        </div>
      </div>

      <ScrollArea className="flex-1 -mx-6 px-6">
        <div className="space-y-6 pb-6">
          {!diffs || diffs.length === 0 ? (
            <div className="text-center p-8 text-muted-foreground border rounded-lg bg-muted/20">
              No changes found in this commit.
            </div>
          ) : (
            diffs.map((file, idx) => (
              <Card key={idx} className="overflow-hidden">
                <CardHeader className="bg-muted/50 py-3 px-4 border-b flex flex-row items-center gap-2 space-y-0">
                  <FileIcon className="h-4 w-4 text-muted-foreground" />
                  <CardTitle className="text-sm font-medium flex items-center gap-2">
                    {file.path}
                    <span className={`text-[10px] uppercase px-1.5 py-0.5 rounded font-bold ${
                      file.status === 'added' ? 'bg-green-500/20 text-green-500' :
                      file.status === 'deleted' ? 'bg-red-500/20 text-red-500' :
                      'bg-blue-500/20 text-blue-500'
                    }`}>
                      {file.status}
                    </span>
                  </CardTitle>
                </CardHeader>
                <CardContent className="p-0">
                  <div className="overflow-x-auto font-mono text-xs p-4 bg-background">
                    {file.diff.split('\n').map((line, i) => {
                      const isAdd = line.startsWith('+') && !line.startsWith('+++');
                      const isDel = line.startsWith('-') && !line.startsWith('---');
                      const isHeader = line.startsWith('@@') || line.startsWith('+++') || line.startsWith('---');
                      
                      let className = 'px-2 py-0.5 whitespace-pre ';
                      if (isAdd) className += 'bg-green-500/10 text-green-600 dark:text-green-400';
                      else if (isDel) className += 'bg-red-500/10 text-red-600 dark:text-red-400';
                      else if (isHeader) className += 'text-blue-500 font-bold';
                      else className += 'text-muted-foreground';

                      return (
                        <div key={i} className={className}>
                          {line || ' '}
                        </div>
                      );
                    })}
                  </div>
                </CardContent>
              </Card>
            ))
          )}
        </div>
      </ScrollArea>
    </div>
  );
}
