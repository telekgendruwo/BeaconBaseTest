const API_BASE = import.meta.env.VITE_API_URL || '';

export interface AgentManifest {
  id: string;
  run_id: string | null;
  name: string;
  description: string;
  manifest_json: any;
  capabilities_count: number;
  endpoints_count: number;
  on_chain_id: string | null;
  fid: number | null;
  created_at: string;
}

export interface GenerateResponse {
  manifest: any;
  agents_md: string;
  id: string | null;
}

export interface ValidationResult {
  valid: boolean;
  errors: string[];
  warnings: string[];
}

export async function searchAgents(query?: string, limit = 20, offset = 0) {
  const params = new URLSearchParams();
  if (query) params.set('q', query);
  params.set('limit', String(limit));
  params.set('offset', String(offset));

  const res = await fetch(`${API_BASE}/api/agents?${params}`);
  if (!res.ok) throw new Error(`Failed to fetch agents: ${res.statusText}`);
  return res.json() as Promise<{ agents: AgentManifest[]; count: number }>;
}

export async function getAgent(id: string) {
  const res = await fetch(`${API_BASE}/api/agents/${id}`);
  if (!res.ok) throw new Error(`Failed to fetch agent: ${res.statusText}`);
  return res.json() as Promise<AgentManifest>;
}

export async function generateManifest(
  githubUrl: string,
  provider = 'gemini',
  paymentHeaders?: Record<string, string>
): Promise<GenerateResponse> {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (paymentHeaders) Object.assign(headers, paymentHeaders);

  const res = await fetch(`${API_BASE}/api/generate`, {
    method: 'POST',
    headers,
    body: JSON.stringify({ github_url: githubUrl, provider }),
  });

  if (res.status === 402) {
    // Payment required — return headers for payment flow
    const error: any = new Error('Payment required');
    error.status = 402;
    error.runId = res.headers.get('x-payment-run-id');
    error.amount = res.headers.get('x-payment-amount');
    error.address = res.headers.get('x-payment-address-base');
    throw error;
  }

  if (!res.ok) throw new Error(`Generate failed: ${res.statusText}`);
  return res.json();
}

export async function validateContent(content: string): Promise<ValidationResult> {
  const res = await fetch(`${API_BASE}/api/validate`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ content }),
  });

  if (!res.ok) throw new Error(`Validate failed: ${res.statusText}`);
  return res.json();
}
