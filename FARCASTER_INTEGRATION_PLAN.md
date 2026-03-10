# Farcaster Integration Plan for Beacon

## New Module Structure

```
src/farcaster/
  mod.rs              # Module root
  bot.rs              # Mention polling, command parsing, scan orchestration
  neynar.rs           # Neynar API client (fetch mentions, post casts)
  github_scanner.rs   # Fetch repos via GitHub API → build RepoContext
  api.rs              # Axum handlers for Mini App API endpoints

miniapp/              # NEW — separate JS/TS frontend
  package.json
  vite.config.ts
  public/.well-known/farcaster.json   # Mini App manifest
  src/
    App.tsx
    components/
      AgentSearch.tsx
      AgentCard.tsx
      GenerateForm.tsx
      ValidateForm.tsx
      PaymentFlow.tsx
    hooks/
      useFarcasterContext.ts
      useBeaconApi.ts
    lib/
      api.ts
      farcaster.ts
```

---

## Phase 1: Foundation (shared by all 3 features)

| Step | What | Why |
|------|------|-----|
| 1.1 | Create `src/farcaster/mod.rs`, add `mod farcaster` to `main.rs` | Module scaffold |
| 1.2 | Build `src/farcaster/github_scanner.rs` | Both bot and Mini App need to scan remote repos. Uses GitHub API to build `RepoContext` (same struct as `scan_local()`) — fetches README, package manifest, up to 50 source files under 50KB |
| 1.3 | Add `agent_manifests` table to Supabase | Stores generated manifests for search/browse. Columns: `id, run_id, name, description, manifest_json (jsonb), capabilities_count, endpoints_count, on_chain_id, fid, created_at` |
| 1.4 | Add `/api/agents` (GET, search/list) and `/api/agents/:id` (GET) endpoints | Mini App and bot both need to query stored manifests |

---

## Phase 2: Farcaster Bot

| Step | What | Details |
|------|------|---------|
| 2.1 | Build `src/farcaster/neynar.rs` | `reqwest` client against `api.neynar.com/v2`. Key functions: `fetch_mentions(fid, cursor)`, `post_cast(text, parent_hash, channel_id)` |
| 2.2 | Build `src/farcaster/bot.rs` | Tokio task polling mentions every 30s. Parses commands: `scan <url>`, `validate <url>`, `help`. Calls `github_scanner` → `inferrer::infer_capabilities()` → `generator::generate()`. Chunks output into threaded replies (1024 char limit per cast) |
| 2.3 | Add `Wrap` event listener | Second Tokio task using ethers-rs to watch `Wrap(address,uint256,uint256)` events on `BeaconAgency` contract. Posts new registrations to `/beacon-agents` channel |
| 2.4 | Add `FarcasterBot` CLI subcommand | `beacon farcaster-bot --poll-interval 30 --channel beacon-agents` |
| 2.5 | Add `farcaster_scans` table | Tracks: `id, cast_hash, github_url, status, agents_md, reply_hash, created_at`. Prevents re-scanning the same mention |

### Environment Variables

```
NEYNAR_API_KEY        — Neynar API key
NEYNAR_SIGNER_UUID    — Managed signer UUID for the bot
FARCASTER_BOT_FID     — Bot's Farcaster ID
GITHUB_TOKEN          — Optional, for higher GitHub API rate limits
```

### Bot Data Models

```rust
pub struct BotConfig {
    pub neynar_api_key: String,
    pub signer_uuid: String,        // Neynar managed signer for posting
    pub bot_fid: u64,               // The bot's Farcaster ID
    pub channel_id: String,         // e.g. "beacon-agents"
    pub poll_interval_secs: u64,    // e.g. 30
    pub agency_address: String,     // BeaconAgency contract address
}

pub enum BotCommand {
    Scan { github_url: String },
    Validate { github_url: String },
    Help,
    Unknown,
}
```

### Command Parsing

Parse cast text after "@beacon" mention:
- `scan github.com/user/repo` → `BotCommand::Scan`
- `validate github.com/user/repo` → `BotCommand::Validate`
- `help` → `BotCommand::Help`
- Anything else → `BotCommand::Unknown` (reply with help text)

### Cast Threading Strategy (1024 char limit)

1. First reply: agent name, description, capability count, endpoint count
2. Subsequent replies: 2-3 capabilities per cast with names and one-line descriptions
3. Final reply: link to full AGENTS.md or "use `beacon generate` for full output"

### On-Chain Event Broadcasting

Uses `ethers_providers::Provider` to watch for `Wrap(address,uint256,uint256)` events on `BeaconAgency` at `0xd8b934580fcE35a11B58C6D73aDeE468a2833fa8`. Posts to `/beacon-agents` channel:

```
New agent identity registered on Base!
Token ID: {tokenId}
Owner: {to}
Cost: {premium} wei
View: https://basescan.org/tx/{txHash}
```

---

## Phase 3: Mini App + Farcaster Wallet Payments

| Step | What | Details |
|------|------|---------|
| 3.1 | Init `miniapp/` | Vite + React + TypeScript. Deps: `@farcaster/frame-sdk`, `viem`, `@tanstack/react-query` |
| 3.2 | Farcaster SDK setup | `sdk.actions.ready()` on load. `useFarcasterContext` hook exposes FID, username, wallet |
| 3.3 | Browse/Search UI | `AgentSearch` (search bar + grid) and `AgentCard` (capabilities, endpoints, on-chain link) components hitting `/api/agents` |
| 3.4 | Generate & Validate forms | Paste GitHub URL → calls Beacon API. Handle 402 response by triggering payment flow |
| 3.5 | Payment flow (`PaymentFlow.tsx`) | On 402: read `x-payment-run-id` and `x-payment-address-base` headers → use `sdk.wallet.ethProvider` + `viem` to send USDC transfer → resubmit with payment headers. **No changes to `verifier.rs`** |
| 3.6 | Serve Mini App from Axum | Add `tower-http` crate. Serve `miniapp/dist/` as static files. Add CORS layer for Warpcast webview |
| 3.7 | Mini App manifest | `public/.well-known/farcaster.json` with name, icon, homeUrl, webhookUrl |
| 3.8 | Add webhook + payment status endpoints | `/api/farcaster/webhook` (lifecycle events) + `/api/payment/status/:run_id` (polling UI) |

### Payment Flow Details

1. User triggers `generate` or `validate` in the Mini App
2. Frontend calls `POST /generate` on Beacon API
3. Beacon returns `402 Payment Required` with `x-payment-run-id`, `x-payment-amount` (0.09), `x-payment-address-base`
4. Frontend initiates USDC transfer via Farcaster wallet:
   - USDC contract: `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` (Base)
   - Amount: `90000` (= $0.09, USDC has 6 decimals)
5. User approves in Farcaster wallet prompt
6. Frontend resubmits with `x-payment-txn-hash`, `x-payment-chain: base`, `x-payment-run-id`
7. Beacon verifies via existing `verifier::verify_base()` — no backend changes needed

### Payment Flow Pseudocode

```typescript
import { sdk } from '@farcaster/frame-sdk';
import { createWalletClient, custom, encodeFunctionData } from 'viem';
import { base } from 'viem/chains';

const USDC_ADDRESS = '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913';

async function payForBeacon(recipientAddress: string, amount: number) {
  const provider = sdk.wallet.ethProvider;
  const client = createWalletClient({ chain: base, transport: custom(provider) });
  const [address] = await client.getAddresses();

  const data = encodeFunctionData({
    abi: ERC20_ABI,
    functionName: 'transfer',
    args: [recipientAddress, BigInt(amount * 1e6)],
  });

  const txHash = await client.sendTransaction({
    account: address,
    to: USDC_ADDRESS,
    data,
  });

  return txHash;
}
```

### Mini App UI Flow

1. **Home Screen**: Search bar + grid of recently registered agents
2. **Agent Detail**: Full capabilities list, endpoints table, on-chain identity link
3. **Generate Tab**: Paste GitHub URL, select provider, submit. Payment flow if needed. Result inline.
4. **Validate Tab**: Paste AGENTS.md content, submit. Validation report displayed.

### Mini App Manifest (`public/.well-known/farcaster.json`)

```json
{
  "accountAssociation": { "..." : "..." },
  "frame": {
    "version": "next",
    "name": "Beacon",
    "iconUrl": "https://beacon.example.com/icon.png",
    "homeUrl": "https://beacon.example.com/miniapp",
    "splashImageUrl": "https://beacon.example.com/splash.png",
    "splashBackgroundColor": "#000000",
    "webhookUrl": "https://beacon.example.com/api/farcaster/webhook"
  }
}
```

---

## Phase 4: Polish

- Rate limiting for bot (debounce duplicate scans, respect Neynar's 300 req/min)
- Redis caching for agent manifest search results
- Error handling + retries for Neynar API calls
- Monitoring/logging for bot process

---

## Cargo.toml Additions

```toml
url = "2"                                                    # URL parsing for bot
tower-http = { version = "0.5", features = ["fs", "cors"] } # Static serving + CORS
```

---

## New Database Tables

### `agent_manifests` (searchable index for Mini App)

| Column | Type | Purpose |
|--------|------|---------|
| id | uuid | Primary key |
| run_id | text | FK to runs table |
| name | text | Agent name (indexed for search) |
| description | text | Agent description |
| manifest_json | jsonb | Full AgentsManifest as JSON |
| capabilities_count | int | For display |
| endpoints_count | int | For display |
| on_chain_id | text | ERC-7527 token ID if registered |
| fid | bigint | Farcaster FID of user who generated it |
| created_at | timestamptz | Auto |

### `farcaster_scans` (bot dedup + tracking)

| Column | Type | Purpose |
|--------|------|---------|
| id | uuid | Primary key |
| cast_hash | text | Farcaster cast hash that triggered the scan |
| github_url | text | Repo URL scanned |
| status | text | pending/scanning/complete/failed |
| agents_md | text | Generated output |
| reply_hash | text | Hash of the bot's reply cast |
| created_at | timestamptz | Auto |

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| `scan_local()` only works on local paths | `github_scanner.rs` builds same `RepoContext` from GitHub API |
| Cast limit = 1024 chars, AGENTS.md is long | Bot posts summary in threaded replies |
| Farcaster wallet = Base only | Beacon already supports Base USDC |
| Mini App needs HTTPS | Beacon Cloud already serves HTTPS |
| Neynar API rate limits (300 req/min free) | 30s poll interval + batched replies stays well under limit |
| `identity.rs` ABI mismatch with actual contract | Only need event listening for bot, not calling `wrap` |

---

## Critical Existing Files

- `src/main.rs` — Add `FarcasterBot` subcommand, new API routes, CORS, static serving
- `src/models.rs` — `RepoContext` and `AgentsManifest` types that github_scanner must produce
- `src/verifier.rs` — Existing Base USDC verification, unchanged by Farcaster wallet flow
- `src/db.rs` — Add `agent_manifests` and `farcaster_scans` table functions
- `src/inferrer.rs` — `infer_capabilities(&RepoContext, ...)` is the integration point between github_scanner and inference pipeline
