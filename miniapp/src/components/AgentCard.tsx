import type { AgentManifest } from '../lib/api';

interface AgentCardProps {
  agent: AgentManifest;
  onClick?: (agent: AgentManifest) => void;
}

export function AgentCard({ agent, onClick }: AgentCardProps) {
  return (
    <div className="agent-card" onClick={() => onClick?.(agent)}>
      <h3>{agent.name}</h3>
      <p className="agent-description">{agent.description}</p>
      <div className="agent-stats">
        <span className="stat">
          <strong>{agent.capabilities_count}</strong> capabilities
        </span>
        <span className="stat">
          <strong>{agent.endpoints_count}</strong> endpoints
        </span>
      </div>
      {agent.on_chain_id && (
        <a
          className="on-chain-link"
          href={`https://basescan.org/token/${agent.on_chain_id}`}
          target="_blank"
          rel="noopener noreferrer"
          onClick={(e) => e.stopPropagation()}
        >
          View on-chain
        </a>
      )}
      <time className="agent-date">
        {new Date(agent.created_at).toLocaleDateString()}
      </time>
    </div>
  );
}
