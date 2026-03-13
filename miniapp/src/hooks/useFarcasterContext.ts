import { useEffect, useState } from 'react';
import { initFarcaster, sdk } from '../lib/farcaster';

interface FarcasterUser {
  fid: number | null;
  username: string | null;
  displayName: string | null;
}

export function useFarcasterContext() {
  const [user, setUser] = useState<FarcasterUser>({
    fid: null,
    username: null,
    displayName: null,
  });
  const [isReady, setIsReady] = useState(false);

  useEffect(() => {
    initFarcaster().then(() => {
      setIsReady(true);
      const ctx = sdk.context;
      if (ctx?.user) {
        setUser({
          fid: ctx.user.fid ?? null,
          username: ctx.user.username ?? null,
          displayName: ctx.user.displayName ?? null,
        });
      }
    });
  }, []);

  return { user, isReady };
}
