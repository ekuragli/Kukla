<script>
  import { invoke } from '@tauri-apps/api/core';
  import { formatInvokeError } from './formatters.js';
  import SearchBar from './SearchBar.svelte';
  import ResultsList from './ResultsList.svelte';
  import DetailPanel from './DetailPanel.svelte';
  import UploadPanel from './UploadPanel.svelte';
  import SettingsPanel from './SettingsPanel.svelte';
  import SearchHistory from './SearchHistory.svelte';
  import PersonalArchive from './PersonalArchive.svelte';
import ConvertPanel from './ConvertPanel.svelte';
  import ChatDrawer from './ChatDrawer.svelte';
  import OnboardingOverlay from './OnboardingOverlay.svelte';
  import ShortcutsOverlay from './ShortcutsOverlay.svelte';
  import FaqOverlay from './FaqOverlay.svelte';
  import UpdateNotifier from './UpdateNotifier.svelte';

  let { onlock } = $props();

  const modKey = (() => {
    if (typeof navigator === 'undefined') return 'Ctrl';
    const ua = navigator.userAgent || '';
    const platform = navigator.userAgentData?.platform || '';
    return /Mac|iPhone|iPad/.test(platform + ua) ? '⌘' : 'Ctrl';
  })();

  let results = $state([]);
  let selectedResult = $state(null);
  let isLoading = $state(false);
  let error = $state('');
  let activeView = $state('search');
  let chatOpen = $state(false);
  let chatContext = $state(null);
  let filters = $state({
    daire: null,
    year_start: null,
    year_end: null,
    court_type: null,
    source: null,
    score_min: null,
    score_max: null,
  });
  let showOnboarding = $state(false);
  let showShortcuts = $state(false);
  let showFaq = $state(false);
  let showFilters = $state(false);
  let searchPrefill = $state('');
  let searchMode = $state('');
  let archiveEnabled = $state(false);
  let focusedResultIndex = $state(-1);
  let lastQuery = $state('');
  let prevFilterSnapshot = $state(null);
  let searchPage = $state(1);
  let hasMoreResults = $state(false);
  let loadingMore = $state(false);

  function isTypingContext() {
    const el = document.activeElement;
    if (!el) return false;
    const tag = el.tagName;
    return tag === 'INPUT' || tag === 'TEXTAREA' || el.isContentEditable;
  }

  function focusSearch() {
    activeView = 'search';
    requestAnimationFrame(() => {
      const textarea = document.querySelector('.search-bar textarea');
      if (textarea) {
        textarea.focus();
        textarea.select();
      }
    });
  }

  function scrollFocusedIntoView() {
    requestAnimationFrame(() => {
      const card = document.querySelector(`.result-card.focused`);
      card?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    });
  }

  function handleKeydown(e) {
    const mod = e.ctrlKey || e.metaKey;

    if (mod && e.key === '/') {
      e.preventDefault();
      showShortcuts = !showShortcuts;
      scheduleActivityPing();
      return;
    }

    if (showShortcuts) {
      if (e.key === 'Escape') {
        e.preventDefault();
        showShortcuts = false;
      }
      return;
    }

    if (showOnboarding) return;

    if (mod && e.key === 'k') {
      e.preventDefault();
      focusSearch();
      scheduleActivityPing();
      return;
    }

    if (mod && e.key === 'l') {
      e.preventDefault();
      handleLock();
      return;
    }

    if (mod && e.key === 'j') {
      e.preventDefault();
      toggleChat();
      return;
    }

    if (mod && e.key === 'e') {
      e.preventDefault();
      const exportBtn = document.querySelector('.export-btn');
      if (exportBtn && !exportBtn.disabled) exportBtn.click();
      scheduleActivityPing();
      return;
    }

    if (mod && !e.shiftKey && !e.altKey) {
      const viewKeys = { '1': 'search', '2': 'upload', '3': 'archive', '4': 'settings', '5': 'convert' };
      if (viewKeys[e.key]) {
        e.preventDefault();
        activeView = viewKeys[e.key];
        if (activeView === 'search') focusSearch();
        scheduleActivityPing();
        return;
      }
    }

    if (e.key === 'Escape') {
      if (selectedResult) {
        handleBack();
        return;
      }
      if (showFilters) {
        showFilters = false;
        return;
      }
      if (focusedResultIndex >= 0) {
        focusedResultIndex = -1;
        return;
      }
    }

    if (activeView === 'search' && !selectedResult && results.length > 0 && !isTypingContext()) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        focusedResultIndex = focusedResultIndex < 0
          ? 0
          : Math.min(focusedResultIndex + 1, results.length - 1);
        scrollFocusedIntoView();
        scheduleActivityPing();
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        focusedResultIndex = Math.max(focusedResultIndex - 1, 0);
        scrollFocusedIntoView();
        scheduleActivityPing();
        return;
      }
      if (e.key === 'Enter' && focusedResultIndex >= 0) {
        e.preventDefault();
        handleSelectResult(results[focusedResultIndex]);
        return;
      }
    }

    scheduleActivityPing();
  }

  function showOnboardingIfFirstRun() {
    try {
      const seen = localStorage.getItem('kukla_onboarding_seen');
      if (!seen) {
        showOnboarding = true;
        localStorage.setItem('kukla_onboarding_seen', 'true');
      }
    } catch (_) {
      // localStorage unavailable
    }
  }

  $effect(() => {
    showOnboardingIfFirstRun();
    invoke('get_settings')
      .then((s) => { archiveEnabled = s.personal_archive_enabled; })
      .catch(() => {});
  });

  function applyScoreFilter(list) {
    let out = list;
    if (filters.score_min != null) {
      // Çevrimiçi sonuçların benzerliği 0 olabilir; bunları filtre dışı bırak.
      out = out.filter((r) => r.similarity_percent === 0 || r.similarity_percent >= filters.score_min);
    }
    if (filters.score_max != null) {
      out = out.filter((r) => r.similarity_percent === 0 || r.similarity_percent <= filters.score_max);
    }
    return out;
  }

  function mapSearchResults(items) {
    return items.map(r => ({
      ...r,
      sourceType: r.online_source || (r.snippet ? 'local' : 'yargitay'),
    }));
  }

  function dedupeSearchResults(list) {
    const seen = new Set();
    return list.filter((r) => {
      const id = r.decision?.id;
      if (id && id > 0) {
        if (seen.has(id)) return false;
        seen.add(id);
      }
      return true;
    });
  }

  async function runSearch(query, { append = false } = {}) {
    if (append) {
      loadingMore = true;
    } else {
      isLoading = true;
      error = '';
      selectedResult = null;
      focusedResultIndex = -1;
      searchPage = 1;
      hasMoreResults = false;
    }
    lastQuery = query;

    try {
      const courtTypes = filters.court_type
        ? [filters.court_type]
        : ['YARGITAYKARARI'];

      const nextPage = append ? searchPage + 1 : 1;
      const response = await invoke('search_unified', {
        query,
        courtTypes,
        daire: filters.daire || null,
        yearStart: filters.year_start || null,
        yearEnd: filters.year_end || null,
        sourceFilter: filters.source || null,
        page: nextPage,
      });

      searchMode = response.search_mode || '';
      const mapped = applyScoreFilter(mapSearchResults(response.results));
      if (append) {
        results = dedupeSearchResults([...results, ...mapped]);
        searchPage = response.page;
      } else {
        results = mapped;
        searchPage = response.page;
      }
      hasMoreResults = response.has_more;
    } catch (e) {
      const msg = formatInvokeError(e);
      if (msg.includes('Oturum kilitli') || msg.includes('zaman aşımı')) {
        onlock();
        return;
      }
      if (!append) {
        error = msg || 'Arama yapılamadı. İnternet bağlantınızı kontrol edin veya tekrar deneyin.';
      }
    } finally {
      if (append) {
        loadingMore = false;
      } else {
        isLoading = false;
      }
    }
  }

  async function handleSearch(query) {
    await runSearch(query);
  }

  async function handleLoadMore() {
    if (!lastQuery.trim() || loadingMore || !hasMoreResults) return;
    await runSearch(lastQuery, { append: true });
  }

  function handleSearchFromPdf(text) {
    searchPrefill = text;
    activeView = 'search';
    handleSearch(text);
  }

  function handleSearchFromHistory(query) {
    const textarea = document.querySelector('.search-bar textarea');
    if (textarea) {
      const nativeInputValueSetter = Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value').set;
      nativeInputValueSetter.call(textarea, query);
      textarea.dispatchEvent(new Event('input', { bubbles: true }));
    }
    handleSearch(query);
  }

  function handleSelectResult(result) {
    selectedResult = result;
  }

  function handleBack() {
    selectedResult = null;
  }

  function toggleChat() {
    chatOpen = !chatOpen;
    if (!chatOpen) chatContext = null;
    scheduleActivityPing();
  }

  // DetailPanel'deki "Sohbette Sor": seçili karar metni sohbete bağlam olarak gider.
  function askChat(context) {
    chatContext = context;
    chatOpen = true;
  }

  function handleFilterChange(newFilters) {
    filters = newFilters;
  }

  $effect(() => {
    const snapshot = JSON.stringify(filters);
    if (prevFilterSnapshot === null) {
      prevFilterSnapshot = snapshot;
      return;
    }
    if (prevFilterSnapshot === snapshot) return;
    prevFilterSnapshot = snapshot;
    if (!lastQuery.trim() || activeView !== 'search' || selectedResult) return;
    handleSearch(lastQuery);
  });

  async function handleLock() {
    try {
      await invoke('auth_lock');
    } catch (_) {}
    onlock();
  }

  let activityTimer = null;

  function scheduleActivityPing() {
    if (activityTimer) clearTimeout(activityTimer);
    activityTimer = setTimeout(async () => {
      try {
        await invoke('auth_ping_activity');
      } catch (_) {}
    }, 1000);
  }

  async function checkSession() {
    try {
      const active = await invoke('auth_check_session');
      if (!active) onlock();
    } catch (_) {
      onlock();
    }
  }

  $effect(() => {
    if (typeof window === 'undefined') return;

    const onActivity = () => scheduleActivityPing();
    const sessionInterval = setInterval(checkSession, 30_000);

    window.addEventListener('mousemove', onActivity);
    window.addEventListener('mousedown', onActivity);
    window.addEventListener('scroll', onActivity, true);

    return () => {
      clearInterval(sessionInterval);
      if (activityTimer) clearTimeout(activityTimer);
      window.removeEventListener('mousemove', onActivity);
      window.removeEventListener('mousedown', onActivity);
      window.removeEventListener('scroll', onActivity, true);
    };
  });
</script>

<svelte:window onkeydown={handleKeydown} />

{#if showOnboarding}
  <OnboardingOverlay onclose={() => showOnboarding = false} />
{/if}

{#if showShortcuts}
  <ShortcutsOverlay onclose={() => showShortcuts = false} {modKey} />
{/if}

{#if showFaq}
  <FaqOverlay onclose={() => showFaq = false} />
{/if}

<div class="layout">
  <aside class="sidebar">
    <div class="sidebar-header">
      <div class="sidebar-logo">K</div>
      <span class="sidebar-title">Kukla</span>
    </div>

    <nav class="sidebar-nav" aria-label="Ana menü">
      <button
        class="nav-item"
        class:active={activeView === 'search'}
        onclick={() => activeView = 'search'}
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>
        </svg>
        Yeni Sorgu
      </button>
      <button
        class="nav-item"
        class:active={activeView === 'upload'}
        onclick={() => activeView = 'upload'}
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/>
        </svg>
        PDF Yükle
      </button>
      <button
        class="nav-item"
        class:active={activeView === 'archive'}
        class:disabled={!archiveEnabled}
        onclick={() => archiveEnabled ? activeView = 'archive' : activeView = 'settings'}
        title={archiveEnabled ? 'Kişisel Karar Arşivi' : 'Ayarlardan etkinleştirin'}
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <path d="M21 8V6a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v2"/><rect x="3" y="8" width="18" height="12" rx="2"/><line x1="10" y1="12" x2="14" y2="12"/>
        </svg>
        Kişisel Arşiv
      </button>
      <button
        class="nav-item"
        class:active={activeView === 'settings'}
        onclick={() => activeView = 'settings'}
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
        </svg>
        Ayarlar
      </button>
      <button
        class="nav-item"
        class:active={chatOpen}
        onclick={toggleChat}
        title="Sohbet Asistanı (Ctrl+J)"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
        </svg>
        Sohbet
      </button>
      <button
        class="nav-item"
        class:active={activeView === 'convert'}
        onclick={() => activeView = 'convert'}
        title="Dosya Dönüştürücü"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>
        </svg>
        Araçlar
      </button>
    </nav>

    {#if activeView === 'search'}
      <SearchHistory onselect={handleSearchFromHistory} />
    {/if}

    <div class="sidebar-footer">
      <button class="nav-item help-btn" onclick={() => showFaq = true} title="SSS">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><line x1="12" y1="17" x2="12.01" y2="17"/>
        </svg>
        SSS
      </button>
      <button class="nav-item help-btn" onclick={() => showOnboarding = true} title="Yardım">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><line x1="12" y1="17" x2="12.01" y2="17"/>
        </svg>
        Yardım
      </button>
      <button class="nav-item lock-btn" onclick={handleLock}>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>
        </svg>
        Kilitle
      </button>
    </div>
  </aside>

  <main class="main-content">
    <UpdateNotifier />
    {#if activeView === 'search'}
      {#if selectedResult}
        <DetailPanel
          result={selectedResult}
          onback={handleBack}
          searchQuery={lastQuery}
          onaskchat={askChat}
        />
      {:else}
        <div class="search-view">
          <SearchBar onsearch={handleSearch} isLoading={isLoading} initialQuery={searchPrefill} />
          {#if error}
            <div class="error-banner" role="alert" aria-live="polite">
              <span>{error}</span>
              <button class="retry-btn" onclick={() => handleSearch(lastQuery)}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                  <polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
                </svg>
                Tekrar Dene
              </button>
            </div>
          {/if}
          <ResultsList
            results={results}
            isLoading={isLoading}
            hasMore={hasMoreResults}
            loadingMore={loadingMore}
            onloadmore={handleLoadMore}
            onselect={handleSelectResult}
            bind:filters
            bind:showFilters
            bind:focusedIndex={focusedResultIndex}
            onfilterchange={handleFilterChange}
            {searchMode}
          />
        </div>
      {/if}
    {:else if activeView === 'upload'}
      <UploadPanel onsearchfrompdf={handleSearchFromPdf} />
    {:else if activeView === 'archive'}
      <PersonalArchive onsettingschange={(enabled) => archiveEnabled = enabled} />
    {:else if activeView === 'settings'}
      <SettingsPanel />
    {:else if activeView === 'convert'}
      <ConvertPanel />
    {/if}
    {#if chatOpen}
      <ChatDrawer
        onclose={() => { chatOpen = false; chatContext = null; }}
        context={chatContext}
      />
    {/if}
  </main>
</div>

<style>
  .layout {
    display: flex;
    height: 100vh;
  }

  .sidebar {
    width: 220px;
    background: var(--bg-secondary);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
  }

  .sidebar-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 20px 16px;
    border-bottom: 1px solid var(--border);
  }

  .sidebar-logo {
    width: 32px;
    height: 32px;
    background: var(--accent);
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 16px;
    font-weight: 700;
    color: white;
  }

  .sidebar-title {
    font-size: 16px;
    font-weight: 600;
  }

  .sidebar-nav {
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-secondary);
    font-size: 14px;
    transition: all 0.15s;
    width: 100%;
    text-align: left;
  }

  .nav-item:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .nav-item.active {
    background: var(--accent-muted);
    color: var(--accent);
  }

  .nav-item.disabled {
    opacity: 0.5;
  }

  .sidebar-footer {
    padding: 8px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .lock-btn {
    color: var(--text-muted);
  }

  .lock-btn:hover {
    color: var(--warning);
  }

  .help-btn {
    color: var(--text-muted);
  }

  .help-btn:hover {
    color: var(--accent);
  }

  .main-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    position: relative;
  }

  .search-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .error-banner {
    background: rgba(239, 68, 68, 0.1);
    color: var(--danger);
    padding: 10px 20px;
    font-size: 13px;
    border-bottom: 1px solid rgba(239, 68, 68, 0.2);
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .error-banner span {
    flex: 1;
  }

  .retry-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    background: rgba(239, 68, 68, 0.2);
    color: var(--danger);
    padding: 5px 12px;
    border-radius: var(--radius);
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    transition: all 0.15s;
  }

  .retry-btn:hover {
    background: rgba(239, 68, 68, 0.3);
  }
</style>