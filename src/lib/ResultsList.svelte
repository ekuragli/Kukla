<script>
  import { formatDecisionDate } from './formatters.js';

  let {
    results,
    isLoading,
    hasMore = false,
    loadingMore = false,
    onloadmore,
    onselect,
    filters,
    onfilterchange,
    showFilters = $bindable(false),
    focusedIndex = $bindable(-1),
    searchMode = '',
  } = $props();

  function getSourceBadge(result) {
    if (result.sourceType === 'yargitay') {
      return { label: 'Yargıtay', cls: 'yargitay' };
    }
    if (result.sourceType === 'bedesten') {
      return { label: 'Danıştay', cls: 'danistay' };
    }
    const source = result.decision?.source;
    if (source === 'Yargitay') return { label: 'Yargıtay', cls: 'yargitay' };
    if (source === 'Danistay') return { label: 'Danıştay', cls: 'danistay' };
    if (source === 'Bam') return { label: 'BAM', cls: 'bam' };
    if (source === 'UserUploaded') return { label: 'Yerel', cls: 'local' };
    return { label: source || 'Yerel', cls: 'local' };
  }

  function formatScore(percent) {
    if (percent >= 80) return 'yüksek';
    if (percent >= 60) return 'orta';
    return 'düşük';
  }

  function scoreClass(percent) {
    if (percent >= 80) return 'high';
    if (percent >= 60) return 'mid';
    return 'low';
  }

  function updateFilter(key, value) {
    onfilterchange?.({ ...filters, [key]: value });
  }

  function clearFilters() {
    onfilterchange?.({
      daire: null,
      year_start: null,
      year_end: null,
      court_type: null,
      source: null,
      score_min: null,
      score_max: null,
    });
  }

  function hasActiveFilters() {
    return filters?.daire || filters?.year_start || filters?.year_end
      || filters?.court_type || filters?.source
      || filters?.score_min != null || filters?.score_max != null;
  }

  function searchModeLabel() {
    if (searchMode === 'hybrid_semantic') return 'Hibrit (semantic + keyword)';
    if (searchMode === 'online_keyword') return 'Çevrimiçi arama';
    return '';
  }
</script>

<div class="results-list">
  {#if isLoading}
    <div class="loading-state">
      <div class="spinner"></div>
      <p>Kararlar taranıyor...</p>
    </div>
  {:else if results.length > 0}
    <div class="results-header">
      <span class="count">
        {results.length} sonuç bulundu
        {#if searchModeLabel()}
          <span class="mode-badge">{searchModeLabel()}</span>
        {/if}
      </span>
      <div class="header-actions">
        {#if hasActiveFilters()}
          <span class="filter-active-badge">Filtre aktif</span>
        {/if}
        <button class="filter-toggle" onclick={() => showFilters = !showFilters}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3"/>
          </svg>
          Filtrele
        </button>
      </div>
    </div>

    {#if showFilters}
      <div class="filter-panel">
        <div class="filter-row">
          <label for="filter-court">Mahkeme</label>
          <select id="filter-court" value={filters?.court_type || ''} onchange={(e) => updateFilter('court_type', e.target.value || null)}>
            <option value="">Tümü</option>
            <option value="YARGITAYKARARI">Yargıtay</option>
            <option value="DANISTAYKARAR">Danıştay</option>
          </select>
        </div>
        <div class="filter-row">
          <label for="filter-source">Kaynak</label>
          <select id="filter-source" value={filters?.source || ''} onchange={(e) => updateFilter('source', e.target.value || null)}>
            <option value="">Tümü</option>
            <option value="online">Çevrimiçi (Yargıtay / Danıştay)</option>
            <option value="local">Yerel (PDF / Arşiv)</option>
          </select>
        </div>
        <div class="filter-row">
          <label for="filter-daire">Daire</label>
          <select id="filter-daire" value={filters?.daire || ''} onchange={(e) => updateFilter('daire', e.target.value || null)}>
            <option value="">Tümü</option>
            <option value="Hukuk Genel Kurulu">Hukuk Genel Kurulu</option>
            <option value="Ceza Genel Kurulu">Ceza Genel Kurulu</option>
            <option value="1. Hukuk Dairesi">1. Hukuk Dairesi</option>
            <option value="2. Hukuk Dairesi">2. Hukuk Dairesi</option>
            <option value="3. Hukuk Dairesi">3. Hukuk Dairesi</option>
            <option value="4. Hukuk Dairesi">4. Hukuk Dairesi</option>
            <option value="5. Hukuk Dairesi">5. Hukuk Dairesi</option>
            <option value="6. Hukuk Dairesi">6. Hukuk Dairesi</option>
            <option value="7. Hukuk Dairesi">7. Hukuk Dairesi</option>
            <option value="8. Hukuk Dairesi">8. Hukuk Dairesi</option>
            <option value="9. Hukuk Dairesi">9. Hukuk Dairesi</option>
            <option value="10. Hukuk Dairesi">10. Hukuk Dairesi</option>
            <option value="11. Hukuk Dairesi">11. Hukuk Dairesi</option>
            <option value="12. Hukuk Dairesi">12. Hukuk Dairesi</option>
            <option value="13. Hukuk Dairesi">13. Hukuk Dairesi</option>
            <option value="14. Hukuk Dairesi">14. Hukuk Dairesi</option>
            <option value="15. Hukuk Dairesi">15. Hukuk Dairesi</option>
            <option value="16. Hukuk Dairesi">16. Hukuk Dairesi</option>
            <option value="17. Hukuk Dairesi">17. Hukuk Dairesi</option>
            <option value="18. Hukuk Dairesi">18. Hukuk Dairesi</option>
            <option value="19. Hukuk Dairesi">19. Hukuk Dairesi</option>
            <option value="20. Hukuk Dairesi">20. Hukuk Dairesi</option>
            <option value="21. Hukuk Dairesi">21. Hukuk Dairesi</option>
            <option value="22. Hukuk Dairesi">22. Hukuk Dairesi</option>
            <option value="23. Hukuk Dairesi">23. Hukuk Dairesi</option>
          </select>
        </div>
        <div class="filter-row">
          <label for="filter-year-start">Yıl Başlangıç</label>
          <input id="filter-year-start" type="number" placeholder="örn. 2020" min="1950" max="2030"
            value={filters?.year_start || ''}
            onchange={(e) => updateFilter('year_start', e.target.value ? parseInt(e.target.value) : null)} />
        </div>
        <div class="filter-row">
          <label for="filter-year-end">Yıl Bitiş</label>
          <input id="filter-year-end" type="number" placeholder="örn. 2024" min="1950" max="2030"
            value={filters?.year_end || ''}
            onchange={(e) => updateFilter('year_end', e.target.value ? parseInt(e.target.value) : null)} />
        </div>
        <div class="filter-row">
          <label for="filter-score-min">Min. Benzerlik %</label>
          <input id="filter-score-min" type="number" placeholder="örn. 60" min="0" max="100"
            value={filters?.score_min ?? ''}
            onchange={(e) => updateFilter('score_min', e.target.value !== '' ? parseInt(e.target.value) : null)} />
        </div>
        <div class="filter-row">
          <label for="filter-score-max">Maks. Benzerlik %</label>
          <input id="filter-score-max" type="number" placeholder="örn. 100" min="0" max="100"
            value={filters?.score_max ?? ''}
            onchange={(e) => updateFilter('score_max', e.target.value !== '' ? parseInt(e.target.value) : null)} />
        </div>
        {#if hasActiveFilters()}
          <button type="button" class="clear-filters-btn" onclick={clearFilters}>Filtreleri Temizle</button>
        {/if}
      </div>
    {/if}

    <div class="results-scroll">
      {#each results as result, i}
        {@const decisionDate = formatDecisionDate(result.decision.karar_tarihi)}
        <button
          class="result-card"
          class:focused={focusedIndex === i}
          onclick={() => { focusedIndex = i; onselect(result); }}
        >
          <div class="card-header">
            {#if getSourceBadge(result)}
              <span class="source-badge {getSourceBadge(result).cls}">
                {getSourceBadge(result).label}
              </span>
            {/if}
            {#if result.similarity_percent > 0}
              <span class="score {scoreClass(result.similarity_percent)}">
                %{result.similarity_percent} ({formatScore(result.similarity_percent)})
              </span>
            {:else if result.sourceType === 'yargitay' || result.sourceType === 'bedesten'}
              <span class="score online-score">Çevrimiçi</span>
            {/if}
          </div>
          <div class="card-refs">
            {#if result.decision.esas_no}
              <span class="ref">E: {result.decision.esas_no}</span>
            {/if}
            {#if result.decision.karar_no}
              <span class="ref">K: {result.decision.karar_no}</span>
            {/if}
            {#if result.decision.daire}
              <span class="ref">{result.decision.daire}</span>
            {/if}
            {#if decisionDate}
              <span class="ref date">{decisionDate}</span>
            {/if}
          </div>
          <p class="snippet">{result.snippet || (result.decision.summary || '')}</p>
        </button>
      {/each}
    </div>
    {#if hasMore}
      <div class="load-more-row">
        <button
          type="button"
          class="load-more-btn"
          onclick={() => onloadmore?.()}
          disabled={loadingMore}
        >
          {loadingMore ? 'Yükleniyor...' : 'Daha Fazla Sonuç Yükle'}
        </button>
      </div>
    {/if}
  {:else}
    <div class="empty-state">
      <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="1.5">
        <circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>
      </svg>
      <p>Henüz arama yapılmadı. Sorgunuzu yazın veya PDF yükleyin</p>
      <p class="hint">Örn: "Kira bedelinin uyarlanması", "iş kazası tazminatı"</p>
    </div>
  {/if}
</div>

<style>
  .results-list {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .results-header {
    padding: 12px 24px;
    border-bottom: 1px solid var(--border);
    font-size: 13px;
    color: var(--text-secondary);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .mode-badge {
    display: inline-block;
    margin-left: 8px;
    font-size: 11px;
    color: var(--accent);
    background: var(--accent-muted);
    padding: 2px 6px;
    border-radius: 4px;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .filter-active-badge {
    font-size: 11px;
    font-weight: 600;
    color: var(--accent);
    background: var(--accent-muted);
    padding: 2px 8px;
    border-radius: 4px;
  }

  .filter-toggle {
    display: flex;
    align-items: center;
    gap: 4px;
    background: transparent;
    color: var(--text-muted);
    font-size: 12px;
    padding: 4px 8px;
    border-radius: var(--radius);
  }

  .filter-toggle:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .filter-panel {
    padding: 12px 24px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
  }

  .filter-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 140px;
  }

  .filter-row label {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
  }

  .filter-row select,
  .filter-row input {
    font-size: 13px;
    padding: 6px 10px;
    border-radius: var(--radius);
    background: var(--bg-primary);
    border: 1px solid var(--border);
    color: var(--text-primary);
  }

  .clear-filters-btn {
    align-self: flex-end;
    font-size: 12px;
    color: var(--text-muted);
    padding: 6px 10px;
    border-radius: var(--radius);
    background: var(--bg-primary);
    border: 1px solid var(--border);
  }

  .clear-filters-btn:hover {
    color: var(--text-primary);
    border-color: var(--accent);
  }

  .results-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 12px 24px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .result-card {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 16px;
    text-align: left;
    transition: border-color 0.15s, background 0.15s;
    width: 100%;
  }

  .result-card:hover,
  .result-card.focused {
    border-color: var(--accent);
    background: var(--bg-tertiary);
  }

  .result-card.focused {
    box-shadow: 0 0 0 1px var(--accent);
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
  }

  .source-badge {
    font-size: 11px;
    font-weight: 600;
    padding: 3px 8px;
    border-radius: 4px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .source-badge.yargitay {
    background: rgba(99, 102, 241, 0.15);
    color: #818cf8;
  }

  .source-badge.danistay {
    background: rgba(34, 197, 94, 0.15);
    color: #4ade80;
  }

  .source-badge.bam {
    background: rgba(234, 179, 8, 0.15);
    color: #facc15;
  }

  .source-badge.local {
    background: rgba(148, 163, 184, 0.15);
    color: #94a3b8;
  }

  .source-badge.online {
    background: rgba(56, 189, 248, 0.15);
    color: #38bdf8;
  }

  .score {
    font-size: 12px;
    font-weight: 600;
    padding: 3px 8px;
    border-radius: 4px;
  }

  .score.high {
    background: rgba(34, 197, 94, 0.15);
    color: #4ade80;
  }

  .score.mid {
    background: rgba(234, 179, 8, 0.15);
    color: #facc15;
  }

  .score.low {
    background: rgba(239, 68, 68, 0.1);
    color: #f87171;
  }

  .online-score {
    background: rgba(56, 189, 248, 0.15);
    color: #38bdf8;
  }

  .card-refs {
    display: flex;
    gap: 12px;
    margin-bottom: 8px;
  }

  .ref {
    font-size: 12px;
    color: var(--text-muted);
  }

  .ref.date {
    color: var(--text-secondary);
    font-weight: 500;
  }

  .snippet {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.5;
    display: -webkit-box;
    line-clamp: 3;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    flex: 1;
    gap: 12px;
    color: var(--text-secondary);
    font-size: 14px;
  }

  .spinner {
    width: 28px;
    height: 28px;
    border: 3px solid var(--bg-tertiary);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .load-more-row {
    padding: 12px 24px 20px;
    border-top: 1px solid var(--border);
    display: flex;
    justify-content: center;
  }

  .load-more-btn {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    color: var(--text-primary);
    padding: 8px 16px;
    border-radius: var(--radius);
    font-size: 13px;
    font-weight: 600;
    transition: all 0.15s;
  }

  .load-more-btn:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }

  .load-more-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    flex: 1;
    gap: 12px;
    color: var(--text-muted);
    font-size: 14px;
  }

  .hint {
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
