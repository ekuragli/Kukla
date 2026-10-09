<script>
  let { onsearch, isLoading, initialQuery = '' } = $props();

  const MIN_QUERY_CHARS = 10;

  let query = $state('');
  let queryWarning = $state('');

  $effect(() => {
    if (initialQuery?.trim()) {
      query = initialQuery;
    }
  });

  function handleSubmit(e) {
    e.preventDefault();
    const trimmed = query.trim();
    if (!trimmed) return;
    if ([...trimmed].length < MIN_QUERY_CHARS) {
      queryWarning = `Sorgu en az ${MIN_QUERY_CHARS} karakter olmalıdır.`;
      return;
    }
    queryWarning = '';
    onsearch(trimmed);
  }
</script>

<div class="search-bar">
  <form onsubmit={handleSubmit}>
    <div class="input-wrapper">
      <label class="sr-only" for="search-query">Arama sorgusu</label>
      <textarea
        id="search-query"
        placeholder="Dava özetinizi veya anahtar kelimeleri yazın..."
        bind:value={query}
        rows="3"
        onkeydown={(e) => {
          if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSubmit(e);
          }
        }}
      ></textarea>
      <button type="submit" disabled={isLoading || !query.trim()}>
        {#if isLoading}
          <span class="btn-spinner"></span>
        {:else}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>
          </svg>
        {/if}
        Ara
      </button>
    </div>
  </form>
  {#if queryWarning}
    <p class="query-warning" role="alert">{queryWarning}</p>
  {/if}
  <p class="hint">
    Enter ile ara · Shift+Enter yeni satır · Ctrl+K odaklan
    <span class="hint-sep">·</span>
    Örn: "Kira bedelinin uyarlanması"
  </p>
</div>

<style>
  .search-bar {
    padding: 20px 24px;
    border-bottom: 1px solid var(--border);
  }

  .input-wrapper {
    display: flex;
    gap: 8px;
    align-items: flex-start;
  }

  textarea {
    flex: 1;
    resize: none;
    padding: 12px 16px;
    font-size: 14px;
    line-height: 1.5;
    min-height: 68px;
  }

  button {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--accent);
    color: white;
    padding: 12px 20px;
    border-radius: var(--radius);
    font-weight: 600;
    transition: background 0.15s;
    white-space: nowrap;
    height: 68px;
  }

  button:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(255,255,255,0.3);
    border-top-color: white;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .query-warning {
    color: var(--danger);
    font-size: 12px;
    margin-top: 8px;
  }

  .hint {
    color: var(--text-muted);
    font-size: 12px;
    margin-top: 8px;
  }

  .hint-sep {
    margin: 0 4px;
    opacity: 0.5;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
</style>
