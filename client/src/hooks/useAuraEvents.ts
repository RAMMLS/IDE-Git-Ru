import { useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';

export function useAuraEvents() {
  const queryClient = useQueryClient();

  useEffect(() => {
    // According to the prompt: ws://localhost:3000/api/events
    const ws = new WebSocket('ws://localhost:3000/api/events');

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        console.log('WebSocket Event:', data);
        // Invalidate queries to refresh data when a change occurs
        if (data.type === 'COMMIT_ADDED' || data.type === 'STATUS_CHANGED') {
          queryClient.invalidateQueries({ queryKey: ['commits'] });
          queryClient.invalidateQueries({ queryKey: ['status'] });
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
