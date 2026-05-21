import { useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';

export function useAuraEvents() {
  const queryClient = useQueryClient();

  useEffect(() => {
    const explicitUrl = import.meta.env.VITE_AURA_WS_URL?.trim();
    const fallbackUrl = `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}/api/events`;
    const ws = new WebSocket(explicitUrl || fallbackUrl);

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);

        if (data.type === 'REPO_LIST_CHANGED') {
          queryClient.invalidateQueries({ queryKey: ['repos'] });
          return;
        }

        if (
          data.type === 'REPO_UPDATED' ||
          data.type === 'COMMIT_ADDED' ||
          data.type === 'STATUS_CHANGED'
        ) {
          queryClient.invalidateQueries({ queryKey: ['repos'] });
          queryClient.invalidateQueries({ queryKey: ['commits'] });
          queryClient.invalidateQueries({ queryKey: ['status'] });
          queryClient.invalidateQueries({ queryKey: ['branches'] });
          queryClient.invalidateQueries({ queryKey: ['remotes'] });
          queryClient.invalidateQueries({ queryKey: ['diff'] });
        }
      } catch (err) {
        console.error('Failed to parse websocket message', err);
      }
    };

    ws.onerror = (error) => {
      console.error('WebSocket Error:', error);
    };

    return () => {
      ws.close();
    };
  }, [queryClient]);
}
