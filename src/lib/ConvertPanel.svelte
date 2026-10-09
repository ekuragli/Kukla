<script>
  import { invoke } from '@tauri-apps/api/core';
  import { formatInvokeError } from './formatters.js';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { revealItemInDir } from '@tauri-apps/plugin-opener';

  const TARGETS = [
    { value: 'txt', label: 'TXT', hint: 'Düz metin' },
    { value: 'docx', label: 'DOCX', hint: 'Word belgesi' },
    { value: 'pdf', label: 'PDF', hint: 'Yazdırılabilir belge' },
  ];

  const FORMAT_LABELS = { pdf: 'PDF', docx: 'DOCX', txt: 'TXT/MD', udf: 'UDF (UYAP)', zip: 'UDF/ZIP' };

  let filePath = $state('');
  let targetFormat = $state('txt');
  let outputPath = $state('');
  let converting = $state(false);
  let result = $state(null);
  let error = $state('');

  function sourceLabel(path) {
    const ext = (path.split('.').pop() || '').toLowerCase();
    return FORMAT_LABELS[ext] || ext.toUpperCase();
  }

  async function pickSourceFile() {
    const selected = await open({
      multiple: false,
      filters: [
        { name: 'Belgeler', extensions: ['pdf', 'docx', 'txt', 'md', 'udf', 'zip'] },
        { name: 'UYAP Doküman', extensions: ['udf', 'zip'] },
        { name: 'Word', extensions: ['docx'] },
        { name: 'PDF', extensions: ['pdf'] },
        { name: 'Metin', extensions: ['txt', 'md'] },
      ],
    });
    if (selected) {
      filePath = selected;
      result = null;
      error = '';
    }
  }

  async function pickOutputFile() {
    const ext = targetFormat;
    const suggested = `${(filePath.split(/[/\\]/).pop() || 'belge').replace(/\.[^.]+$/, '')}.converted.${ext}`;
    const selected = await save({
      defaultPath: suggested,
      filters: [{ name: TARGETS.find((t) => t.value === ext)?.label || ext, extensions: [ext] }],
    });
    if (selected !== null && selected !== undefined) outputPath = selected;
  }

  async function convert() {
    if (!filePath) return;
    converting = true;
    error = '';
    result = null;
    try {
      result = await invoke('convert_file', {
        filePath,
        targetFormat,
        outputPath: outputPath || null,
      });
    } catch (err) {
      error = formatInvokeError(err) || 'Dönüştürme sırasında hata oluştu.';
    } finally {
      converting = false;
    }
  }

  async function revealOutput() {
    if (!result?.output_path) return;
    try {
      await revealItemInDir(result.output_path);
    } catch (err) {
      error = formatInvokeError(err) || 'Klasör açılamadı.';
    }
  }
</script>

<div class="convert-panel">
  <div class="convert-header">
    <h1>Araçlar</h1>
    <p class="subtitle">PDF, DOCX, TXT ve UYAP (<code>.udf</code>) dosyalarını birbirine dönüştürün</p>
  </div>

  <button type="button" class="source-box" onclick={pickSourceFile}>
    <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="12" y1="18" x2="12" y2="12"/><line x1="9" y1="15" x2="15" y2="15"/>
    </svg>
    <span class="source-text">
      {#if filePath}
        <strong>{filePath.split(/[/\\]/).pop()}</strong>
        <span class="source-path">{filePath}</span>
      {:else}
        Dosya seçmek için tıklayın
        <span class="source-hint">PDF · DOCX · TXT · UDF (UYAP)</span>
      {/if}
    </span>
    <span class="change-btn">Değiştir</span>
  </button>

  <div class="target-row">
    <span class="row-label">Hedef biçim</span>
    <div class="target-options" role="radiogroup" aria-label="Hedef biçim">
      {#each TARGETS as target (target.value)}
        <button
          type="button"
          class="target-btn"
          class:active={targetFormat === target.value}
          aria-pressed={targetFormat === target.value}
          onclick={() => { targetFormat = target.value; outputPath = ''; }}
        >
          <strong>{target.label}</strong>
          <span>{target.hint}</span>
        </button>
      {/each}
    </div>
  </div>

  <div class="output-row">
    <span class="row-label">Çıktı</span>
    <div class="output-value">
      {#if outputPath}
        <code>{outputPath}</code>
        <button type="button" class="link-btn" onclick={() => outputPath = ''}>Masaüstüne kaydet</button>
      {:else}
        <span class="output-hint">Varsayılan: Masaüstü</span>
        <button type="button" class="link-btn" onclick={pickOutputFile}>Farklı kaydet…</button>
      {/if}
    </div>
  </div>

  <button type="button" class="convert-btn" disabled={!filePath || converting} onclick={convert}>
    {#if converting}
      <span class="spinner" role="status" aria-label="Dönüştürülüyor"></span>
      Dönüştürülüyor…
    {:else}
      Dönüştür
    {/if}
  </button>

  {#if error}
    <div class="error-box" role="alert" aria-live="polite">{error}</div>
  {/if}

  {#if result}
    <div class="success-box" role="status" aria-live="polite">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <polyline points="20 6 9 17 4 12"/>
      </svg>
      <span>
        {result.source_format.toUpperCase()} → {result.target_format.toUpperCase()} tamamlandı
        ({result.char_count.toLocaleString('tr-TR')} karakter)
      </span>
      <button type="button" class="link-btn" onclick={revealOutput}>Klasörde göster</button>
    </div>
    {#if result.warning}
      <div class="warning-box" role="status">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
        </svg>
        {result.warning}
      </div>
    {/if}
    <p class="output-path">{result.output_path}</p>
  {/if}

  <div class="notes">
    <p><strong>UDF (UYAP):</strong> .udf dosyası bir ZIP arşividir; metin <code>content.xml</code> içinden okunur. E-imza (<code>sign.sgn</code>) PDF'e taşınmaz.</p>
    <p><strong>PDF:</strong> Görüntü tabanlı (taranmış) PDF'lerden metin çıkarılamaz. DOCX ve UDF çevirileri metni olduğu gibi aktarır; görseller taşınmaz.</p>
  </div>
</div>

<style>
  .convert-panel {
    padding: 32px;
    max-width: 640px;
  }

  .convert-header {
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

  .subtitle code,
  .notes code {
    font-size: 12px;
    background: var(--bg-tertiary);
    padding: 1px 5px;
    border-radius: 4px;
  }

  .source-box {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    text-align: left;
    border: 2px dashed var(--border);
    border-radius: var(--radius-lg);
    padding: 18px 20px;
    cursor: pointer;
    color: var(--text-secondary);
    background: transparent;
    transition: all 0.2s;
  }

  .source-box:hover {
    border-color: var(--accent);
    background: var(--accent-muted);
    color: var(--accent);
  }

  .source-text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
    min-width: 0;
    font-size: 14px;
  }

  .source-text strong {
    color: var(--text-primary);
    font-size: 14px;
  }

  .source-box:hover .source-text strong {
    color: var(--accent);
  }

  .source-path,
  .source-hint {
    font-size: 12px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .change-btn {
    font-size: 12px;
    font-weight: 600;
    color: var(--accent);
    flex-shrink: 0;
  }

  .target-row,
  .output-row {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-top: 18px;
  }

  .row-label {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
    width: 84px;
    flex-shrink: 0;
  }

  .target-options {
    display: flex;
    gap: 8px;
    flex: 1;
  }

  .target-btn {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.15s;
    text-align: left;
  }

  .target-btn strong {
    font-size: 13px;
    color: var(--text-primary);
  }

  .target-btn span {
    font-size: 11px;
    color: var(--text-muted);
  }

  .target-btn:hover {
    border-color: var(--accent);
  }

  .target-btn.active {
    border-color: var(--accent);
    background: var(--accent-muted);
  }

  .target-btn.active strong {
    color: var(--accent);
  }

  .output-value {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
    min-width: 0;
    font-size: 13px;
  }

  .output-value code {
    font-size: 12px;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .output-hint {
    color: var(--text-muted);
    font-size: 13px;
  }

  .link-btn {
    background: none;
    border: none;
    color: var(--accent);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
  }

  .link-btn:hover {
    text-decoration: underline;
  }

  .convert-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    margin-top: 22px;
    padding: 12px;
    background: var(--accent);
    color: white;
    border: none;
    border-radius: var(--radius);
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
  }

  .convert-btn:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .convert-btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(255, 255, 255, 0.35);
    border-top-color: white;
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
    margin-top: 14px;
  }

  .success-box {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    background: rgba(34, 197, 94, 0.1);
    color: #4ade80;
    padding: 12px 16px;
    border-radius: var(--radius);
    font-size: 13px;
    margin-top: 14px;
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

  .output-path {
    font-size: 11px;
    color: var(--text-muted);
    word-break: break-all;
    margin-top: 8px;
  }

  .notes {
    margin-top: 24px;
    border-top: 1px solid var(--border);
    padding-top: 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .notes p {
    font-size: 12px;
    color: var(--text-muted);
    line-height: 1.55;
  }

  .notes strong {
    color: var(--text-secondary);
  }
</style>
