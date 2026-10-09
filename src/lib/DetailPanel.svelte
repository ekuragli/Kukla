<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { revealItemInDir } from '@tauri-apps/plugin-opener';
  import { formatDecisionDate, formatInvokeError } from './formatters.js';

  let { result, onback, searchQuery = '', onaskchat = null } = $props();

  let summary = $state('');
  let ratioDecidendi = $state('');
  let fullText = $state('');
  let sourceUrl = $state('');
  let sourceCitation = $state('');
  let hallucinationScore = $state(1.0);
  let loadingSummary = $state(false);
  let loadingDoc = $state(false);
  let summaryError = $state('');
  let docError = $state('');
  let exporting = $state(false);
  let exportMsg = $state('');
  let exportFormat = $state('txt');
  let copyMsg = $state('');
  let caseFacts = $state('');
  let petitionDraft = $state('');
  let citedDecisions = $state('');
  let petitionDisclaimer = $state('');
  let petitionHallucination = $state(1.0);
  let loadingPetition = $state(false);
  let petitionError = $state('');
  let petitionExportFormat = $state('docx');
  let petitionExportMsg = $state('');
  let petitionExporting = $state(false);
  let petitionCopyMsg = $state('');
  let petitionTemplates = $state([]);
  let selectedPetitionType = $state('genel');
  let petitionTemplateLabel = $state('');
  let aiRequestId = $state('');
  let aiSessionId = $state('');
  let aiPreview = $state('');
  let aiElapsed = $state(0);
  let aiCancelled = $state(false);
  let aiRetryNote = $state('');
  let aiTimer = null;

  const MIN_SUMMARY_CHARS = 500;
  const MIN_PETITION_CONTEXT_CHARS = 200;
  const MIN_CASE_FACTS_CHARS = 30;

  $effect(() => {
    invoke('get_petition_templates')
      .then((templates) => { petitionTemplates = templates; })
      .catch(() => {
        petitionTemplates = [{
          id: 'genel',
          label: 'Genel Dilekçe',
          description: 'Esnek yapı; çoğu duruma uyarlanabilir genel taslak.',
        }];
      });
  });

  function selectedTemplateDescription() {
    const match = petitionTemplates.find((t) => t.id === selectedPetitionType);
    return match?.description || '';
  }

  $effect(() => {
    const _id = result.decision.id;
    summary = '';
    ratioDecidendi = '';
    fullText = '';
    sourceUrl = '';
    sourceCitation = '';
    hallucinationScore = 1.0;
    summaryError = '';
    docError = '';
    exportMsg = '';
    copyMsg = '';
    caseFacts = searchQuery || '';
    petitionDraft = '';
    citedDecisions = '';
    petitionDisclaimer = '';
    petitionHallucination = 1.0;
    petitionError = '';
    petitionExportMsg = '';
    petitionCopyMsg = '';
    petitionTemplateLabel = '';
    aiRequestId = '';
    aiSessionId = '';
    aiPreview = '';
    aiElapsed = 0;
    aiCancelled = false;
    if (aiTimer) {
      clearInterval(aiTimer);
      aiTimer = null;
    }
  });

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
        aiRetryNote = 'Seçili model yanıt vermedi; yedek modelle tekrar deneniyor…';
      }),
    );
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  });

  function beginAiRequest() {
    aiRequestId =
      typeof crypto !== 'undefined' && crypto.randomUUID
        ? crypto.randomUUID()
        : `${Date.now()}-${Math.random().toString(16).slice(2)}`;
    aiSessionId = '';
    aiPreview = '';
    aiElapsed = 0;
    aiCancelled = false;
    aiRetryNote = '';
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
    loadingSummary = false;
    loadingPetition = false;
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

  function documentCommand() {
    if (result.sourceType === 'yargitay') return 'get_yargitay_document';
    if (result.sourceType === 'bedesten') return 'get_bedesten_document';
    return null;
  }

  function documentSourceArg() {
    if (result.sourceType !== 'bedesten') return {};
    const src = result.decision?.source;
    return src ? { source: src } : {};
  }

  $effect(() => {
    const command = documentCommand();
    if (!command) return;
    const docId = result.decision.id;
    if (!docId) return;

    let cancelled = false;
    loadingDoc = true;
    docError = '';

    invoke(command, { documentId: String(docId), ...documentSourceArg(),
      esasNo: result.decision.esas_no || null,
      kararNo: result.decision.karar_no || null,
      daire: result.decision.daire || null,
      kararTarihi: result.decision.karar_tarihi || null,
    })
      .then((doc) => {
        if (cancelled) return;
        fullText = doc.summary;
        sourceUrl = doc.source_citation;
      })
      .catch((err) => {
        if (cancelled) return;
        docError = formatInvokeError(err) || 'Döküman alınamadı.';
      })
      .finally(() => {
        if (!cancelled) loadingDoc = false;
      });

    return () => {
      cancelled = true;
    };
  });

  function summaryContext() {
    return fullText || result.snippet || (result.decision.summary || '');
  }

  function canSummarize() {
    return summaryContext().trim().length >= MIN_SUMMARY_CHARS;
  }

  function canDraftPetition() {
    return summaryContext().trim().length >= MIN_PETITION_CONTEXT_CHARS;
  }

  function canGeneratePetition() {
    return canDraftPetition() && caseFacts.trim().length >= MIN_CASE_FACTS_CHARS;
  }

  function decisionCitationHeader() {
    const refs = [];
    if (result.decision.esas_no) refs.push(`Esas: ${result.decision.esas_no}`);
    if (result.decision.karar_no) refs.push(`Karar: ${result.decision.karar_no}`);
    if (result.decision.daire) refs.push(result.decision.daire);
    const date = formatDecisionDate(result.decision.karar_tarihi);
    if (date) refs.push(date);
    return refs.length ? refs.join(' | ') : 'Kaynak bilgisi yok';
  }

  function formatWithCitation(text, section) {
    return `${text}\n\n[${decisionCitationHeader()}]\n— Kukla İçtihat Raporu (${section})`;
  }

  async function copyToClipboard(text, withCitation = false, section = 'Özet') {
    const payload = withCitation ? formatWithCitation(text, section) : text;
    try {
      await navigator.clipboard.writeText(payload);
      copyMsg = 'Kopyalandı!';
      setTimeout(() => copyMsg = '', 2000);
    } catch (_) {
      copyMsg = 'Kopyalanamadı';
      setTimeout(() => copyMsg = '', 2000);
    }
  }

  async function loadSummary() {
    loadingSummary = true;
    summaryError = '';
    beginAiRequest();
    const requestId = aiRequestId;
    try {
      const llmResponse = await invoke('summarize_decision', {
        decisionId: result.decision.id,
        context: summaryContext(),
        query: searchQuery || null,
        requestId,
      });
      if (aiCancelled || requestId !== aiRequestId) return;
      summary = llmResponse.summary;
      ratioDecidendi = llmResponse.ratio_decidendi;
      sourceCitation = llmResponse.source_citation;
      hallucinationScore = llmResponse.hallucination_score;
    } catch (err) {
      if (aiCancelled) return;
      summaryError = formatInvokeError(err) || 'Özet alınamadı.';
    } finally {
      if (requestId === aiRequestId || !aiRequestId) endAiRequest();
      loadingSummary = false;
    }
  }

  async function openInBrowser() {
    if (!fullText) return;
    try {
      await invoke('open_decision_in_browser', {
        title: decisionCitationHeader(),
        body: fullText,
        sourceType: result.sourceType,
        externalUrl: sourceUrl || null,
      });
    } catch (err) {
      docError = formatInvokeError(err) || 'Tarayıcıda açılamadı.';
    }
  }

  async function loadFullText() {
    const command = documentCommand();
    if (!command) return;
    loadingDoc = true;
    docError = '';
    try {
      const docId = String(result.decision.id);
      const doc = await invoke(command, { documentId: docId, ...documentSourceArg() });
      fullText = doc.summary;
      sourceUrl = doc.source_citation;
    } catch (err) {
      docError = formatInvokeError(err) || 'Döküman alınamadı.';
    } finally {
      loadingDoc = false;
    }
  }

  async function generatePetition() {
    if (!canGeneratePetition() || loadingPetition) return;
    loadingPetition = true;
    petitionError = '';
    petitionExportMsg = '';
    beginAiRequest();
    const requestId = aiRequestId;
    try {
      const response = await invoke('generate_petition_draft', {
        decisionId: result.decision.id,
        caseFacts: caseFacts.trim(),
        decisionContext: summaryContext(),
        citation: decisionCitationHeader(),
        ratioDecidendi: ratioDecidendi || null,
        aiSummary: summary || null,
        petitionType: selectedPetitionType,
        requestId,
      });
      if (aiCancelled || requestId !== aiRequestId) return;
      petitionDraft = response.draft;
      citedDecisions = response.cited_decisions;
      petitionDisclaimer = response.disclaimer;
      petitionHallucination = response.hallucination_score;
      petitionTemplateLabel = response.template_label || '';
    } catch (err) {
      if (aiCancelled) return;
      petitionError = formatInvokeError(err) || 'Dilekçe taslağı oluşturulamadı.';
    } finally {
      if (requestId === aiRequestId || !aiRequestId) endAiRequest();
      loadingPetition = false;
    }
  }

  async function exportPetition() {
    if (!petitionDraft || petitionExporting) return;
    petitionExporting = true;
    petitionExportMsg = '';
    try {
      const path = await invoke('export_petition_draft', {
        draft: petitionDraft,
        citedDecisions: citedDecisions || null,
        disclaimer: petitionDisclaimer || null,
        citation: decisionCitationHeader(),
        format: petitionExportFormat,
        templateLabel: petitionTemplateLabel || null,
      });
      try {
        await revealItemInDir(path);
      } catch (_) {}
      petitionExportMsg = `Dilekçe kaydedildi: ${path}`;
    } catch (err) {
      petitionExportMsg = formatInvokeError(err) || 'Dışa aktarılamadı.';
    } finally {
      petitionExporting = false;
    }
  }

  async function copyPetition(withCitation = false) {
    let payload = petitionDraft;
    if (withCitation) {
      payload = `${petitionDraft}\n\n[${decisionCitationHeader()}]\n— Kukla Dilekçe Taslağı`;
    }
    try {
      await navigator.clipboard.writeText(payload);
      petitionCopyMsg = 'Kopyalandı!';
      setTimeout(() => petitionCopyMsg = '', 2000);
    } catch (_) {
      petitionCopyMsg = 'Kopyalanamadı';
      setTimeout(() => petitionCopyMsg = '', 2000);
    }
  }

  async function handleExport() {
    if (exporting) return;
    exporting = true;
    exportMsg = '';
    try {
      const path = await invoke('export_report', {
        decisionIds: [result.decision.id],
        format: exportFormat,
        contexts: [{
          decisionId: result.decision.id,
          sourceType: result.sourceType || null,
          esasNo: result.decision.esas_no || null,
          kararNo: result.decision.karar_no || null,
          daire: result.decision.daire || null,
          fullText: fullText || null,
          summary: summary || result.decision.summary || null,
          ratioDecidendi: ratioDecidendi || null,
          sourceCitation: sourceCitation || null,
        }],
      });
      try {
        await revealItemInDir(path);
      } catch (_) {}
      exportMsg = `Rapor kaydedildi: ${path}`;
    } catch (err) {
      exportMsg = formatInvokeError(err) || 'Dışa aktarılamadı.';
    } finally {
      exporting = false;
    }
  }

  const sectionFormats = $state({ karar: 'txt', ozet: 'txt', ratio: 'txt' });
  const sectionMsgs = $state({});
  const sectionBusy = $state({});

  async function exportSection(kind, title, body) {
    if (!body || !body.trim() || sectionBusy[kind]) return;
    const format = sectionFormats[kind] || 'txt';
    sectionMsgs[kind] = '';
    sectionBusy[kind] = true;
    try {
      const path = await invoke('export_section', { kind, title, body, format });
      try {
        await revealItemInDir(path);
      } catch (_) {}
      sectionMsgs[kind] = `Kaydedildi: ${path}`;
    } catch (err) {
      sectionMsgs[kind] = formatInvokeError(err) || 'Dışa aktarılamadı.';
    } finally {
      sectionBusy[kind] = false;
    }
  }

  function getSourceLabel(source) {
    const labels = { Yargitay: 'Yargıtay', Danistay: 'Danıştay', Bam: 'BAM', UserUploaded: 'Kullanıcı Yüklemesi' };
    return labels[source] || source;
  }

  function isOnlineResult() {
    return result.sourceType === 'yargitay' || result.sourceType === 'bedesten';
  }

  function onlineSourceLabel() {
    if (result.sourceType === 'yargitay') return 'Yargıtay (resmi)';
    if (result.sourceType === 'bedesten') return 'Bedesten';
    return getSourceLabel(result.decision.source);
  }
  function askChat() {
    if (!onaskchat || !fullText) return;
    onaskchat({
      label: decisionCitationHeader(),
      text: fullText,
    });
  }
</script>

<div class="detail-panel">
  <div class="detail-header">
    <button class="back-btn" onclick={onback}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <polyline points="15 18 9 12 15 6"/>
      </svg>
      Sonuçlara Dön
    </button>
    <div class="header-meta">
      <span class="source-label" class:online={isOnlineResult()}>
        {isOnlineResult() ? onlineSourceLabel() : getSourceLabel(result.decision.source)}
      </span>
      {#if result.similarity_percent > 0}
        <span class="score-badge">%{result.similarity_percent} benzerlik</span>
      {:else if isOnlineResult()}
        <span class="score-badge online">Çevrimiçi</span>
      {/if}
      <select class="export-format" bind:value={exportFormat} disabled={exporting} aria-label="Dışa aktarma formatı">
        <option value="txt">TXT</option>
        <option value="docx">DOCX</option>
        <option value="pdf">PDF</option>
      </select>
      <button type="button" class="export-btn" onclick={handleExport} disabled={exporting}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
        </svg>
        {exporting ? 'Aktarılıyor...' : 'Dışa Aktar'}
      </button>
      {#if onaskchat}
        <button type="button" class="ask-chat-btn" onclick={askChat} disabled={!fullText} title="Kararı sohbet asistanına sor">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
          </svg>
          Sohbette Sor
        </button>
      {/if}
    </div>
  </div>

  <div class="detail-body">
    {#if exportMsg}
      <div class="export-msg">{exportMsg}</div>
    {/if}

    <div class="detail-section">
      <h2>Karar Bilgileri</h2>
      <div class="info-grid">
        {#if result.decision.esas_no}
          <div class="info-item">
            <span class="info-label">Esas No</span>
            <span class="info-value">{result.decision.esas_no}</span>
          </div>
        {/if}
        {#if result.decision.karar_no}
          <div class="info-item">
            <span class="info-label">Karar No</span>
            <span class="info-value">{result.decision.karar_no}</span>
          </div>
        {/if}
        {#if result.decision.daire}
          <div class="info-item">
            <span class="info-label">Daire</span>
            <span class="info-value">{result.decision.daire}</span>
          </div>
        {/if}
        {#if formatDecisionDate(result.decision.karar_tarihi)}
          <div class="info-item">
            <span class="info-label">Karar Tarihi</span>
            <span class="info-value">{formatDecisionDate(result.decision.karar_tarihi)}</span>
          </div>
        {/if}
      </div>
    </div>

    {#if loadingDoc}
      <div class="loading-summary">
        <div class="spinner"></div>
        <p>Döküman indiriliyor...</p>
      </div>
    {/if}

    {#if docError}
      <div class="error-box">
        <span>{docError}</span>
        <button class="error-retry-btn" onclick={loadFullText}>Tekrar Dene</button>
      </div>
    {/if}

    {#if fullText}
      <div class="detail-section">
        <div class="section-header">
          <h2>Karar Metni</h2>
          <div class="copy-actions">
            <button class="copy-btn" onclick={() => copyToClipboard(fullText, true, 'Karar Metni')}>Kaynaklı Kopyala</button>
            <select class="section-format" bind:value={sectionFormats.karar} aria-label="Karar metni dosya biçimi">
              <option value="txt">TXT</option>
              <option value="docx">DOCX</option>
              <option value="pdf">PDF</option>
            </select>
            <button class="copy-btn" disabled={sectionBusy.karar} onclick={() => exportSection('karar_metni', 'Karar Metni', fullText)}>
              {sectionBusy.karar ? 'Kaydediliyor…' : 'İndir'}
            </button>
          </div>
        </div>
        {#if sectionMsgs.karar_metni}
          <span class="section-export-msg">{sectionMsgs.karar_metni}</span>
        {/if}
        <div class="summary-text">{fullText}</div>
        {#if fullText}
          <p class="source-link">
            <button type="button" class="source-open-btn" onclick={openInBrowser}>
              {result.sourceType === 'yargitay' ? 'Tarayıcıda görüntüle' : 'Adalet Bakanlığı\'nda görüntüle'}
            </button>
          </p>
        {/if}
      </div>
    {/if}

    {#if canSummarize()}
      {#if !summary && !loadingSummary && !summaryError}
        <div class="summary-prompt">
          <p>Bu kararın YZ özetini ve <em>ratio decidendi</em> analizini görmek ister misiniz?</p>
          <button class="summarize-btn" onclick={loadSummary}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/>
            </svg>
            Özet Oluştur
          </button>
        </div>
      {/if}

      {#if loadingSummary}
        <div class="loading-summary">
          <div class="spinner"></div>
          <p>Özet hazırlanıyor... {formatElapsed(aiElapsed)}</p>
          {#if aiRetryNote}
            <span class="ai-retry-note">{aiRetryNote}</span>
          {/if}
          {#if aiPreview}
            <div class="ai-preview">{aiPreview}</div>
          {/if}
          <button class="cancel-ai-btn" onclick={cancelAiRequest}>İptal</button>
        </div>
      {/if}

      {#if summaryError}
        <div class="error-box">
          <span>{summaryError}</span>
          <button class="error-retry-btn" onclick={loadSummary}>Tekrar Dene</button>
        </div>
      {/if}

      {#if summary}
        {#if hallucinationScore < 0.7}
          <div class="hallucination-warning">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
            </svg>
            <span>Düşük kaynak benzerliği (%{Math.round(hallucinationScore * 100)}). Bu özet kaynak metinle uyumlu olmayabilir. Lütfen doğrulayın.</span>
          </div>
        {:else}
          <div class="hallucination-ok">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="20 6 9 17 4 12"/>
            </svg>
            <span>Kaynak benzerliği: %{Math.round(hallucinationScore * 100)}</span>
          </div>
        {/if}

        <div class="detail-section">
          <div class="section-header">
            <h2>YZ Özeti</h2>
            <div class="copy-actions">
              <button class="copy-btn" onclick={() => copyToClipboard(summary)}>
                {copyMsg || 'Kopyala'}
              </button>
              <button class="copy-btn" onclick={() => copyToClipboard(summary, true, 'YZ Özeti')}>
                Kaynaklı Kopyala
              </button>
              <select class="section-format" bind:value={sectionFormats.ozet} aria-label="Özet dosya biçimi">
                <option value="txt">TXT</option>
                <option value="docx">DOCX</option>
                <option value="pdf">PDF</option>
              </select>
              <button class="copy-btn" disabled={sectionBusy.ozet} onclick={() => exportSection('ozet', 'YZ Özeti', summary)}>
                {sectionBusy.ozet ? 'Kaydediliyor…' : 'İndir'}
              </button>
            </div>
          </div>
          {#if sectionMsgs.ozet}
            <span class="section-export-msg">{sectionMsgs.ozet}</span>
          {/if}
          <p class="summary-text">{summary}</p>
        </div>
      {/if}

      {#if sourceCitation}
        <div class="detail-section">
          <div class="section-header">
            <h2>Kaynak Referansı</h2>
            <button class="copy-btn" onclick={() => copyToClipboard(sourceCitation, true, 'Kaynak')}>Kaynaklı Kopyala</button>
          </div>
          <p class="citation-text">{sourceCitation}</p>
        </div>
      {/if}

      {#if ratioDecidendi}
        <div class="detail-section">
          <div class="section-header">
            <h2>Ratio Decidendi</h2>
            <div class="copy-actions">
              <button class="copy-btn" onclick={() => copyToClipboard(ratioDecidendi)}>Kopyala</button>
              <button class="copy-btn" onclick={() => copyToClipboard(ratioDecidendi, true, 'Ratio Decidendi')}>Kaynaklı Kopyala</button>
              <select class="section-format" bind:value={sectionFormats.ratio} aria-label="Ratio decidendi dosya biçimi">
                <option value="txt">TXT</option>
                <option value="docx">DOCX</option>
                <option value="pdf">PDF</option>
              </select>
              <button class="copy-btn" disabled={sectionBusy.ratio} onclick={() => exportSection('ratio', 'Ratio Decidendi', ratioDecidendi)}>
                {sectionBusy.ratio ? 'Kaydediliyor…' : 'İndir'}
              </button>
            </div>
          </div>
          {#if sectionMsgs.ratio}
            <span class="section-export-msg">{sectionMsgs.ratio}</span>
          {/if}
          <p class="summary-text">{ratioDecidendi}</p>
        </div>
      {/if}
    {/if}

    {#if canDraftPetition()}
      <div class="detail-section petition-section">
        <h2>Dilekçe Taslağı</h2>
        <p class="petition-hint">
          Seçili içtihata dayalı dilekçe taslağı oluşturun. Metin YZ desteklidir; kullanmadan önce gözden geçirin.
        </p>

        <label class="petition-label" for="petition-type">Dilekçe Türü</label>
        <select
          id="petition-type"
          class="petition-type-select"
          bind:value={selectedPetitionType}
          disabled={loadingPetition}
        >
          {#each petitionTemplates as template}
            <option value={template.id}>{template.label}</option>
          {/each}
        </select>
        {#if selectedTemplateDescription()}
          <p class="petition-field-hint">{selectedTemplateDescription()}</p>
        {/if}

        <label class="petition-label" for="case-facts">Müvekkil / Dava Özeti</label>
        <textarea
          id="case-facts"
          class="case-facts-input"
          bind:value={caseFacts}
          rows="4"
          placeholder="Davanın olay örgüsünü, tarafları ve talebinizi kısaca yazın..."
          disabled={loadingPetition}
        ></textarea>

        {#if !petitionDraft && !loadingPetition && !petitionError}
          <button
            class="summarize-btn petition-btn"
            onclick={generatePetition}
            disabled={!canGeneratePetition()}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
              <polyline points="14 2 14 8 20 8"/>
              <line x1="16" y1="13" x2="8" y2="13"/>
              <line x1="16" y1="17" x2="8" y2="17"/>
            </svg>
            Dilekçe Taslağı Oluştur
          </button>
          {#if caseFacts.trim().length > 0 && caseFacts.trim().length < MIN_CASE_FACTS_CHARS}
            <p class="petition-field-hint">Dava özeti en az {MIN_CASE_FACTS_CHARS} karakter olmalıdır.</p>
          {/if}
        {/if}

        {#if loadingPetition}
          <div class="loading-summary">
            <div class="spinner"></div>
            <p>Dilekçe taslağı hazırlanıyor... {formatElapsed(aiElapsed)}</p>
            {#if aiRetryNote}
              <span class="ai-retry-note">{aiRetryNote}</span>
            {/if}
            {#if aiPreview}
              <div class="ai-preview">{aiPreview}</div>
            {/if}
            <button class="cancel-ai-btn" onclick={cancelAiRequest}>İptal</button>
          </div>
        {/if}

        {#if petitionError}
          <div class="error-box">
            <span>{petitionError}</span>
            <button class="error-retry-btn" onclick={generatePetition} disabled={!canGeneratePetition()}>Tekrar Dene</button>
          </div>
        {/if}

        {#if petitionDraft}
          {#if petitionHallucination < 0.7}
            <div class="hallucination-warning">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
              </svg>
              <span>Düşük kaynak benzerliği (%{Math.round(petitionHallucination * 100)}). Taslağı içtihat metniyle karşılaştırarak doğrulayın.</span>
            </div>
          {:else}
            <div class="hallucination-ok">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="20 6 9 17 4 12"/>
              </svg>
              <span>Kaynak benzerliği: %{Math.round(petitionHallucination * 100)}</span>
            </div>
          {/if}

          <div class="section-header">
            <h2>{petitionTemplateLabel ? `${petitionTemplateLabel} — Taslak` : 'Taslak Metin'}</h2>
            <div class="copy-actions">
              <button class="copy-btn" onclick={() => copyPetition(false)}>
                {petitionCopyMsg || 'Kopyala'}
              </button>
              <button class="copy-btn" onclick={() => copyPetition(true)}>Kaynaklı Kopyala</button>
              <select class="export-format" bind:value={petitionExportFormat} disabled={petitionExporting} aria-label="Dilekçe dışa aktarma formatı">
                <option value="txt">TXT</option>
                <option value="docx">DOCX</option>
                <option value="pdf">PDF</option>
              </select>
              <button type="button" class="export-btn" onclick={exportPetition} disabled={petitionExporting}>
                {petitionExporting ? 'Aktarılıyor...' : 'Dışa Aktar'}
              </button>
            </div>
          </div>
          <p class="summary-text">{petitionDraft}</p>

          {#if citedDecisions}
            <div class="detail-section nested-section">
              <h2>İçtihat Referansları</h2>
              <p class="citation-text">{citedDecisions}</p>
            </div>
          {/if}

          {#if petitionDisclaimer}
            <p class="petition-disclaimer">{petitionDisclaimer}</p>
          {/if}

          {#if petitionExportMsg}
            <div class="export-msg">{petitionExportMsg}</div>
          {/if}

          <button class="copy-btn regenerate-btn" onclick={generatePetition} disabled={loadingPetition || !canGeneratePetition()}>
            Yeniden Oluştur
          </button>
        {/if}
      </div>
    {/if}

    {#if !fullText && !isOnlineResult()}
      <div class="detail-section">
        <div class="section-header">
          <h2>Karar Metni (Önizleme)</h2>
          <button class="copy-btn" onclick={() => copyToClipboard(result.snippet || (result.decision.summary || ''), true, 'Önizleme')}>Kaynaklı Kopyala</button>
        </div>
        <p class="snippet-full">{result.snippet || (result.decision.summary || '')}</p>
      </div>
    {/if}
  </div>
</div>

<style>
  .detail-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .detail-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 24px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .back-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    background: transparent;
    color: var(--text-secondary);
    font-size: 13px;
    padding: 6px 10px;
    border-radius: var(--radius);
    transition: all 0.15s;
  }

  .back-btn:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .header-meta {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .source-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--accent);
    background: var(--accent-muted);
    padding: 3px 8px;
    border-radius: 4px;
  }

  .source-label.online {
    color: #38bdf8;
    background: rgba(56, 189, 248, 0.1);
  }

  .score-badge {
    font-size: 12px;
    font-weight: 600;
    color: #4ade80;
    background: rgba(34, 197, 94, 0.1);
    padding: 3px 8px;
    border-radius: 4px;
  }

  .score-badge.online {
    color: #38bdf8;
    background: rgba(56, 189, 248, 0.1);
  }

  .export-format {
    font-size: 12px;
    padding: 5px 8px;
    border-radius: var(--radius);
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    border: 1px solid var(--border);
  }

  .export-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
    background: var(--bg-tertiary);
    padding: 5px 10px;
    border-radius: var(--radius);
    transition: all 0.15s;
  }

  .export-btn:hover:not(:disabled) {
    background: var(--accent-muted);
    color: var(--accent);
  }

  .export-btn:disabled {
    opacity: 0.5;
  }

  .ask-chat-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    font-weight: 600;
    color: #fff;
    background: var(--accent);
    padding: 5px 10px;
    border-radius: var(--radius);
    transition: all 0.15s;
  }

  .ask-chat-btn:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .ask-chat-btn:disabled {
    opacity: 0.5;
  }

  .export-msg {
    background: rgba(34, 197, 94, 0.1);
    color: #4ade80;
    padding: 8px 14px;
    border-radius: var(--radius);
    font-size: 12px;
    margin-bottom: 16px;
  }

  .detail-body {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
  }

  .detail-section {
    margin-bottom: 24px;
  }

  h2 {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 12px;
  }

  .info-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 12px;
  }

  .info-item {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 12px;
  }

  .info-label {
    display: block;
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 4px;
  }

  .info-value {
    font-size: 14px;
    font-weight: 500;
  }

  .summary-prompt {
    background: var(--accent-muted);
    border: 1px solid rgba(99, 102, 241, 0.3);
    border-radius: var(--radius);
    padding: 20px;
    text-align: center;
    margin-bottom: 24px;
  }

  .summary-prompt p {
    color: var(--text-secondary);
    margin-bottom: 12px;
    font-size: 14px;
  }

  .summarize-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: var(--accent);
    color: white;
    padding: 8px 16px;
    border-radius: var(--radius);
    font-weight: 600;
    transition: background 0.15s;
  }

  .summarize-btn:hover {
    background: var(--accent-hover);
  }

  .loading-summary {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 24px;
    color: var(--text-secondary);
    font-size: 13px;
  }

  .ai-preview {
    width: 100%;
    max-height: 160px;
    overflow: hidden;
    text-align: left;
    white-space: pre-wrap;
    word-break: break-word;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 10px 12px;
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-secondary);
    opacity: 0.9;
  }

  .cancel-ai-btn {
    background: rgba(239, 68, 68, 0.12);
    color: var(--danger);
    border: 1px solid rgba(239, 68, 68, 0.35);
    padding: 4px 14px;
    border-radius: var(--radius);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }

  .cancel-ai-btn:hover {
    background: rgba(239, 68, 68, 0.22);
  }

  .ai-retry-note {
    font-size: 12px;
    color: var(--warning, #d97706);
    text-align: center;
    max-width: 420px;
  }

  .spinner {
    width: 24px;
    height: 24px;
    border: 3px solid var(--bg-tertiary);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .error-box {
    background: rgba(239, 68, 68, 0.1);
    color: var(--danger);
    padding: 10px 14px;
    border-radius: var(--radius);
    font-size: 13px;
    margin-bottom: 24px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }

  .error-retry-btn {
    background: rgba(239, 68, 68, 0.2);
    color: var(--danger);
    padding: 4px 12px;
    border-radius: var(--radius);
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
  }

  .error-retry-btn:hover {
    background: rgba(239, 68, 68, 0.3);
  }

  .summary-text {
    font-size: 14px;
    line-height: 1.7;
    color: var(--text-primary);
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 16px;
    white-space: pre-wrap;
  }

  .snippet-full {
    font-size: 13px;
    line-height: 1.7;
    color: var(--text-secondary);
  }

  .source-link {
    margin-top: 12px;
    font-size: 13px;
  }

  .source-open-btn {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent);
    font-size: 13px;
    cursor: pointer;
    text-decoration: underline;
  }

  .source-open-btn:hover {
    color: var(--accent-hover);
  }

  .hallucination-warning {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: var(--danger);
    padding: 10px 14px;
    border-radius: var(--radius);
    font-size: 13px;
    margin-bottom: 24px;
  }

  .hallucination-warning svg {
    flex-shrink: 0;
    margin-top: 1px;
  }

  .hallucination-ok {
    display: flex;
    align-items: center;
    gap: 6px;
    background: rgba(34, 197, 94, 0.1);
    color: #4ade80;
    padding: 8px 14px;
    border-radius: var(--radius);
    font-size: 12px;
    margin-bottom: 16px;
  }

  .citation-text {
    font-size: 13px;
    color: var(--accent);
    background: var(--accent-muted);
    border: 1px solid rgba(99, 102, 241, 0.2);
    border-radius: var(--radius);
    padding: 12px;
    font-style: italic;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }

  .section-header h2 {
    margin-bottom: 0;
  }

  .copy-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--text-muted);
    background: var(--bg-tertiary);
    padding: 4px 8px;
    border-radius: var(--radius);
    transition: all 0.15s;
  }

  .copy-btn:hover {
    background: var(--accent-muted);
    color: var(--accent);
  }

  .copy-actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    align-items: center;
  }

  .section-format {
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 3px 6px;
    cursor: pointer;
  }

  .section-format:hover {
    border-color: var(--accent);
  }

  .section-export-msg {
    display: block;
    font-size: 11px;
    color: var(--text-muted);
    word-break: break-all;
    margin-bottom: 8px;
  }

  .petition-section {
    border-top: 1px solid var(--border);
    padding-top: 24px;
  }

  .petition-hint {
    font-size: 13px;
    color: var(--text-secondary);
    margin-bottom: 12px;
    line-height: 1.5;
  }

  .petition-label {
    display: block;
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 6px;
    margin-top: 12px;
  }

  .petition-label:first-of-type {
    margin-top: 0;
  }

  .petition-type-select {
    width: 100%;
    padding: 10px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 14px;
    margin-bottom: 4px;
  }

  .petition-type-select:focus {
    outline: none;
    border-color: var(--accent);
  }

  .case-facts-input {
    width: 100%;
    min-height: 96px;
    padding: 12px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 14px;
    line-height: 1.6;
    resize: vertical;
    margin-bottom: 12px;
  }

  .case-facts-input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .petition-btn {
    margin-bottom: 8px;
  }

  .petition-field-hint {
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 4px;
  }

  .petition-disclaimer {
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
    margin-top: 12px;
    padding: 10px 12px;
    background: var(--bg-tertiary);
    border-radius: var(--radius);
  }

  .nested-section {
    margin-top: 16px;
    margin-bottom: 0;
  }

  .regenerate-btn {
    margin-top: 12px;
  }
</style>
