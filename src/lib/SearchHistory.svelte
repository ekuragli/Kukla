<script>
  import { invoke } from '@tauri-apps/api/core';

  let { onselect } = $props();

  let history = $state([]);
  let loading = $state(true);
  let loadError = $state('');

  $effect(() => {
    loadHistory();
  });

  async function loadHistory() {
    loading = true;
    loadError = '';
    try {
      history = await invoke('get_search_history', { limit: 20 });
    } catch (e) {
      history = [];
      loadError = typeof e === 'string' ? e : 'Arama geçmişi yüklenemedi.';
    } finally {
      loading = false;
    }
  }

  async function clearHistory() {
    try {
      await invoke('clear_search_history');
      history = [];
    } catch (e) {
      loadError = typeof e === 'string' ? e : 'Geçmiş temizlenemedi.';
    }
  }
</script>

<div class="history-panel">
  <div class="history-header">
    <h3>Arama Geçmişi</h3>
    {#if !loading && history.length > 0}
      <button class="clear-btn" onclick={clearHistory} aria-label="Arama geçmişini temizle">Temizle</button>
    {/if}
  </div>
  {#if loading}
    <p class="muted">Yükleniyor...</p>
  {:else if loadError}
    <p class="error" role="alert">{loadError}</p>
    <button class="retry-btn" onclick={loadHistory}>Tekrar Dene</button>
  {:else if history.length === 0}
    <p class="muted">Henüz arama yapılmadı</p>
  {:else}
    <div class="history-list">
      {#each history as item}
        <button class="history-item" onclick={() => onselect(item.query_text)}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>
          </svg>
          <span class="history-query">{item.query_text}</span>
          <span class="history-meta">{item.result_count} sonuç</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .history-panel {
    padding: 12px;
  }

  .history-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
    padding: 0 4px;
  }

  h3 {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin: 0;
  }

  .clear-btn {
    font-size: 10px;
    color: var(--text-muted);
    padding: 2px 6px;
    border-radius: var(--radius);
  }

  .clear-btn:hover {
    color: var(--danger);
    background: rgba(239, 68, 68, 0.1);
  }

  .muted {
    color: var(--text-muted);
    font-size: 12px;
    padding: 8px 4px;
  }

  .error {
    color: var(--danger);
    font-size: 12px;
    padding: 8px 4px;
  }

  .retry-btn {
    font-size: 11px;
    color: var(--accent);
    padding: 4px;
    margin-left: 4px;
  }

  .retry-btn:hover {
    text-decoration: underline;
  }

  .history-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .history-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-secondary);
    font-size: 13px;
    text-align: left;
    width: 100%;
    transition: all 0.15s;
  }

  .history-item:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .history-query {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .history-meta {
    font-size: 11px;
    color: var(--text-muted);
    flex-shrink: 0;
  }
</style>