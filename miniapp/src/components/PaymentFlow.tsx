import { useState } from 'react';
import { sdk } from '@farcaster/frame-sdk';
import { createWalletClient, custom, encodeFunctionData, parseAbi } from 'viem';
import { base } from 'viem/chains';

const USDC_ADDRESS = '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913' as const;

const ERC20_ABI = parseAbi([
  'function transfer(address to, uint256 amount) returns (bool)',
]);

interface PaymentFlowProps {
  runId: string;
  amount: string; // e.g., "0.09"
  recipientAddress: string;
  onComplete: (txHash: string) => void;
  onCancel: () => void;
}

export function PaymentFlow({
  runId,
  amount,
  recipientAddress,
  onComplete,
  onCancel,
}: PaymentFlowProps) {
  const [status, setStatus] = useState<'pending' | 'signing' | 'confirming' | 'error'>('pending');
  const [error, setError] = useState<string | null>(null);

  const handlePay = async () => {
    try {
      setStatus('signing');
      setError(null);

      const provider = sdk.wallet.ethProvider;
      const client = createWalletClient({
        chain: base,
        transport: custom(provider),
      });
      const [address] = await client.getAddresses();

      // Convert amount to USDC units (6 decimals)
      const amountInUnits = BigInt(Math.round(parseFloat(amount) * 1e6));

      const data = encodeFunctionData({
        abi: ERC20_ABI,
        functionName: 'transfer',
        args: [recipientAddress as `0x${string}`, amountInUnits],
      });

      setStatus('confirming');

      const txHash = await client.sendTransaction({
        account: address,
        to: USDC_ADDRESS,
        data,
      });

      onComplete(txHash);
    } catch (err: any) {
      setStatus('error');
      setError(err.message || 'Payment failed');
    }
  };

  return (
    <div className="payment-flow">
      <div className="payment-card">
        <h3>Payment Required</h3>
        <p className="payment-amount">${amount} USDC</p>
        <p className="payment-network">on Base</p>

        {error && <p className="payment-error">{error}</p>}

        <div className="payment-actions">
          {status === 'pending' && (
            <>
              <button className="pay-button" onClick={handlePay}>
                Pay ${amount} USDC
              </button>
              <button className="cancel-button" onClick={onCancel}>
                Cancel
              </button>
            </>
          )}
          {status === 'signing' && <p>Waiting for wallet approval...</p>}
          {status === 'confirming' && <p>Confirming transaction...</p>}
          {status === 'error' && (
            <>
              <button className="pay-button" onClick={handlePay}>
                Retry
              </button>
              <button className="cancel-button" onClick={onCancel}>
                Cancel
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
