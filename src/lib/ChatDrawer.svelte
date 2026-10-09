<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { formatInvokeError } from './formatters.js';

  let { onclose, context = null } = $props();

  let sessions = $state([]);
  let activeSessionId = $state(null);
  let allowExternal = $state(false);
  let pendingExternal = $state(false);
  let consentPending = $state(false);
  let webStatus = $state(null);
  let aiStatus = $state(null);
  let notice = $state('');
  let messages = $state([]);
  let input = $state('');
  let sending = $state(false);
  let error = $state('');
  let loadingHistory = $state(false);

  // YZ akış takibi (DetailPanel ile aynı örüntü)
  let aiRequestId = $state('');
  let aiSessionId = $state('');
  let aiPreview = $state('');
  let aiElapsed = $state(0);
  let aiCancelled = false;
  let aiRetryNote = $state('');
  let aiTimer = null;

  // DetailPanel'dan "Sohbette sor" ile gelen bağlam
  let attachedContext = $state(null);

  const CONSENT_KEY = 'kukla_web_consent_v1';

  const SUGGESTIONS = [
    'Kira uyarlma davalarında içtihat ne diyor?',
    'İş kazası tazminatında kusur hesabı nasıl yapılır?',
    'Boşanmada kazanılmış mallara katılma koşulları neler?',
  ];

  function hasConsent() {
    try {
      return !!localStorage.getItem(CONSENT_KEY);
    } catch (_) {
      return false;
    }
  }

  $effect(() => {
    if (context) {
      attachedContext = { label: context.label || 'Seçili karar', text: context.text || '' };
    }
  });

  $effect(() => {
    loadSessions();
    invoke('get_web_search_status')
      .then((status) => { webStatus = status; })
      .catch(() => { webStatus = null; });
    invoke('get_ai_status')
      .then((status) => { aiStatus = status; })
      .catch(() => { aiStatus = null; });
  });

  async function loadSessions() {
    try {
      const list = await invoke('chat_sessions');
      sessions = list;
    } catch (err) {
      error = formatInvokeError(err) || 'Sohbetler yüklenemedi.';
    }
  }

  async function openSession(session) {
    activeSessionId = session.id;
    allowExternal = !!session.allow_external;
    loadingHistory = true;
    try {
      messages = await invoke('chat_history', { sessionId: session.id });
    } catch (err) {
      error = formatInvokeError(err) || 'Sohbet geçmişi yüklenemedi.';
      messages = [];
    } finally {
      loadingHistory = false;
    }
  }

  function newSession() {
    activeSessionId = null;
    messages = [];
    allowExternal = false;
    attachedContext = null;
    error = '';
    notice = '';
  }

  // İnternet modu: ilk açılışta açık onay istenir, sonra oturuma yazılır.
  function requestExternalToggle() {
    if (allowExternal) {
      applyExternal(false);
      return;
    }
    if (!hasConsent()) {
      consentPending = true;
      return;
    }
    applyExternal(true);
  }

  function confirmConsent() {
    consentPending = false;
    try {
      localStorage.setItem(CONSENT_KEY, '1');
    } catch (_) {}
    applyExternal(true);
  }

  async function applyExternal(value) {
    allowExternal = value;
    if (!activeSessionId) {
      pendingExternal = value;
      return;
    }
    try {
      await invoke('chat_set_external', { sessionId: activeSessionId, allowExternal: value });
    } catch (err) {
      error = formatInvokeError(err) || 'Paylaşım tercihi güncellenemedi.';
    }
  }

  async function deleteSession(session) {
    try {
      await invoke('chat_delete_session', { sessionId: session.id });
      if (activeSessionId === session.id) {
        activeSessionId = null;
        messages = [];
      }
      await loadSessions();
    } catch (err) {
      error = formatInvokeError(err) || 'Sohbet silinemedi.';
    }
  }

  function buildOutgoing(text) {
    if (!attachedContext) return text;
    const header = `[Bağlam: ${attachedContext.label}]\n\n${attachedContext.text}\n\nSoru: ${text}`;
    attachedContext = null;
    return header;
  }

  async function send(prefilled) {
    const raw = (prefilled ?? input).trim();
    if (!raw || sending) return;
    // Bağlam (Sohbette Sor) ilk mesaja gömülür.
    const text = buildOutgoing(raw);
    error = '';
    notice = '';
    sending = true;
    beginAiRequest();
    const requestId = aiRequestId;

    let sessionId = activeSessionId;
    if (!sessionId) {
      try {
        sessionId = await invoke('chat_new_session', { allowExternal: pendingExternal });
        activeSessionId = sessionId;
        pendingExternal = false;
      } catch (err) {
        error = formatInvokeError(err) || 'Sohbet oturumu açılamadı.';
        sending = false;
        endAiRequest();
        return;
      }
    }

    messages = [
      ...messages,
      {
        id: -Date.now(),
        session_id: sessionId,
        role: 'user',
        content: text,
        sources_json: null,
        created_at: new Date().toISOString(),
      },
    ];
    input = '';
    scrollToBottom();

    try {
      const response = await invoke('chat_send', {
        sessionId,
        message: text,
        requestId,
      });
      if (aiCancelled || requestId !== aiRequestId) return;
      notice = response.notice || '';
      messages = [
        ...messages,
        {
          id: response.message_id,
          session_id: sessionId,
          role: 'assistant',
          content: response.reply,
          sources_json: JSON.stringify(response.sources || []),
          created_at: new Date().toISOString(),
        },
      ];
      await loadSessions();
    } catch (err) {
      if (aiCancelled) return;
      error = formatInvokeError(err) || 'Yanıt alınamadı.';
    } finally {
      if (requestId === aiRequestId || !aiRequestId) endAiRequest();
      sending = false;
      scrollToBottom();
    }
  }

  function beginAiRequest() {
    aiRequestId =
      typeof crypto !== 'undefined' && crypto.randomUUID
        ? crypto.randomUUID()
        : `${Date.now()}-${Math.random().toString(16).slice(2)}`;
    aiSessionId = '';
    aiPreview = '';
    aiRetryNote = '';
    aiElapsed = 0;
    aiCancelled = false;
    if (aiTimer) clearInterval(aiTimer);
    const startedAt = Date.now();
    aiTimer = setInterval(() => {
      aiElapsed = Math.floor((Date.now() - startedAt) / 1000);
    }, 1000);
  }

  function endAiRequest() {
    if (aiTimer) {
      clearInterval(aiTimer);
      aiTimer = null;
    }
    aiRequestId = '';
    aiPreview = '';
    aiRetryNote = '';
  }

  async function cancelAiRequest() {
    aiCancelled = true;
    const sessionId = aiSessionId;
    endAiRequest();
    sending = false;
    if (sessionId) {
      try {
        await invoke('abort_ai_request', { sessionId });
      } catch (_) {}
    }
  }

  function formatElapsed(seconds) {
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m}:${String(s).padStart(2, '0')}`;
  }

  function aiBadge() {
    if (aiStatus === null) return { text: 'YZ kontrol ediliyor', cls: 'pending' };
    const cloud = aiStatus.chat_backend === 'cloud';
    if (aiStatus.opencode?.healthy || cloud) {
      return { text: cloud ? 'YZ: bulut sağlayıcı' : 'YZ: hazır', cls: 'on' };
    }
    return { text: 'YZ: sunucu bekleniyor', cls: 'pending' };
  }

  $effect(() => {
    const unlisteners = [];
    let disposed = false;
    const track = (promise) => {
      promise.then((unlisten) => {
        if (disposed) unlisten();
        else unlisteners.push(unlisten);
      }).catch(() => {});
    };
    track(
      listen('ai://session', (event) => {
        const payload = event.payload || {};
        if (payload.requestId && payload.requestId !== aiRequestId) return;
        if (payload.sessionId) aiSessionId = payload.sessionId;
      }),
    );
    track(
      listen('ai://progress', (event) => {
        const payload = event.payload || {};
        if (payload.requestId && payload.requestId !== aiRequestId) return;
        if (payload.sessionId) aiSessionId = payload.sessionId;
        if (payload.text) aiPreview = payload.text;
      }),
    );
    track(
      listen('ai://retry', (event) => {
        const payload = event.payload || {};
        if (payload.requestId && payload.requestId !== aiRequestId) return;
        aiRetryNote = 'Seçili model yanıt vermedi; yedek modelle tekrar deneniyor...';
      }),
    );
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  });

  function scrollToBottom() {
    requestAnimationFrame(() => {
      const box = document.querySelector('.chat-messages');
      if (box) box.scrollTop = box.scrollHeight;
    });
  }

  function parseSources(json) {
    if (!json) return [];
    try {
      const parsed = JSON.parse(json);
      return Array.isArray(parsed) ? parsed : [];
    } catch (_) {
      return [];
    }
  }

  function onKeydown(event) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      send();
    }
  }
</script>

<aside class="chat-drawer" aria-label="Sohbet asistanı">
  <div class="chat-header">
    <div class="chat-title">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
      </svg>
      Sohbet
      {#if allowExternal}
        <span class="chat-mode-badge web" title="Sorularınız Tavily ile internette de aranır">İnternet</span>
      {:else}
        <span class="chat-mode-badge" title="Bu modda veriler cihazınızdan çıkmaz">Yerel</span>
      {/if}
      <span class="chat-ai-badge {aiBadge().cls}" title={aiBadge().text}>{aiBadge().text}</span>
    </div>
    <button
      class="chat-internet-toggle"
      class:active={allowExternal}
      onclick={requestExternalToggle}
      title="İnternet modu: sorularınız Tavily ile internette aranır (ilk kullanımda onay istenir)"
      aria-pressed={allowExternal}
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
      </svg>
      İnternet
    </button>
    <button class="chat-new-btn" onclick={newSession} title="Yeni sohbet">
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
      </svg>
    </button>
    <button class="chat-close-btn" onclick={onclose} title="Kapat" aria-label="Sohbeti kapat">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
      </svg>
    </button>
  </div>

  {#if consentPending}
    <div class="chat-consent">
      <p>
        <strong>İnternet modu</strong> açıldığında sorularınız Tavily'ye gönderilir ve yanıtlar
        internet sonuçlarına da dayanır. Yüklediğiniz belgeler gönderilmez; yalnızca soru metni
        ve arama sonuçları paylaşılır.
      </p>
      <div class="chat-consent-actions">
        <button class="chat-consent-yes" onclick={confirmConsent}>Anladım, etkinleştir</button>
        <button class="chat-consent-no" onclick={() => (consentPending = false)}>Vazgeç</button>
      </div>
    </div>
  {/if}

  {#if allowExternal && webStatus && !webStatus.has_web_search_key}
    <div class="chat-warning">
      İnternet araması etkin ama Tavily anahtarı tanımlı değil — Ayarlar → YZ Altyapısı bölümünden
      ekleyin (ayda 1.000 arama ücretsiz).
    </div>
  {/if}

  {#if sessions.length > 0}
    <div class="chat-sessions">
      {#each sessions.slice(0, 8) as session (session.id)}
        <div class="chat-session-item" class:active={session.id === activeSessionId}>
          <button class="chat-session-open" onclick={() => openSession(session)}>
            <span class="chat-session-title">{session.title}</span>
            <span class="chat-session-count">{session.message_count}</span>
          </button>
          <button
            class="chat-session-delete"
            onclick={() => deleteSession(session)}
            title="Sohbeti sil"
            aria-label="Sohbeti sil"
          >
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
              <polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
            </svg>
          </button>
        </div>
      {/each}
    </div>
  {/if}

  <div class="chat-messages">
    {#if error}
      <div class="chat-error" role="alert">{error}</div>
    {/if}

    {#if notice}
      <div class="chat-notice" role="status">{notice}</div>
    {/if}

    {#if loadingHistory}
      <p class="chat-empty">Yükleniyor...</p>
    {:else if messages.length === 0}
      <div class="chat-empty">
        <p class="chat-empty-title">Nasıl yardımcı olabilirim?</p>
        <p class="chat-empty-hint">
          Yerel arşivinizdeki kararlar ve genel hukuki bilgilerle yanıt veririm.
        </p>
        <div class="chat-suggestions">
          {#each SUGGESTIONS as suggestion}
            <button class="chat-suggestion" onclick={() => send(suggestion)}>
              {suggestion}
            </button>
          {/each}
        </div>
      </div>
    {:else}
      {#each messages as message (message.id)}
        <div class="chat-message" class:user={message.role === 'user'}>
          <div class="chat-bubble">
            <p class="chat-text">{message.content}</p>
            {#if parseSources(message.sources_json).length > 0}
              <div class="chat-sources">
                {#each parseSources(message.sources_json) as source, index (index)}
                  <span
                    class="chat-source-chip"
                    class:web={source.kind === 'web'}
                    title={source.url || source.label}
                  >
                    [{index + 1}] {source.label}
                  </span>
                {/each}
              </div>
            {/if}
          </div>
        </div>
      {/each}

      {#if sending}
        <div class="chat-message assistant">
          <div class="chat-bubble streaming">
            {#if aiRetryNote}
              <p class="chat-text thinking">{aiRetryNote}</p>
            {/if}
            {#if aiPreview}
              <p class="chat-text">{aiPreview}</p>
            {:else if !aiRetryNote}
              <p class="chat-text thinking">Düşünüyor... {formatElapsed(aiElapsed)}</p>
            {/if}
          </div>
        </div>
      {/if}
    {/if}
  </div>

  {#if attachedContext}
    <div class="chat-context-chip">
      <span title={attachedContext.label}>Bağlam: {attachedContext.label}</span>
      <button onclick={() => (attachedContext = null)} aria-label="Bağlamı kaldır">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
    </div>
  {/if}

  <div class="chat-input-area">
    <textarea
      class="chat-input"
      bind:value={input}
      onkeydown={onKeydown}
      placeholder="Sorunuzu yazın... (Enter ile gönder)"
      rows="2"
      disabled={sending}
    ></textarea>
    {#if sending}
      <button class="chat-cancel-btn" onclick={cancelAiRequest} title="Üretimi durdur">
        Durdur
      </button>
    {:else}
      <button class="chat-send-btn" onclick={() => send()} disabled={!input.trim()} title="Gönder">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/>
        </svg>
      </button>
    {/if}
  </div>
</aside>

<style>
  .chat-drawer {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    width: 380px;
    max-width: 100%;
    background: var(--bg-secondary);
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    z-index: 30;
    box-shadow: -8px 0 24px rgba(0, 0, 0, 0.25);
  }

  .chat-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .chat-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
    flex: 1;
  }

  .chat-mode-badge {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.4px;
    text-transform: uppercase;
    color: var(--success);
    background: rgba(34, 197, 94, 0.12);
    border: 1px solid rgba(34, 197, 94, 0.3);
    border-radius: 999px;
    padding: 2px 8px;
  }

  .chat-mode-badge.web {
    color: var(--warning);
    background: rgba(234, 179, 8, 0.12);
    border-color: rgba(234, 179, 8, 0.35);
  }

  .chat-ai-badge {
    font-size: 10px;
    font-weight: 600;
    border-radius: 999px;
    padding: 2px 8px;
    border: 1px solid var(--border);
    color: var(--text-muted);
    white-space: nowrap;
  }

  .chat-ai-badge.on {
    color: var(--success);
    border-color: rgba(34, 197, 94, 0.3);
    background: rgba(34, 197, 94, 0.1);
  }

  .chat-ai-badge.pending {
    color: var(--text-muted);
  }

  .chat-internet-toggle {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: transparent;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
    padding: 4px 10px;
    cursor: pointer;
    flex-shrink: 0;
    transition: all 0.15s;
  }

  .chat-internet-toggle:hover {
    color: var(--text-primary);
    border-color: var(--text-muted);
  }

  .chat-internet-toggle.active {
    color: var(--warning);
    border-color: rgba(234, 179, 8, 0.5);
    background: rgba(234, 179, 8, 0.1);
  }

  .chat-consent {
    margin: 10px 12px 0;
    padding: 10px 12px;
    border: 1px solid rgba(234, 179, 8, 0.35);
    background: rgba(234, 179, 8, 0.08);
    border-radius: var(--radius);
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-secondary);
    flex-shrink: 0;
  }

  .chat-consent p {
    margin: 0 0 8px;
  }

  .chat-consent strong {
    color: var(--text-primary);
  }

  .chat-consent-actions {
    display: flex;
    gap: 8px;
  }

  .chat-consent-actions button {
    flex: 1;
    border-radius: var(--radius);
    font-size: 12px;
    font-weight: 600;
    padding: 6px 10px;
    cursor: pointer;
  }

  .chat-consent-yes {
    border: none;
    background: var(--warning);
    color: #1e293b;
  }

  .chat-consent-no {
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-secondary);
  }

  .chat-warning {
    margin: 10px 12px 0;
    padding: 8px 10px;
    border: 1px solid rgba(234, 179, 8, 0.3);
    background: rgba(234, 179, 8, 0.07);
    color: var(--text-secondary);
    border-radius: var(--radius);
    font-size: 11.5px;
    line-height: 1.5;
    flex-shrink: 0;
  }

  .chat-notice {
    background: rgba(99, 102, 241, 0.08);
    border: 1px solid rgba(99, 102, 241, 0.25);
    color: var(--text-secondary);
    border-radius: var(--radius);
    padding: 8px 10px;
    font-size: 11.5px;
    line-height: 1.5;
  }

  .chat-source-chip.web {
    color: var(--warning);
    border-color: rgba(234, 179, 8, 0.4);
  }

  .chat-new-btn,
  .chat-close-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border: none;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    flex-shrink: 0;
  }

  .chat-new-btn:hover,
  .chat-close-btn:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .chat-sessions {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 168px;
    overflow-y: auto;
    padding: 8px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .chat-session-item {
    display: flex;
    align-items: center;
    border-radius: var(--radius);
  }

  .chat-session-item.active {
    background: var(--accent-muted);
  }

  .chat-session-open {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 12.5px;
    padding: 7px 9px;
    cursor: pointer;
    text-align: left;
    border-radius: var(--radius);
  }

  .chat-session-item.active .chat-session-open {
    color: var(--text-primary);
  }

  .chat-session-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chat-session-count {
    font-size: 11px;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .chat-session-delete {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: none;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    margin-right: 4px;
    flex-shrink: 0;
  }

  .chat-session-delete:hover {
    background: rgba(239, 68, 68, 0.15);
    color: var(--danger);
  }

  .chat-messages {
    flex: 1;
    overflow-y: auto;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .chat-empty {
    margin: auto;
    text-align: center;
    color: var(--text-muted);
    max-width: 280px;
  }

  .chat-empty-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--text-secondary);
    margin: 0 0 6px;
  }

  .chat-empty-hint {
    font-size: 12.5px;
    margin: 0 0 14px;
    line-height: 1.5;
  }

  .chat-suggestions {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .chat-suggestion {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    font-size: 12px;
    padding: 8px 10px;
    cursor: pointer;
    text-align: left;
    transition: all 0.15s;
  }

  .chat-suggestion:hover {
    border-color: var(--accent);
    color: var(--text-primary);
  }

  .chat-message {
    display: flex;
  }

  .chat-message.user {
    justify-content: flex-end;
  }

  .chat-bubble {
    max-width: 88%;
    border-radius: var(--radius-lg);
    padding: 9px 12px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
  }

  .chat-message.user .chat-bubble {
    background: var(--accent-muted);
    border-color: rgba(99, 102, 241, 0.35);
  }

  .chat-bubble.streaming {
    border-style: dashed;
  }

  .chat-text {
    margin: 0;
    font-size: 13px;
    line-height: 1.55;
    color: var(--text-primary);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .chat-text.thinking {
    color: var(--text-muted);
    font-style: italic;
  }

  .chat-sources {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }

  .chat-source-chip {
    font-size: 10.5px;
    color: var(--text-secondary);
    background: var(--bg-primary);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 3px 8px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chat-error {
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: var(--danger);
    border-radius: var(--radius);
    padding: 8px 10px;
    font-size: 12px;
  }

  .chat-context-chip {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 0 12px 8px;
    padding: 7px 10px;
    background: var(--accent-muted);
    border: 1px solid rgba(99, 102, 241, 0.35);
    border-radius: var(--radius);
    font-size: 11.5px;
    color: var(--text-primary);
    flex-shrink: 0;
  }

  .chat-context-chip span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chat-context-chip button {
    display: inline-flex;
    align-items: center;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    padding: 2px;
    border-radius: 4px;
    flex-shrink: 0;
  }

  .chat-context-chip button:hover {
    color: var(--danger);
  }

  .chat-input-area {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    padding: 10px 12px 12px;
    border-top: 1px solid var(--border);
    flex-shrink: 0;
  }

  .chat-input {
    flex: 1;
    resize: none;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-primary);
    color: var(--text-primary);
    font-family: var(--font);
    font-size: 13px;
    line-height: 1.5;
    padding: 9px 10px;
  }

  .chat-input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .chat-input:disabled {
    opacity: 0.6;
  }

  .chat-send-btn,
  .chat-cancel-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: var(--radius);
    cursor: pointer;
    flex-shrink: 0;
    font-size: 12px;
    font-weight: 600;
  }

  .chat-send-btn {
    width: 36px;
    height: 36px;
    background: var(--accent);
    color: #fff;
  }

  .chat-send-btn:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .chat-send-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .chat-cancel-btn {
    height: 36px;
    padding: 0 12px;
    background: rgba(239, 68, 68, 0.15);
    color: var(--danger);
    border: 1px solid rgba(239, 68, 68, 0.3);
  }

  .chat-cancel-btn:hover {
    background: rgba(239, 68, 68, 0.25);
  }
</style>
