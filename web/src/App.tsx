import React from 'react';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AppLayout } from '@/components/layout/AppLayout';
import { RepoProvider } from '@/components/providers/repo-provider';
import { BranchesPage } from '@/pages/BranchesPage';
import { CodePage } from '@/pages/CodePage';
import { CommitsPage } from '@/pages/CommitsPage';
import { DiffPage } from '@/pages/DiffPage';
import { ItemsPage } from '@/pages/ItemsPage';
import { StatusPage } from '@/pages/StatusPage';
import { useAuraEvents } from '@/hooks/useAuraEvents';

const queryClient = new QueryClient();

function EventProvider({ children }: { children: React.ReactNode }) {
  useAuraEvents();
  return <>{children}</>;
}

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <RepoProvider>
        <EventProvider>
          <BrowserRouter>
            <Routes>
              <Route path="/" element={<AppLayout />}>
                <Route index element={<CodePage />} />
                <Route path="commits" element={<CommitsPage />} />
                <Route path="diff/:hash" element={<DiffPage />} />
                <Route path="branches" element={<BranchesPage />} />
                <Route path="pull-requests" element={<ItemsPage kind="pull-requests" />} />
                <Route path="issues" element={<ItemsPage kind="issues" />} />
                <Route path="source-control" element={<StatusPage />} />
              </Route>
            </Routes>
          </BrowserRouter>
        </EventProvider>
      </RepoProvider>
    </QueryClientProvider>
  );
}

export default App;
