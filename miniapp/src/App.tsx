import { useState } from 'react';
import { useFarcasterContext } from './hooks/useFarcasterContext';
import { AgentSearch } from './components/AgentSearch';
import { GenerateForm } from './components/GenerateForm';
import { ValidateForm } from './components/ValidateForm';
import './App.css';

type Tab = 'browse' | 'generate' | 'validate';

function App() {
  const [tab, setTab] = useState<Tab>('browse');
  const { user } = useFarcasterContext();

  return (
    <div className="app">
      <header className="app-header">
        <h1>Beacon</h1>
        <p className="tagline">Make any repo agent-ready. Instantly.</p>
        {user.username && (
          <p className="user-info">@{user.username}</p>
        )}
      </header>

      <nav className="app-nav">
        <button
          className={`nav-tab ${tab === 'browse' ? 'active' : ''}`}
          onClick={() => setTab('browse')}
        >
          Browse
        </button>
        <button
          className={`nav-tab ${tab === 'generate' ? 'active' : ''}`}
          onClick={() => setTab('generate')}
        >
          Generate
        </button>
        <button
          className={`nav-tab ${tab === 'validate' ? 'active' : ''}`}
          onClick={() => setTab('validate')}
        >
          Validate
        </button>
      </nav>

      <main className="app-main">
        {tab === 'browse' && <AgentSearch />}
        {tab === 'generate' && <GenerateForm />}
        {tab === 'validate' && <ValidateForm />}
      </main>
    </div>
  );
}

export default App;
