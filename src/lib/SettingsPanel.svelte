<script>
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import {
    formatInvokeError,
    validatePasswordRules,
    computeStrength,
    strengthLabel,
    strengthColor,
  } from './formatters.js';
  import { check } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { revealItemInDir } from '@tauri-apps/plugin-opener';

  let currentPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let message = $state('');
  let isError = $state(false);
  let changing = $state(false);
  let settings = $state({
    personal_archive_enabled: false,
    auto_lock_minutes: 15,
  });
  let settingsMessage = $state('');
  let settingsError = $state(false);
  let savingSettings = $state(false);

  let aiStatus = $state(null);
  let aiLoading = $state(false);
  let aiMessage = $state('');
  let webStatus = $state(null);
  let webSearchKeyInput = $state('');
  let savingWebKey = $state(false);
  let webKeyMessage = $state('');
  let webKeyError = $state(false);
  let cloudStatus = $state(null);
  let cloudProvider = $state('');
  let cloudModel = $state('');
  let cloudBaseUrl = $state('');
  let cloudKeyInput = $state('');
  let savingCloud = $state(false);
  let cloudMessage = $state('');
  let cloudError = $state(false);
  let aiError = $state(false);

  let updateChecking = $state(false);
  let updateInstalling = $state(false);
  let updateMessage = $state('');
  let updateError = $state(false);
  let updateAvailable = $state(null);
  let downloadProgress = $state(0);
  let downloadTotal = $state(0);

  let appPaths = $state(null);

  onMount(() => {
    loadSettings();
    loadAiStatus();
    loadWebStatus();
    loadCloudStatus();
    invoke('get_app_paths')
      .then((paths) => {
        appPaths = paths;
      })
      .catch(() => {});
  });

  async function loadSettings() {
    try {
      const loaded = await invoke('get_settings');
      settings = { ...settings, ...loaded };
    } catch (_) {
      // ayarlar okunamazsa mevcut varsayılanlarla devam edilir
    }
  }

  async function loadAiStatus() {
    try {
      aiStatus = await invoke('get_ai_status');
    } catch (err) {
      aiError = true;
      aiMessage = formatInvokeError(err) || 'YZ durumu okunamadı.';
    }
  }

  // Sunucuya erişilemiyorsa uygulama onu gizli alt süreç olarak başlatır.
  async function handleStartServer() {
    aiLoading = true;
    aiMessage = '';
    aiError = false;
    try {
      const url = await invoke('start_opencode_server');
      aiMessage = `opencode sunucusu hazır: ${url}`;
    } catch (err) {
      aiError = true;
      aiMessage = formatInvokeError(err) || 'Sunucu başlatılamadı.';
    } finally {
      aiLoading = false;
      await loadAiStatus();
    }
  }

  // Eski sürümde kimliksiz indekslenen çevrimiçi kayıtları temizler.
  async function handleClearOnlineIndex() {
    if (!window.confirm('Çevrimiçi kararların indeksi temizlenecek. Kararları yeniden açtığınızda güncel biçimde indekslenir. Devam edilsin mi?')) {
      return;
    }
    aiLoading = true;
    aiMessage = '';
    aiError = false;
    try {
      const removed = await invoke('clear_online_index_command');
      aiMessage = removed > 0
        ? `${removed} eski kayıt temizlendi. Kararları yeniden açarak indeksleyebilirsiniz.`
        : 'Temizlenecek eski kayıt bulunamadı.';
    } catch (err) {
      aiError = true;
      aiMessage = formatInvokeError(err) || 'İndeks temizlenemedi.';
    } finally {
      aiLoading = false;
    }
  }

  async function loadWebStatus() {
    try {
      webStatus = await invoke('get_web_search_status');
    } catch (err) {
      webStatus = null;
    }
  }

  // Tavily anahtarını kaydeder veya siler (clear=true).
  async function handleSaveWebKey(clear = false) {
    savingWebKey = true;
    webKeyMessage = '';
    webKeyError = false;
    const key = clear ? null : webSearchKeyInput.trim();
    if (!clear && !key) {
      savingWebKey = false;
      webKeyError = true;
      webKeyMessage = 'Anahtar boş olamaz.';
      return;
    }
    try {
      webStatus = await invoke('set_web_search_key', { key });
      webSearchKeyInput = '';
      webKeyMessage = clear ? 'Anahtar silindi.' : 'Anahtar kaydedildi.';
    } catch (err) {
      webKeyError = true;
      webKeyMessage = formatInvokeError(err) || 'Anahtar kaydedilemedi.';
    } finally {
      savingWebKey = false;
    }
  }

  async function loadCloudStatus() {
    try {
      cloudStatus = await invoke('get_cloud_status');
      cloudProvider = cloudStatus.provider ?? '';
      cloudModel = cloudStatus.model ?? '';
      cloudBaseUrl = cloudStatus.base_url ?? '';
    } catch (err) {
      cloudStatus = null;
    }
  }

  // Sağlayıcı/model/adres değişimini hemen kaydeder.
  async function handleCloudConfigChange() {
    savingCloud = true;
    cloudMessage = '';
    cloudError = false;
    try {
      cloudStatus = await invoke('set_cloud_config', {
        provider: cloudProvider || null,
        model: cloudModel || null,
        baseUrl: cloudBaseUrl || null,
      });
      cloudMessage = 'Bulut yapılandırması kaydedildi.';
    } catch (err) {
      cloudError = true;
      cloudMessage = formatInvokeError(err) || 'Yapılandırma kaydedilemedi.';
    } finally {
      savingCloud = false;
    }
  }

  async function handleSaveCloudKey(clear = false) {
    savingCloud = true;
    cloudMessage = '';
    cloudError = false;
    const key = clear ? null : cloudKeyInput.trim();
    if (!clear && !key) {
      savingCloud = false;
      cloudError = true;
      cloudMessage = 'Anahtar boş olamaz.';
      return;
    }
    try {
      await invoke('set_cloud_key', { key });
      cloudKeyInput = '';
      await loadCloudStatus();
      cloudMessage = clear ? 'Anahtar silindi.' : 'Anahtar kaydedildi.';
    } catch (err) {
      cloudError = true;
      cloudMessage = formatInvokeError(err) || 'Anahtar kaydedilemedi.';
    } finally {
      savingCloud = false;
    }
  }

  async function loadCloudModels() {
    await loadCloudStatus();
    if ((cloudStatus?.models?.length ?? 0) === 0) {
      cloudError = true;
      cloudMessage = cloudStatus?.has_key
        ? 'Model listesi alınamadı; model kimliğini elle girin.'
        : 'Önce API anahtarını kaydedin.';
    } else {
      cloudError = false;
      cloudMessage = `${cloudStatus.models.length} model bulundu.`;
    }
  }

  async function handlePrepareEmbedding() {
    aiLoading = true;
    aiMessage = '';
    aiError = false;
    try {
      const status = await invoke('prepare_embedding');
      aiStatus = { ...(aiStatus || {}), embedding: status };
      aiMessage = 'Embedding modeli hazır.';
    } catch (err) {
      aiError = true;
      aiMessage = formatInvokeError(err) || 'Embedding modeli hazırlanamadı.';
    } finally {
      aiLoading = false;
    }
  }

  async function handleModelChange(e) {
    const value = e.currentTarget.value.trim();
    savingSettings = true;
    settingsMessage = '';
    settingsError = false;
    try {
      const updated = { ...settings, ai_model: value ? value : null };
      await invoke('update_settings', { settings: updated });
      settings = updated;
      aiStatus = aiStatus ? { ...aiStatus, selected_model: updated.ai_model } : aiStatus;
      settingsMessage = value
        ? `Model seçildi: ${value}`
        : 'opencode varsayılanı kullanılacak.';
    } catch (err) {
      settingsError = true;
      settingsMessage = formatInvokeError(err) || 'Model seçimi kaydedilemedi.';
      e.currentTarget.value = settings.ai_model ?? '';
    } finally {
      savingSettings = false;
    }
  }

  function embeddingStateLabel() {
    const state = aiStatus?.embedding?.state;
    switch (state) {
      case 'ready':
        return 'Hazır';
      case 'loading':
        return 'İndiriliyor / yükleniyor...';
      case 'not_loaded':
        return 'İndirildi, yüklenmedi';
      case 'missing':
        return 'İndirilmedi';
      case 'error':
        return 'Hata';
      default:
        return 'Bilinmiyor';
    }
  }

  function embeddingStateClass() {
    const state = aiStatus?.embedding?.state;
    if (state === 'ready') return 'on';
    if (state === 'error') return 'err';
    return 'off';
  }

  function opencodeModelLabel() {
    const model = aiStatus?.opencode?.model;
    const agent = aiStatus?.opencode?.agent;
    if (!model) return 'Model seçilmedi';
    return agent ? `${model} · ${agent}` : model;
  }

  async function openDataFolder() {
    if (!appPaths?.dataDir) return;
    try {
      await revealItemInDir(appPaths.dataDir);
    } catch (_) {}
  }

  async function handleChangePassword(e) {
    e.preventDefault();
    message = '';
    isError = false;

    if (newPassword.trim().length === 0) {
      message = 'Yeni şifre boş olamaz.';
      isError = true;
      return;
    }
    const ruleErrors = validatePasswordRules(newPassword);
    if (ruleErrors.length > 0) {
      message = ruleErrors[0];
      isError = true;
      return;
    }
    if (newPassword !== confirmPassword) {
      message = 'Yeni şifreler eşleşmiyor.';
      isError = true;
      return;
    }

    changing = true;
    try {
      await invoke('auth_set_password', {
        currentPassword,
        newPassword,
      });
      try {
        await invoke('log_audit_event', { eventType: 'settings_change', details: 'password_changed' });
      } catch (_) {}
      message = 'Şifre başarıyla değiştirildi.';
      isError = false;
      currentPassword = '';
      newPassword = '';
      confirmPassword = '';
    } catch (err) {
      message = formatInvokeError(err) || 'Şifre değiştirilemedi.';
      isError = true;
    } finally {
      changing = false;
    }
  }

  async function handleCheckUpdate() {
    updateChecking = true;
    updateMessage = '';
    updateError = false;
    updateAvailable = null;
    downloadProgress = 0;
    downloadTotal = 0;

    try {
      const update = await check();
      if (update) {
        updateAvailable = update;
        updateMessage = `Yeni sürüm mevcut: v${update.version}`;
      } else {
        updateMessage = 'Uygulama güncel.';
      }
    } catch (err) {
      updateError = true;
      updateMessage = formatInvokeError(err) || 'Güncelleme kontrol edilemedi.';
    } finally {
      updateChecking = false;
    }
  }

  async function handleInstallUpdate() {
    if (!updateAvailable) return;

    updateInstalling = true;
    updateMessage = 'Güncelleme indiriliyor...';
    updateError = false;
    downloadProgress = 0;
    downloadTotal = 0;

    try {
      await updateAvailable.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            downloadTotal = event.data.contentLength ?? 0;
            break;
          case 'Progress':
            downloadProgress += event.data.chunkLength;
            break;
          case 'Finished':
            updateMessage = 'Güncelleme kuruluyor, uygulama yeniden başlatılacak...';
            break;
        }
      });
      await relaunch();
    } catch (err) {
      updateError = true;
      updateMessage = formatInvokeError(err) || 'Güncelleme kurulamadı.';
      updateInstalling = false;
    }
  }
</script>

<div class="settings-panel">
  <div class="settings-header">
    <h1>Ayarlar</h1>
    <p class="subtitle">Uygulama tercihleri ve güvenlik ayarları</p>
  </div>

  <div class="settings-section">
    <h2>Şifre Değiştir</h2>
    <form onsubmit={handleChangePassword}>
      <div class="field">
        <label for="current">Mevcut Şifre</label>
        <input
          id="current"
          type="password"
          bind:value={currentPassword}
          placeholder="Mevcut şifrenizi girin"
        />
      </div>
      <div class="field">
        <label for="new">Yeni Şifre</label>
        <input
          id="new"
          type="password"
          bind:value={newPassword}
          placeholder="En az 12 karakter"
        />
      </div>
      {#if newPassword.length > 0}
        {@const score = computeStrength(newPassword)}
        <div class="strength-meter">
          <div class="strength-bar" style="width: {score}%; background: {strengthColor(score)};"></div>
          <span class="strength-label" style="color: {strengthColor(score)};">
            {strengthLabel(score)} ({score}%)
          </span>
        </div>
      {/if}
      <div class="field">
        <label for="confirm">Yeni Şifre (Tekrar)</label>
        <input
          id="confirm"
          type="password"
          bind:value={confirmPassword}
          placeholder="Yeni şifrenizi tekrar girin"
        />
      </div>
      {#if message}
        <div class="message" class:error={isError} class:success={!isError}>
          {message}
        </div>
      {/if}
      <button type="submit" disabled={changing || !currentPassword || !newPassword || !confirmPassword}>
        {changing ? 'Değiştiriliyor...' : 'Şifreyi Değiştir'}
      </button>
    </form>
  </div>

  <div class="settings-section">
    <h2>Oturum Güvenliği</h2>
    <div class="field">
      <label for="auto-lock">Otomatik Kilit (dakika)</label>
      <select
        id="auto-lock"
        value={settings.auto_lock_minutes}
        disabled={savingSettings}
        onchange={async (e) => {
          const minutes = parseInt(e.currentTarget.value, 10);
          savingSettings = true;
          settingsMessage = '';
          settingsError = false;
          try {
            const updated = { ...settings, auto_lock_minutes: minutes };
            await invoke('update_settings', { settings: updated });
            settings = updated;
            settingsMessage = minutes === 0
              ? 'Otomatik kilit devre dışı.'
              : `Otomatik kilit ${minutes} dakikaya ayarlandı.`;
          } catch (err) {
            settingsError = true;
            settingsMessage = formatInvokeError(err) || 'Ayar güncellenemedi.';
          } finally {
            savingSettings = false;
          }
        }}
      >
        <option value={0}>Kapalı</option>
        <option value={5}>5 dakika</option>
        <option value={15}>15 dakika</option>
        <option value={30}>30 dakika</option>
        <option value={60}>60 dakika</option>
      </select>
      <p class="field-hint">Hareketsizlik süresi sonunda uygulama otomatik kilitlenir.</p>
    </div>
  </div>

  <div class="settings-section">
    <h2>Veri Konumu</h2>
    <p class="field-hint section-hint">Tüm veriler yerel olarak şifreli saklanır. Denetim logları oturum açıldıktan sonra şifrelenir.</p>
    {#if appPaths}
      <div class="path-list">
        <div class="path-row">
          <span class="path-label">Veri klasörü</span>
          <code class="path-value">{appPaths.dataDir}</code>
        </div>
        <div class="path-row">
          <span class="path-label">Denetim logu</span>
          <code class="path-value">{appPaths.auditLogPath}</code>
        </div>
      </div>
      <button type="button" class="path-open-btn" onclick={openDataFolder}>Veri Klasörünü Aç</button>
    {/if}
  </div>

  <div class="settings-section">
    <h2>Gizlilik ve Arşiv</h2>
    <div class="toggle-row">
      <div>
        <span class="toggle-label">Kişisel Karar Arşivi</span>
        <p class="toggle-desc">Kendi PDF kararlarınızı yükleyip arama yapın (KVKK onayı gerekir).</p>
      </div>
      <label class="switch">
        <input
          type="checkbox"
          checked={settings.personal_archive_enabled}
          onchange={async (e) => {
            const enabled = e.currentTarget.checked;
            savingSettings = true;
            settingsMessage = '';
            settingsError = false;
            try {
              const updated = { ...settings, personal_archive_enabled: enabled };
              await invoke('update_settings', { settings: updated });
              settings = updated;
              settingsMessage = enabled
                ? 'Kişisel arşiv etkinleştirildi.'
                : 'Kişisel arşiv devre dışı bırakıldı.';
            } catch (err) {
              settingsError = true;
              settingsMessage = formatInvokeError(err) || 'Ayar güncellenemedi.';
              e.currentTarget.checked = settings.personal_archive_enabled;
            } finally {
              savingSettings = false;
            }
          }}
          disabled={savingSettings}
        />
        <span class="slider"></span>
      </label>
    </div>
    {#if settingsMessage}
      <div class="message" class:error={settingsError} class:success={!settingsError}>
        {settingsMessage}
      </div>
    {/if}
  </div>

  <div class="settings-section">
    <h2>YZ Altyapısı</h2>
    <p class="field-hint section-hint">
      Özet, dilekçe ve sohbet üretimi <code>opencode</code> üzerinden yapılır; sunucuya
      uygulama kendisi bağlanır, gerekirse gizli olarak başlatır. Modeli
      aşağıdan seçersiniz (seçilmezse sunucudaki ilk model kullanılır). Semantik arama
      yerel embedding modeliyle çalışır.
    </p>

    <div class="ai-status-grid">
        <div class="ai-status-row">
          <span class="ai-status-label">opencode sunucusu</span>
          <span class="status-pill" class:on={aiStatus?.opencode?.healthy} class:err={aiStatus && !aiStatus.opencode.healthy}>
            {aiStatus?.opencode?.healthy ? 'Bağlı' : aiStatus ? 'Bağlı değil' : 'Kontrol ediliyor'}
          </span>
          <span class="ai-status-value">
            {aiStatus?.opencode?.base_url || ''}{#if aiStatus?.opencode?.managed_by_app}
              <span class="ai-managed-note" title="Sunucuyu Kukla başlattı; uygulama kapanınca durdurulur">(uygulama yönetti)</span>
            {/if}
          </span>
        </div>
        <div class="ai-status-row">
          <span class="ai-status-label">Seçili model</span>
          <span class="ai-status-value">{settings.ai_model || 'otomatik (sunucudaki ilk model)'}</span>
        </div>
      <div class="ai-status-row">
        <span class="ai-status-label">Son kullanılan model</span>
        <span class="ai-status-value">{opencodeModelLabel()}</span>
      </div>
      <div class="ai-status-row">
        <span class="ai-status-label">Embedding ({aiStatus?.embedding?.dim || 384} boyut)</span>
        <span
          class="status-pill"
          class:on={embeddingStateClass() === 'on'}
          class:err={embeddingStateClass() === 'err'}
        >
          {embeddingStateLabel()}
        </span>
      </div>
    </div>

    <div class="field">
      <label for="ai-model">Model</label>
      <select
        id="ai-model"
        value={settings.ai_model ?? ''}
        onchange={handleModelChange}
        disabled={savingSettings}
      >
        <option value="">Otomatik (sunucudaki ilk model)</option>
        {#each aiStatus?.models ?? [] as model (model.provider_id + '/' + model.id)}
          <option value="{model.provider_id}/{model.id}">{model.provider_id}/{model.id}</option>
        {/each}
      </select>
      <p class="field-hint">
        Özet ve dilekçe istekleri bu modelle çalışır; liste terminaldeki opencode sunucusundan gelir.
        Seçili model hata verirse uygulama otomatik olarak hızlı bir yedeğe geçer.
        Ölçümlere göre özet için hızlı olanlar: <code>space-bunny-free</code>, <code>big-pickle</code>.
        {#if aiStatus?.opencode?.healthy && (aiStatus?.models?.length ?? 0) === 0}
          Model listesi alınamadı, sunucuyu yeniden kontrol edin.
        {/if}
      </p>
    </div>

    {#if aiStatus && !aiStatus.opencode.healthy}
      <p class="field-hint">
        Terminalde başlatın: <code>opencode serve --port 4096</code>
        {#if aiStatus.opencode.detail}<span class="ai-detail">({aiStatus.opencode.detail})</span>{/if}
      </p>
    {/if}
    {#if aiStatus?.embedding?.path}
      <p class="field-hint">Model dizini: <code>{aiStatus.embedding.path}</code></p>
    {/if}

    <div class="ai-actions">
      <button type="button" onclick={handlePrepareEmbedding} disabled={aiLoading || embeddingStateClass() === 'on'}>
        {aiLoading
          ? 'Model hazırlanıyor...'
          : embeddingStateClass() === 'on'
            ? 'Model Hazır'
            : 'Embedding Modelini İndir'}
      </button>
      <button type="button" onclick={loadAiStatus} disabled={aiLoading}>Durumu Yenile</button>
      <button
        type="button"
        onclick={handleStartServer}
        disabled={aiLoading || aiStatus?.opencode?.healthy}
        title="opencode sunucusunu uygulama yönetiminde başlatır"
      >
        {aiLoading ? 'Başlatılıyor...' : 'Sunucuyu Başlat'}
      </button>
      <button
        type="button"
        onclick={handleClearOnlineIndex}
        disabled={aiLoading}
        title="Eski sürümde indekslenen kimliksiz çevrimiçi kayıtları temizler; kararları yeniden açtığınızda düzgün şekilde indekslenir"
      >
        Çevrimiçi İndeksi Temizle
      </button>
    </div>

    {#if aiStatus?.embedding?.detail}
      <div class="message error">{aiStatus.embedding.detail}</div>
    {/if}
    {#if aiMessage}
      <div class="message" class:error={aiError} class:success={!aiError}>{aiMessage}</div>
    {/if}

    <h3 class="ai-subsection-title">İnternet Araştırması (sohbet için)</h3>
    <p class="field-hint">
      Sohbet asistanının "İnternet" modunda sorularınızı <strong>Tavily</strong> ile
      internette aramasını sağlar. Ücretsiz katman ayda 1.000 arama kredisi sunar;
      anahtar <a href="https://tavily.com" target="_blank" rel="noreferrer">tavily.com</a>
      adresinden alınır ve şifreli veritabanınızda saklanır.
    </p>
    <div class="ai-key-row">
      <label for="web-search-key">Tavily API anahtarı</label>
      <input
        id="web-search-key"
        type="password"
        bind:value={webSearchKeyInput}
        placeholder={webStatus?.has_web_search_key ? `Tanımlı (${webStatus.web_search_key_hint})` : 'tvly-...'}
        disabled={savingWebKey}
        autocomplete="off"
      />
      <button type="button" onclick={handleSaveWebKey} disabled={savingWebKey}>
        {savingWebKey ? 'Kaydediliyor...' : 'Kaydet'}
      </button>
      {#if webStatus?.has_web_search_key}
        <button type="button" class="ai-key-clear" onclick={() => handleSaveWebKey(true)} disabled={savingWebKey}>
          Sil
        </button>
      {/if}
    </div>
    {#if webKeyMessage}
      <div class="message" class:error={webKeyError} class:success={!webKeyError}>{webKeyMessage}</div>
    {/if}
    {#if webStatus && !webStatus.has_web_search_key}
      <p class="field-hint">Anahtar tanımlı değil — sohbette İnternet modu kapalı kalır.</p>
    {/if}

    <h3 class="ai-subsection-title">Bulut YZ Sağlayıcıları (ücretsiz katmanlar)</h3>
    <p class="field-hint">
      Sohbetin YZ arka ucunu kendi <code>opencode</code> sunucunuz yerine ücretsiz bulut
      katmanlarından biri yapabilirsiniz. Bu mod yalnızca sohbet oturumunda
      <strong>İnternet modu</strong> açıkken çalışır (sorularınız sağlayıcıya gönderilir).
    </p>

    <div class="ai-key-row">
      <label for="cloud-provider">Sağlayıcı</label>
      <select id="cloud-provider" bind:value={cloudProvider} onchange={handleCloudConfigChange} disabled={savingCloud}>
        <option value="">Yok (yerel opencode kullan)</option>
        {#each cloudStatus?.presets ?? [] as preset (preset.id)}
          <option value={preset.id}>{preset.label}</option>
        {/each}
      </select>
    </div>

    {#if cloudProvider}
      {#each cloudStatus?.presets ?? [] as preset (preset.id)}
        {#if preset.id === cloudProvider}
          <p class="field-hint">{preset.hint}</p>
          {#if preset.needs_base_url}
            <div class="ai-key-row">
              <label for="cloud-base-url">Temel adres (OpenAI uyumlu)</label>
              <input
                id="cloud-base-url"
                type="text"
                bind:value={cloudBaseUrl}
                onchange={handleCloudConfigChange}
                placeholder="https://api.example.com/v1"
                disabled={savingCloud}
              />
            </div>
          {/if}
        {/if}
      {/each}

      <div class="ai-key-row">
        <label for="cloud-model">Model</label>
        {#if (cloudStatus?.models?.length ?? 0) > 0}
          <select id="cloud-model" bind:value={cloudModel} onchange={handleCloudConfigChange} disabled={savingCloud}>
            <option value="">Seçilmedi</option>
            {#each cloudStatus.models ?? [] as model (model)}
              <option value={model}>{model}</option>
            {/each}
          </select>
        {:else}
          <input
            id="cloud-model"
            type="text"
            bind:value={cloudModel}
            onchange={handleCloudConfigChange}
            placeholder="örn. llama-3.3-70b-versatile"
            disabled={savingCloud}
          />
        {/if}
        <button type="button" onclick={loadCloudModels} disabled={savingCloud || !cloudStatus?.has_key}>
          Modelleri Getir
        </button>
      </div>

      <div class="ai-key-row">
        <label for="cloud-key">API anahtarı</label>
        <input
          id="cloud-key"
          type="password"
          bind:value={cloudKeyInput}
          placeholder={cloudStatus?.has_key ? `Tanımlı (${cloudStatus.key_hint})` : 'sk-...'}
          disabled={savingCloud}
          autocomplete="off"
        />
        <button type="button" onclick={handleSaveCloudKey} disabled={savingCloud}>
          Kaydet
        </button>
        {#if cloudStatus?.has_key}
          <button type="button" class="ai-key-clear" onclick={() => handleSaveCloudKey(true)} disabled={savingCloud}>
            Sil
          </button>
        {/if}
      </div>

      {#if cloudStatus?.quota}
        <p class="field-hint">Kalan kota: {cloudStatus.quota}</p>
      {/if}
      {#if cloudMessage}
        <div class="message" class:error={cloudError} class:success={!cloudError}>{cloudMessage}</div>
      {/if}
    {/if}
  </div>

  <div class="settings-section">
    <h2>Güncellemeler</h2>
    <p class="field-hint section-hint">
      Güncellemeler imzalı paketlerle doğrulanır. İnternet bağlantısı gerektirir.
    </p>
    <div class="update-actions">
      <button
        type="button"
        disabled={updateChecking || updateInstalling}
        onclick={handleCheckUpdate}
      >
        {updateChecking ? 'Kontrol ediliyor...' : 'Güncellemeleri Kontrol Et'}
      </button>
      {#if updateAvailable}
        <button
          type="button"
          class="install-btn"
          disabled={updateInstalling}
          onclick={handleInstallUpdate}
        >
          {updateInstalling ? 'Kuruluyor...' : `v${updateAvailable.version} Kur`}
        </button>
      {/if}
    </div>
    {#if updateInstalling && downloadTotal > 0}
      <div class="progress-bar">
        <div
          class="progress-fill"
          style="width: {Math.min(100, (downloadProgress / downloadTotal) * 100)}%;"
        ></div>
      </div>
      <p class="field-hint">
        {Math.round((downloadProgress / downloadTotal) * 100)}% indirildi
      </p>
    {/if}
    {#if updateMessage}
      <div class="message" class:error={updateError} class:success={!updateError}>
        {updateMessage}
      </div>
    {/if}
    {#if updateAvailable?.body}
      <div class="update-notes">
        <p class="notes-title">Sürüm notları</p>
        <p class="notes-body">{updateAvailable.body}</p>
      </div>
    {/if}
  </div>

  <div class="settings-section">
    <h2>Uygulama Hakkında</h2>
    <div class="about-info">
      <div class="about-row">
        <span class="about-label">Uygulama</span>
        <span>Kukla v0.1.0</span>
      </div>
      <div class="about-row">
        <span class="about-label">Amaç</span>
        <span>İçtihat Arama Asistanı</span>
      </div>
      <div class="about-row">
        <span class="about-label">Veri Depolama</span>
        <span>Yerel (şifrelenmiş)</span>
      </div>
    </div>
  </div>
</div>

<style>
  .settings-panel {
    padding: 32px;
    max-width: 520px;
    overflow-y: auto;
    height: 100%;
  }

  .settings-header {
    margin-bottom: 32px;
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

  .settings-section {
    margin-bottom: 32px;
  }

  h2 {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 16px;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  label {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-secondary);
  }

  select {
    width: 100%;
    font-size: 14px;
    padding: 8px 12px;
    border-radius: var(--radius);
    background: var(--bg-primary);
    border: 1px solid var(--border);
    color: var(--text-primary);
  }

  .field-hint {
    font-size: 11px;
    color: var(--text-muted);
    margin: 0;
  }

  input {
    width: 100%;
  }

  button {
    background: var(--accent);
    color: white;
    padding: 10px 20px;
    border-radius: var(--radius);
    font-weight: 600;
    transition: background 0.15s;
    align-self: flex-start;
  }

  button:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .message {
    padding: 10px 14px;
    border-radius: var(--radius);
    font-size: 13px;
  }

  .message.error {
    background: rgba(239, 68, 68, 0.1);
    color: var(--danger);
  }

  .message.success {
    background: rgba(34, 197, 94, 0.1);
    color: #4ade80;
  }

  .ai-status-grid {
    display: flex;
    flex-direction: column;
    gap: 8px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 12px 16px;
    margin-bottom: 12px;
  }

  .ai-status-row {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    flex-wrap: wrap;
  }

  .ai-status-label {
    color: var(--text-muted);
    min-width: 180px;
  }

  .ai-status-value {
    color: var(--text-secondary);
    font-family: monospace;
    font-size: 12px;
    word-break: break-all;
  }

  .status-pill {
    padding: 2px 10px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 600;
    background: rgba(148, 163, 184, 0.18);
    color: var(--text-secondary);
    white-space: nowrap;
  }

  .status-pill.on {
    background: rgba(34, 197, 94, 0.15);
    color: #4ade80;
  }

  .status-pill.err {
    background: rgba(239, 68, 68, 0.15);
    color: var(--danger);
  }

  .ai-actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
    margin-top: 4px;
  }

  .ai-subsection-title {
    font-size: 13px;
    color: var(--text-secondary);
    margin: 22px 0 8px;
  }

  .ai-managed-note {
    color: var(--success);
    font-size: 11px;
    margin-left: 4px;
  }

  .ai-key-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .ai-key-row label {
    font-size: 12px;
    color: var(--text-secondary);
    flex-basis: 100%;
  }

  .ai-key-row input {
    flex: 1;
    min-width: 220px;
  }

  .ai-key-row button {
    flex-shrink: 0;
  }

  .ai-key-clear {
    background: rgba(239, 68, 68, 0.15) !important;
    color: var(--danger) !important;
  }

  .ai-detail {
    display: block;
    margin-top: 4px;
  }

  .about-info {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .about-row {
    display: flex;
    justify-content: space-between;
    padding: 12px 16px;
    font-size: 14px;
  }

  .about-row + .about-row {
    border-top: 1px solid var(--border);
  }

  .about-label {
    color: var(--text-muted);
  }

  .strength-meter {
    height: 6px;
    background: var(--bg-tertiary);
    border-radius: 3px;
    overflow: hidden;
    position: relative;
    margin-top: -4px;
  }

  .strength-bar {
    height: 100%;
    border-radius: 3px;
    transition: width 0.3s, background 0.3s;
  }

  .strength-label {
    font-size: 11px;
    font-weight: 600;
    margin-top: 4px;
    display: block;
  }

  .toggle-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 16px;
  }

  .toggle-label {
    font-size: 14px;
    font-weight: 600;
    display: block;
    margin-bottom: 4px;
  }

  .toggle-desc {
    font-size: 12px;
    color: var(--text-muted);
    margin: 0;
  }

  .switch {
    position: relative;
    display: inline-block;
    width: 44px;
    height: 24px;
    flex-shrink: 0;
  }

  .switch input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .slider {
    position: absolute;
    cursor: pointer;
    inset: 0;
    background: var(--bg-tertiary);
    border-radius: 24px;
    transition: 0.2s;
  }

  .slider::before {
    position: absolute;
    content: '';
    height: 18px;
    width: 18px;
    left: 3px;
    bottom: 3px;
    background: white;
    border-radius: 50%;
    transition: 0.2s;
  }

  .switch input:checked + .slider {
    background: var(--accent);
  }

  .switch input:checked + .slider::before {
    transform: translateX(20px);
  }

  .section-hint {
    margin: -8px 0 12px;
  }

  .update-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .install-btn {
    background: #4ade80;
    color: #052e16;
  }

  .install-btn:hover:not(:disabled) {
    background: #22c55e;
  }

  .progress-bar {
    height: 6px;
    background: var(--bg-tertiary);
    border-radius: 3px;
    overflow: hidden;
    margin-top: 12px;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.2s;
  }

  .path-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-bottom: 12px;
  }

  .path-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .path-label {
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .path-value {
    font-size: 12px;
    color: var(--text-secondary);
    word-break: break-all;
    background: var(--bg-tertiary);
    padding: 8px 10px;
    border-radius: var(--radius);
  }

  .path-open-btn {
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    padding: 8px 14px;
    border-radius: var(--radius);
    font-size: 13px;
    font-weight: 500;
  }

  .path-open-btn:hover {
    background: var(--accent-muted);
    color: var(--accent);
  }

  .update-notes {
    margin-top: 12px;
    padding: 12px 14px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-size: 13px;
  }

  .notes-title {
    font-weight: 600;
    margin: 0 0 6px;
    color: var(--text-secondary);
  }

  .notes-body {
    margin: 0;
    color: var(--text-primary);
    white-space: pre-wrap;
  }
</style>
