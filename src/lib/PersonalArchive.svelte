<script>
  import { invoke } from '@tauri-apps/api/core';
  import { formatInvokeError } from './formatters.js';
  import { open } from '@tauri-apps/plugin-dialog';

  let { onsettingschange } = $props();

  let archiveEnabled = $state(false);
  let showIntro = $state(false);
  let decisions = $state([]);
  let loading = $state(true);
  let uploading = $state(false);
  let error = $state('');
  let loadError = $state('');
  let consentGiven = $state(false);
  let showConsent = $state(false);
  let pendingFiles = $state([]);
  let uploadProgress = $state('');

  $effect(() => {
    invoke('get_settings')
      .then((s) => {
        archiveEnabled = s.personal_archive_enabled;
        onsettingschange?.(archiveEnabled);
        if (archiveEnabled) {
          try {
            showIntro = !localStorage.getItem('kukla_archive_onboarding_seen');
          } catch (_) {
            showIntro = true;
          }
          loadDecisions();
        }
      })
      .catch(() => {});
  });

  function dismissIntro() {
    showIntro = false;
    try {
      localStorage.setItem('kukla_archive_onboarding_seen', 'true');
    } catch (_) {}
  }

  async function loadDecisions() {
    loading = true;
    loadError = '';
    try {
      decisions = await invoke('list_personal_decisions');
    } catch (e) {
      decisions = [];
      loadError = typeof e === 'string' ? e : 'Karar listesi yüklenemedi.';
    } finally {
      loading = false;
    }
  }

  async function handlePickFile() {
    const selected = await open({
      multiple: true,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (!selected) return;

    const files = Array.isArray(selected) ? selected : [selected];
    if (files.length === 0) return;

    pendingFiles = files;
    showConsent = true;
  }

  async function handleConsent(approved) {
    if (!approved) {
      showConsent = false;
      pendingFiles = [];
      return;
    }
    consentGiven = true;
    await uploadFiles();
    showConsent = false;
    consentGiven = false;
  }

  async function uploadFiles() {
    uploading = true;
    error = '';
    uploadProgress = '';

    try {
      if (pendingFiles.length === 1) {
        uploadProgress = '1/1 yükleniyor...';
        await invoke('add_personal_decision', {
          filePath: pendingFiles[0],
          consentGiven: true,
        });
      } else {
        uploadProgress = `${pendingFiles.length} dosya yükleniyor...`;
        const result = await invoke('add_personal_decisions_bulk', {
          filePaths: pendingFiles,
          consentGiven: true,
        });
        if (result.failed > 0) {
          error = `${result.succeeded} başarılı, ${result.failed} başarısız. ${result.errors.join(' ')}`;
        } else {
          uploadProgress = `${result.succeeded} karar eklendi.`;
        }
      }
      pendingFiles = [];
      await loadDecisions();
    } catch (err) {
      error = formatInvokeError(err) || 'Karar yüklenemedi.';
    } finally {
      uploading = false;
      if (!error) {
        setTimeout(() => { uploadProgress = ''; }, 3000);
      }
    }
  }

  async function handleDelete(decisionId) {
    try {
      await invoke('delete_personal_decision', { decisionId });
      await loadDecisions();
    } catch (err) {
      error = formatInvokeError(err) || 'Karar silinemedi.';
    }
  }
</script>

<div class="archive-panel">
  <div class="archive-header">
    <h1>Kişisel Karar Arşivi</h1>
    <p class="subtitle">Kendi PDF kararlarınızı sisteme ekleyin ve aramada kullanın</p>
  </div>

  {#if !archiveEnabled}
    <div class="disabled-state">
      <p>Bu özellik varsayılan olarak kapalıdır (KVKK). Kullanmak için <strong>Ayarlar → Gizlilik ve Arşiv</strong> bölümünden etkinleştirin.</p>
      <p class="kvkk-note">Yalnızca kendi mesleki kullanımınız içindir. Yüklediğiniz dosyaların yasal kullanım hakkına sahip olduğunuzu onaylamanız gerekir.</p>
    </div>
  {:else}
  <div class="kvkk-banner" role="note">
    Bu özellik sadece kendi mesleki kullanımınız içindir. Yüklediğiniz dosyalar hiçbir şekilde dışa aktarılmaz veya paylaşılmaz.
  </div>

  {#if showIntro}
    <div class="intro-box">
      <h2>Arşiv Hakkında</h2>
      <p>Kendi PDF kararlarınızı yükleyerek kişisel aramanıza dahil edebilirsiniz. Her yüklemede yasal kullanım onayı istenir; sildiğinizde embedding'ler de kalıcı olarak kaldırılır.</p>
      <button type="button" class="intro-btn" onclick={dismissIntro}>Anladım</button>
    </div>
  {/if}

  <button class="add-btn" onclick={handlePickFile} disabled={uploading}>
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
    </svg>
    {uploading ? 'Yükleniyor...' : 'Karar Ekle (Tek veya Çoklu)'}
  </button>

  {#if uploadProgress && !error}
    <p class="progress-msg">{uploadProgress}</p>
  {/if}

  {#if showConsent}
    <div class="consent-dialog">
      <p>
        {pendingFiles.length === 1
          ? 'Bu özellik sadece kendi mesleki kullanımınız içindir. Yüklediğiniz dosyanın yasal kullanım hakkına sahip olduğunuzu onaylıyorsunuz.'
          : `${pendingFiles.length} dosya seçildi. Bu özellik sadece kendi mesleki kullanımınız içindir. Yüklediğiniz dosyaların yasal kullanım hakkına sahip olduğunuzu onaylıyorsunuz.`}
      </p>
      <div class="consent-actions">
        <button class="btn-approve" onclick={() => handleConsent(true)}>Onaylıyorum</button>
        <button class="btn-reject" onclick={() => handleConsent(false)}>İptal</button>
      </div>
    </div>
  {/if}

  {#if error}
    <div class="error-box">{error}</div>
  {/if}

  {#if loading}
    <p class="muted">Yükleniyor...</p>
  {:else if loadError}
    <div class="error-box" role="alert">{loadError}</div>
    <button class="retry-link" onclick={loadDecisions}>Tekrar Dene</button>
  {:else if decisions.length === 0}
    <div class="empty-state">
      <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="1.5">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/>
      </svg>
      <p>Henüz karar eklenmemiş. PDF yüklemek için yukarıdaki butonu kullanın.</p>
    </div>
  {:else}
    <div class="decision-list">
      {#each decisions as decision}
        <div class="decision-card">
          <div class="card-info">
            <p class="card-preview">{decision.text_preview}</p>
            <p class="card-id">Karar No: {decision.decision_id ?? decision.id}</p>
          </div>
          <button class="delete-btn" onclick={() => handleDelete(decision.id)} aria-label="Kararı sil">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
            </svg>
          </button>
        </div>
      {/each}
    </div>
  {/if}
  {/if}
</div>

<style>
  .archive-panel {
    padding: 32px;
    max-width: 600px;
    overflow-y: auto;
    height: 100%;
  }

  .archive-header {
    margin-bottom: 24px;
  }

  .disabled-state {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 20px;
    font-size: 14px;
    color: var(--text-secondary);
    line-height: 1.6;
  }

  .kvkk-note {
    margin-top: 10px;
    font-size: 13px;
    color: var(--text-muted);
  }

  .kvkk-banner {
    background: rgba(234, 179, 8, 0.1);
    border: 1px solid rgba(234, 179, 8, 0.25);
    color: #facc15;
    padding: 10px 14px;
    border-radius: var(--radius);
    font-size: 12px;
    margin-bottom: 16px;
    line-height: 1.5;
  }

  .intro-box {
    background: var(--accent-muted);
    border: 1px solid rgba(99, 102, 241, 0.25);
    border-radius: var(--radius);
    padding: 16px;
    margin-bottom: 16px;
  }

  .intro-box h2 {
    font-size: 14px;
    margin-bottom: 8px;
  }

  .intro-box p {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.5;
    margin-bottom: 10px;
  }

  .intro-btn {
    background: var(--accent);
    color: white;
    padding: 6px 14px;
    border-radius: var(--radius);
    font-size: 12px;
    font-weight: 600;
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

  .add-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--accent);
    color: white;
    padding: 10px 20px;
    border-radius: var(--radius);
    font-weight: 600;
    transition: background 0.15s;
    margin-bottom: 20px;
  }

  .add-btn:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .add-btn:disabled {
    opacity: 0.5;
  }

  .progress-msg {
    font-size: 13px;
    color: #4ade80;
    margin: -8px 0 16px;
  }

  .consent-dialog {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 20px;
    margin-bottom: 20px;
  }

  .consent-dialog p {
    font-size: 14px;
    color: var(--text-primary);
    margin-bottom: 16px;
    line-height: 1.5;
  }

  .consent-actions {
    display: flex;
    gap: 10px;
  }

  .btn-approve {
    background: var(--accent);
    color: white;
    padding: 8px 16px;
    border-radius: var(--radius);
    font-weight: 600;
  }

  .btn-reject {
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    padding: 8px 16px;
    border-radius: var(--radius);
  }

  .error-box {
    background: rgba(239, 68, 68, 0.1);
    color: var(--danger);
    padding: 10px 14px;
    border-radius: var(--radius);
    font-size: 13px;
    margin-bottom: 16px;
  }

  .retry-link {
    color: var(--accent);
    font-size: 13px;
    margin-bottom: 16px;
  }

  .retry-link:hover {
    text-decoration: underline;
  }

  .muted {
    color: var(--text-muted);
    font-size: 14px;
    padding: 20px 0;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 48px 24px;
    color: var(--text-muted);
    font-size: 14px;
    text-align: center;
  }

  .decision-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .decision-card {
    display: flex;
    align-items: center;
    gap: 12px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 12px 16px;
  }

  .card-info {
    flex: 1;
    min-width: 0;
  }

  .card-preview {
    font-size: 13px;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .card-id {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 2px;
  }

  .delete-btn {
    color: var(--text-muted);
    padding: 6px;
    border-radius: var(--radius);
    transition: all 0.15s;
    flex-shrink: 0;
  }

  .delete-btn:hover {
    background: rgba(239, 68, 68, 0.1);
    color: var(--danger);
  }
</style>
