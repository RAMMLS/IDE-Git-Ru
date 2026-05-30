import { useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';

export function useAuraEvents() {
  const queryClient = useQueryClient();

  useEffect(() => {
    let disposed = false;
    let reconnectTimer: number | undefined;

    const explicitUrl = import.meta.env.VITE_AURA_WS_URL?.trim();
    const fallbackUrl = `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}/api/events`;
    const wsUrl = explicitUrl || fallbackUrl;

    const invalidateRepoQueries = () => {
      queryClient.invalidateQueries({ queryKey: ['repos'] });
      queryClient.invalidateQueries({ queryKey: ['commits'] });
      queryClient.invalidateQueries({ queryKey: ['status'] });
      queryClient.invalidateQueries({ queryKey: ['branches'] });
      queryClient.invalidateQueries({ queryKey: ['remotes'] });
      queryClient.invalidateQueries({ queryKey: ['diff'] });
      queryClient.invalidateQueries({ queryKey: ['tree'] });
      queryClient.invalidateQueries({ queryKey: ['file'] });
      queryClient.invalidateQueries({ queryKey: ['issues'] });
      queryClient.invalidateQueries({ queryKey: ['pull-requests'] });
    };

    const connect = () => {
      const ws = new WebSocket(wsUrl);

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
            invalidateRepoQueries();
          }
        } catch (err) {
          console.error('Failed to parse websocket message', err);
        }
      };

      ws.onerror = () => {
        ws.close();
      };

      ws.onclose = () => {
        if (disposed) {
          return;
        }

        reconnectTimer = window.setTimeout(connect, 1000);
      };

      return ws;
    };

    const socket = connect();

    return () => {
      disposed = true;
      if (reconnectTimer) {
        window.clearTimeout(reconnectTimer);
      }
      socket.close();
    };
  }, [queryClient]);
}
