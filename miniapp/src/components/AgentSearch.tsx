import { useState } from 'react';
import { useSearchAgents } from '../hooks/useBeaconApi';
import { AgentCard } from './AgentCard';
import type { AgentManifest } from '../lib/api';

interface AgentSearchProps {
  onSelect?: (agent: AgentManifest) => void;
}

export function AgentSearch({ onSelect }: AgentSearchProps) {
  const [query, setQuery] = useState('');
  const [searchTerm, setSearchTerm] = useState('');
  const { data, isLoading, error } = useSearchAgents(searchTerm || undefined);

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    setSearchTerm(query);
  };

  return (
    <div className="agent-search">
      <form onSubmit={handleSearch} className="search-form">
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search agents..."
          className="search-input"
        />
        <button type="submit" className="search-button">
          Search
        </button>
      </form>

      {isLoading && <p className="loading">Loading agents...</p>}
      {error && <p className="error">Error: {(error as Error).message}</p>}

      <div className="agents-grid">
        {data?.agents.map((agent) => (
          <AgentCard key={agent.id} agent={agent} onClick={onSelect} />
        ))}
      </div>

      {data && data.agents.length === 0 && (
        <p className="empty">No agents found.</p>
      )}
    </div>
  );
}
