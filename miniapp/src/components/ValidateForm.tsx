import { useState } from 'react';
import { useValidate } from '../hooks/useBeaconApi';

export function ValidateForm() {
  const [content, setContent] = useState('');
  const validate = useValidate();

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!content.trim()) return;
    validate.mutate(content);
  };

  return (
    <div className="validate-form">
      <h2>Validate AGENTS.md</h2>
      <form onSubmit={handleSubmit}>
        <div className="form-group">
          <label htmlFor="agents-content">Paste your AGENTS.md content</label>
          <textarea
            id="agents-content"
            value={content}
            onChange={(e) => setContent(e.target.value)}
            placeholder="# AGENTS.md — My Project..."
            className="form-textarea"
            rows={12}
          />
        </div>
        <button
          type="submit"
          className="form-button"
          disabled={validate.isPending || !content.trim()}
        >
          {validate.isPending ? 'Validating...' : 'Validate'}
        </button>
      </form>

      {validate.isSuccess && (
        <div className={`validation-result ${validate.data.valid ? 'valid' : 'invalid'}`}>
          <h3>{validate.data.valid ? 'Valid' : 'Invalid'}</h3>

          {validate.data.errors.length > 0 && (
            <div className="validation-errors">
              <h4>Errors</h4>
              <ul>
                {validate.data.errors.map((err, i) => (
                  <li key={i}>{err}</li>
                ))}
              </ul>
            </div>
          )}

          {validate.data.warnings.length > 0 && (
            <div className="validation-warnings">
              <h4>Warnings</h4>
              <ul>
                {validate.data.warnings.map((warn, i) => (
                  <li key={i}>{warn}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}

      {validate.isError && (
        <p className="error">Error: {(validate.error as Error).message}</p>
      )}
    </div>
  );
}
