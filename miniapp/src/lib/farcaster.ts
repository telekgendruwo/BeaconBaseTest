import { sdk } from '@farcaster/frame-sdk';

let initialized = false;

export async function initFarcaster() {
  if (initialized) return;
  await sdk.actions.ready();
  initialized = true;
}

export function getFarcasterContext() {
  return sdk.context;
}

export { sdk };
