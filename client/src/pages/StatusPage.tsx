import React, { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '@/api/client';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import { Card, CardHeader, CardTitle, CardContent, CardFooter } from '@/components/ui/card';
import { ScrollArea } from '@/components/ui/scroll-area';
import { FilePlus, FileMinus, FileEdit, CheckCircle2 } from 'lucide-react';

export function StatusPage() {
  const queryClient = useQueryClient();
  const [message, setMessage] = useState('');

  const { data: status, isLoading } = useQuery({ queryKey: ['status'], queryFn: api.getStatus });

  const commitMutation = useMutation({
    mutationFn: api.commit,
    onSuccess: () => {
      setMessage('');
      queryClient.invalidateQueries({ queryKey: ['status'] });
      queryClient.invalidateQueries({ queryKey: ['commits'] });
    },
  });

  if (isLoading) {
    return <div className="p-8 text-muted-foreground">Loading status...</div>;
  }

  const handleCommit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!message.trim()) return;
    commitMutation.mutate(message);
  };

  const hasChanges = status && (status.staged.length > 0 || status.unstaged.length > 0 || status.untracked.length > 0);

  return (
    <div className="h-full flex flex-col p-6 max-w-4xl mx-auto">
      <div className="mb-6">
        <h1 className="text-2xl font-bold tracking-tight">Repository Status</h1>
        <p className="text-muted-foreground">Working tree status and commit form.</p>
      </div>

      {!hasChanges ? (
        <Card className="flex flex-col items-center justify-center p-12 text-center bg-muted/20 border-dashed">
          <CheckCircle2 className="h-12 w-12 text-green-500 mb-4" />
          <CardTitle className="mb-2">Working tree clean</CardTitle>
          <p className="text-muted-foreground">Nothing to commit, working tree clean.</p>
        </Card>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6 flex-1 min-h-0">
          <div className="flex flex-col gap-6 overflow-hidden">
            <Card className="flex-1 flex flex-col min-h-0">
              <CardHeader className="py-4">
                <CardTitle className="text-sm font-semibold">Changes</CardTitle>
              </CardHeader>
              <CardContent className="flex-1 p-0 overflow-hidden">
                <ScrollArea className="h-full px-6 pb-6">
                  <div className="space-y-6">
                    {status.staged.length > 0 && (
                      <div>
                        <h4 className="text-xs font-semibold text-green-500 mb-2 uppercase tracking-wider">Staged</h4>
                        <ul className="space-y-1">
                          {status.staged.map(f => (
                            <li key={f} className="text-sm flex items-center gap-2 text-foreground/80 bg-green-500/10 px-2 py-1 rounded">
                              <FileEdit className="h-3 w-3 text-green-500" /> {f}
                            </li>
                          ))}
                        </ul>
                      </div>
                    )}
                    
                    {status.unstaged.length > 0 && (
                      <div>
                        <h4 className="text-xs font-semibold text-blue-500 mb-2 uppercase tracking-wider">Modified (Unstaged)</h4>
                        <ul className="space-y-1">
                          {status.unstaged.map(f => (
                            <li key={f} className="text-sm flex items-center gap-2 text-foreground/80 bg-blue-500/10 px-2 py-1 rounded">
                              <FileMinus className="h-3 w-3 text-blue-500" /> {f}
                            </li>
                          ))}
                        </ul>
                      </div>
                    )}

                    {status.untracked.length > 0 && (
                      <div>
                        <h4 className="text-xs font-semibold text-red-500 mb-2 uppercase tracking-wider">Untracked</h4>
                        <ul className="space-y-1">
                          {status.untracked.map(f => (
                            <li key={f} className="text-sm flex items-center gap-2 text-foreground/80 bg-red-500/10 px-2 py-1 rounded">
                              <FilePlus className="h-3 w-3 text-red-500" /> {f}
                            </li>
                          ))}
                        </ul>
                      </div>
                    )}
                  </div>
                </ScrollArea>
              </CardContent>
            </Card>
          </div>

          <Card className="flex flex-col h-fit">
            <form onSubmit={handleCommit}>
              <CardHeader>
                <CardTitle>Create Commit</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Commit Message</label>
                  <Textarea 
                    placeholder="Enter commit message..." 
                    value={message}
                    onChange={e => setMessage(e.target.value)}
                    rows={4}
                    disabled={!hasChanges}
                  />
                </div>
              </CardContent>
              <CardFooter>
                <Button 
                  type="submit" 
                  className="w-full" 
                  disabled={!message.trim() || !hasChanges || commitMutation.isPending}
                >
                  {commitMutation.isPending ? 'Committing...' : 'Commit Changes'}
                </Button>
              </CardFooter>
            </form>
          </Card>
        </div>
      )}
    </div>
  );
}
