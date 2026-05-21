import { useQuery } from '@tanstack/react-query';
import { format } from 'date-fns';
import { useNavigate } from 'react-router-dom';
import { api } from '@/api/client';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Card } from '@/components/ui/card';
import { GitCommit } from 'lucide-react';

export function CommitsPage() {
  const navigate = useNavigate();
  const { data: commits, isLoading } = useQuery({ queryKey: ['commits'], queryFn: api.getCommits });

  if (isLoading) {
    return <div className="p-8 text-muted-foreground">Loading commits...</div>;
  }

  if (!commits || commits.length === 0) {
    return <div className="p-8 text-muted-foreground">No commits found.</div>;
  }

  return (
    <div className="h-full flex flex-col p-6">
      <div className="mb-6">
        <h1 className="text-2xl font-bold tracking-tight">Commit History</h1>
        <p className="text-muted-foreground">Recent changes in the repository.</p>
      </div>

      <Card className="flex-1 overflow-hidden flex flex-col">
        <ScrollArea className="flex-1">
          <div className="p-6 relative">
            <div className="absolute left-9 top-0 bottom-0 w-px bg-border" />
            
            <div className="space-y-6">
              {commits.map((commit) => (
                <div key={commit.hash} className="relative flex gap-6 group">
                  <div className="flex flex-col items-center">
                    <div className="h-6 w-6 rounded-full border-2 border-primary bg-background flex items-center justify-center z-10 group-hover:scale-110 transition-transform cursor-pointer"
                         onClick={() => navigate(`/diff/${commit.hash}`)}>
                      <GitCommit className="h-3 w-3 text-primary" />
                    </div>
                  </div>
                  
                  <div className="flex-1 pb-2">
                    <div 
                      className="p-4 rounded-lg border bg-card hover:border-primary/50 transition-colors cursor-pointer shadow-sm"
                      onClick={() => navigate(`/diff/${commit.hash}`)}
                    >
                      <div className="flex justify-between items-start mb-2">
                        <h3 className="font-medium text-foreground">{commit.message}</h3>
                        <span className="text-xs font-mono bg-secondary px-2 py-1 rounded text-secondary-foreground">
                          {commit.hash.substring(0, 7)}
                        </span>
                      </div>
                      <div className="flex items-center text-xs text-muted-foreground space-x-4">
                        <span className="font-medium">{commit.author}</span>
                        <span>{format(new Date(commit.date), 'MMM d, yyyy HH:mm')}</span>
                      </div>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </ScrollArea>
      </Card>
    </div>
  );
}
