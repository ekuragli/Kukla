<script>
  import { invoke } from '@tauri-apps/api/core';
  import LockScreen from './lib/LockScreen.svelte';
  import MainLayout from './lib/MainLayout.svelte';
  import LegalConsentOverlay from './lib/LegalConsentOverlay.svelte';

  let isLocked = $state(true);
  let hasPassword = $state(null);
  let ready = $state(false);
  let initError = $state('');
  let needsLegalConsent = $state(false);

  function checkLegalConsent() {
    try {
      needsLegalConsent = !localStorage.getItem('kukla_legal_consent_v1');
    } catch (_) {
      needsLegalConsent = true;
    }
  }

  $effect(() => {
    invoke('auth_check').then((r) => {
      hasPassword = r;
      // Şifre tanımlı olsa bile her açılışta kilidi göster (auth_check ≠ oturum açık).
      isLocked = true;
      ready = true;
    }).catch((e) => {
      initError = typeof e === 'string' ? e : 'Uygulama başlatılamadı.';
      ready = true;
    });
  });
</script>

{#if !ready}
  <div style="padding:40px;font-family:sans-serif;background:#1e293b;color:#f1f5f9;height:100vh">
    <p>Yükleniyor...</p>
  </div>
{:else if initError}
  <div style="padding:40px;font-family:sans-serif;background:#1e293b;color:#f1f5f9;height:100vh">
    <p role="alert">{initError}</p>
  </div>
{:else if isLocked}
  <LockScreen onunlock={() => { isLocked = false; checkLegalConsent(); }} />
{:else if needsLegalConsent}
  <LegalConsentOverlay onaccept={() => needsLegalConsent = false} />
{:else}
  <MainLayout onlock={() => isLocked = true} />
{/if}