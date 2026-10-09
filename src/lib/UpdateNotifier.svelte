<script>
  import { check } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { formatInvokeError } from './formatters.js';

  let updateInfo = $state(null);
  let checking = $state(true);
  let installing = $state(false);
  let error = $state('');
  let dismissed = $state(false);

  $effect(() => {
    if (import.meta.env.DEV) {
      checking = false;
      return;
    }
    check()
      .then((update) => {
        if (update) updateInfo = update;
      })
      .catch(() => {
        // Güncelleme sunucusu yoksa sessizce devam et (dev/release yapılandırması).
      })
      .finally(() => {
        checking = false;
      });
  });

  async function installUpdate() {
    if (!updateInfo || installing) return;
    installing = true;
    error = '';
    try {
      await updateInfo.downloadAndInstall();
      await relaunch();
    } catch (err) {
      error = formatInvokeError(err) || 'Güncelleme kurulamadı.';
      installing = false;
    }
  }
</script>

{#if !checking && updateInfo && !dismissed}
  <div class="update-banner" role="status">
    <span>Yeni sürüm mevcut: <strong>v{updateInfo.version}</strong></span>
    <div class="actions">
      <button class="install-btn" onclick={installUpdate} disabled={installing}>
        {installing ? 'İndiriliyor...' : 'Güncelle'}
      </button>
      <button class="dismiss-btn" onclick={() => dismissed = true} disabled={installing}>Sonra</button>
    </div>
    {#if error}
      <p class="error">{error}</p>
    {/if}
  </div>
{/if}

<style>
  .update-banner {
    background: rgba(99, 102, 241, 0.12);
    border-bottom: 1px solid rgba(99, 102, 241, 0.25);
    padding: 10px 20px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
    font-size: 13px;
    color: var(--text-primary);
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  .install-btn {
    background: var(--accent);
    color: white;
    padding: 6px 14px;
    border-radius: var(--radius);
    font-size: 12px;
    font-weight: 600;
  }

  .dismiss-btn {
    background: transparent;
    color: var(--text-muted);
    padding: 6px 10px;
    font-size: 12px;
  }

  .error {
    width: 100%;
    color: var(--danger);
    font-size: 12px;
    margin: 4px 0 0;
  }
</style>