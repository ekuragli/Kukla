<script>
  import { invoke } from '@tauri-apps/api/core';
  import { formatInvokeError } from './formatters.js';
  import { open } from '@tauri-apps/plugin-dialog';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  let { onsearchfrompdf } = $props();

  let uploading = $state(false);
  let result = $state(null);
  let error = $state('');
  let dropActive = $state(false);
  let progressMsg = $state('');

  async function handlePickFile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (selected) {
      await uploadFile(selected);
    }
  }

  async function uploadFile(filePath) {
    uploading = true;
    error = '';
    result = null;
    progressMsg = 'PDF işleniyor...';
    try {
      const msg = await invoke('upload_and_index_pdf', { filePath });
      result = msg;
      progressMsg = msg.page_count ? `${msg.page_count} sayfa işlendi.` : '';
    } catch (err) {
      error = formatInvokeError(err) || 'PDF yüklenirken hata oluştu.';
    } finally {
      uploading = false;
      if (!result) progressMsg = '';
    }
  }

  function searchWithExtractedText() {
    const text = result?.suggested_query || result?.text;
    if (!text?.trim() || !onsearchfrompdf) return;
    onsearchfrompdf(text.trim());
  }

  $effect(() => {
    let unlisten = null;

    getCurrentWindow().onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === 'over') {
        dropActive = true;
      } else if (payload.type === 'drop') {
        dropActive = false;
        const pdfPath = payload.paths.find((p) => p.toLowerCase().endsWith('.pdf'));
        if (pdfPath) {
          uploadFile(pdfPath);
        } else if (payload.paths.length > 0) {
          error = 'Yalnızca .pdf dosyaları desteklenir.';
        }
      } else {
        dropActive = false;
      }
    }).then((fn) => {
      unlisten = fn;
    }).catch(() => {});

    return () => {
      unlisten?.();
    };
  });
</script>

<div class="upload-panel">
  <div class="upload-header">
    <h1>PDF Yükle</h1>
    <p class="subtitle">Yargıtay, Danıştay veya BAM kararlarını PDF olarak yükleyin</p>
  </div>

  <div
    class="drop-zone"
    class:active={dropActive}
    onclick={handlePickFile}
    role="button"
    tabindex="0"
    aria-label="PDF dosyası seçmek için tıklayın veya sürükleyin"
    onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handlePickFile(); } }}
  >
    <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
      <polyline points="17 8 12 3 7 8"/>
      <line x1="12" y1="3" x2="12" y2="15"/>
    </svg>
    {#if uploading}
      <div class="uploading-state">
        <div class="spinner" role="status" aria-label="PDF işleniyor"></div>
        <p>{progressMsg || 'PDF işleniyor...'}</p>
      </div>
    {:else}
      <p class="drop-text">PDF dosyasını sürükleyin veya seçmek için tıklayın</p>
      <p class="drop-hint">Yalnızca .pdf dosyaları desteklenir</p>
    {/if}
  </div>

  {#if error}
    <div class="error-box" role="alert" aria-live="polite">{error}</div>
  {/if}

  {#if result}
    <div class="success-box" role="status" aria-live="polite">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <polyline points="20 6 9 17 4 12"/>
      </svg>
      PDF başarıyla yüklendi ({result.page_count} sayfa)
    </div>
    {#if result.warning}
      <div class="warning-box" role="status">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
        </svg>
        {result.warning}
      </div>
    {/if}
    {#if result.suggested_query && onsearchfrompdf}
      <div class="search-from-pdf">
        <p class="preview-label">Önerilen arama sorgusu:</p>
        <p class="preview-text">{result.suggested_query}</p>
        <button type="button" class="search-btn" onclick={searchWithExtractedText}>
          Bu Metinle Ara
        </button>
      </div>
    {/if}
  {/if}
</div>

<style>
  .upload-panel {
    padding: 32px;
    max-width: 600px;
  }

  .upload-header {
    margin-bottom: 24px;
  }

  h1 {
    font-size: 22px;
    font-weight: 700;
    margin-bottom: 6px;
  }

  .subtitle {
    color: var(--text-secondary);
    font-size: 14px;
  }

  .drop-zone {
    border: 2px dashed var(--border);
    border-radius: var(--radius-lg);
    padding: 48px 24px;
    text-align: center;
    cursor: pointer;
    transition: all 0.2s;
    color: var(--text-muted);
  }

  .drop-zone:hover, .drop-zone.active {
    border-color: var(--accent);
    background: var(--accent-muted);
    color: var(--accent);
  }

  .drop-text {
    font-size: 15px;
    margin-top: 12px;
    color: inherit;
  }

  .drop-hint {
    font-size: 12px;
    margin-top: 4px;
    color: var(--text-muted);
  }

  .uploading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
    color: var(--text-secondary);
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
    padding: 12px 16px;
    border-radius: var(--radius);
    font-size: 13px;
    margin-top: 12px;
  }

  .success-box {
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(34, 197, 94, 0.1);
    color: #4ade80;
    padding: 12px 16px;
    border-radius: var(--radius);
    font-size: 13px;
    margin-top: 12px;
  }

  .warning-box {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    background: rgba(234, 179, 8, 0.1);
    color: #facc15;
    padding: 12px 16px;
    border-radius: var(--radius);
    font-size: 13px;
    margin-top: 8px;
  }

  .warning-box svg {
    flex-shrink: 0;
    margin-top: 1px;
  }

  .search-from-pdf {
    margin-top: 12px;
    padding: 14px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .preview-label {
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 6px;
  }

  .preview-text {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.5;
    margin-bottom: 10px;
    max-height: 80px;
    overflow: hidden;
  }

  .search-btn {
    background: var(--accent);
    color: white;
    padding: 8px 16px;
    border-radius: var(--radius);
    font-size: 13px;
    font-weight: 600;
  }

  .search-btn:hover {
    background: var(--accent-hover);
  }
</style>