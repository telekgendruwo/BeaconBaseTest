import { useState } from 'react';
import { useGenerate } from '../hooks/useBeaconApi';
import { PaymentFlow } from './PaymentFlow';

export function GenerateForm() {
  const [githubUrl, setGithubUrl] = useState('');
  const [provider, setProvider] = useState('gemini');
  const [paymentInfo, setPaymentInfo] = useState<{
    runId: string;
    amount: string;
    address: string;
  } | null>(null);

  const generate = useGenerate();

  const handleSubmit = async (e: React.FormEvent, paymentHeaders?: Record<string, string>) => {
    e.preventDefault();
    if (!githubUrl.trim()) return;

    try {
      await generate.mutateAsync({ githubUrl, provider, paymentHeaders });
      setPaymentInfo(null);
    } catch (err: any) {
      if (err.status === 402) {
        setPaymentInfo({
          runId: err.runId,
          amount: err.amount,
          address: err.address,
        });
      }
    }
  };

  const handlePaymentComplete = async (txHash: string) => {
    if (!paymentInfo) return;
    // Resubmit with payment headers
    const paymentHeaders = {
      'x-payment-txn-hash': txHash,
      'x-payment-chain': 'base',
      'x-payment-run-id': paymentInfo.runId,
    };
    try {
      await generate.mutateAsync({ githubUrl, provider, paymentHeaders });
      setPaymentInfo(null);
    } catch {
      // Handle error
    }
  };

  return (
    <div className="generate-form">
      <h2>Generate AGENTS.md</h2>
      <form onSubmit={handleSubmit}>
        <div className="form-group">
          <label htmlFor="github-url">GitHub Repository URL</label>
          <input
            id="github-url"
            type="text"
            value={githubUrl}
            onChange={(e) => setGithubUrl(e.target.value)}
            placeholder="github.com/user/repo"
            className="form-input"
          />
        </div>
        <div className="form-group">
          <label htmlFor="provider">AI Provider</label>
          <select
            id="provider"
            value={provider}
            onChange={(e) => setProvider(e.target.value)}
            className="form-select"
          >
            <option value="gemini">Gemini</option>
            <option value="claude">Claude</option>
          </select>
        </div>
        <button
          type="submit"
          className="form-button"
          disabled={generate.isPending || !githubUrl.trim()}
        >
          {generate.isPending ? 'Generating...' : 'Generate'}
        </button>
      </form>

      {paymentInfo && (
        <PaymentFlow
          runId={paymentInfo.runId}
          amount={paymentInfo.amount}
          recipientAddress={paymentInfo.address}
          onComplete={handlePaymentComplete}
          onCancel={() => setPaymentInfo(null)}
        />
      )}

      {generate.isSuccess && (
        <div className="result">
          <h3>Generated: {generate.data.manifest.name}</h3>
          <pre className="agents-md-output">{generate.data.agents_md}</pre>
        </div>
      )}

      {generate.isError && !paymentInfo && (
        <p className="error">Error: {(generate.error as Error).message}</p>
      )}
    </div>
  );
}
