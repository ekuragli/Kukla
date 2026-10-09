<script>
  import { invoke } from '@tauri-apps/api/core';
  import {
    formatInvokeError,
    validatePasswordRules,
    computeStrength,
    strengthLabel,
    strengthColor,
  } from './formatters.js';

  let { onunlock } = $props();

  let password = $state('');
  let confirmPassword = $state('');
  let error = $state('');
  let submitting = $state(false);
  let isFirstTime = $state(false);
  let checking = $state(true);
  let lockoutSeconds = $state(0);
  let lockoutTimer = $state(null);

  async function checkLockout() {
    try {
      const secs = await invoke('get_lockout_remaining');
      if (secs > 0) {
        lockoutSeconds = secs;
        if (lockoutTimer) clearInterval(lockoutTimer);
        lockoutTimer = setInterval(() => {
          lockoutSeconds--;
          if (lockoutSeconds <= 0) {
            clearInterval(lockoutTimer);
            lockoutTimer = null;
          }
        }, 1000);
      }
    } catch (_) {}
  }

  $effect(() => {
    invoke('auth_check')
      .then((hasPassword) => { isFirstTime = !hasPassword; checking = false; checkLockout(); })
      .catch(() => { isFirstTime = true; checking = false; });
  });

  async function handleSubmit(e) {
    e.preventDefault();
    error = '';

    if (isFirstTime) {
      const ruleErrors = validatePasswordRules(password);
      if (ruleErrors.length > 0) {
        error = ruleErrors[0];
        return;
      }
      if (password !== confirmPassword) {
        error = 'Şifreler eşleşmiyor.';
        return;
      }
    }

    submitting = true;
    try {
      const result = await invoke('auth_unlock', { password });
      if (result) {
        if (lockoutTimer) clearInterval(lockoutTimer);
        onunlock();
      } else {
        checkLockout();
        error = 'Yanlış şifre.';
      }
    } catch (err) {
      error = formatInvokeError(err) || 'Bir hata oluştu.';
      checkLockout();
    } finally {
      submitting = false;
    }
  }
</script>

<div class="lock-screen">
  <div class="lock-card">
    <div class="logo">
      <div class="logo-icon">K</div>
    </div>
    <h1>Kukla</h1>
    <p class="subtitle">İçtihat Arama Asistanı</p>

    {#if checking}
      <div class="checking">Kontrol ediliyor...</div>
    {:else if lockoutSeconds > 0}
      <div class="lockout-message">
        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>
        </svg>
        <p>5 başarısız deneme. Hesap kilitlendi.</p>
        <p class="lockout-timer">{Math.floor(lockoutSeconds / 60)}:{String(lockoutSeconds % 60).padStart(2, '0')} sonra tekrar deneyin.</p>
      </div>
    {:else}
      <form onsubmit={handleSubmit}>
        {#if error}
          <div class="error">{error}</div>
        {/if}

        <p class="info">
          {isFirstTime ? 'Hoş geldiniz! Lütfen bir şifre belirleyin.' : 'Devam etmek için şifrenizi girin.'}
        </p>

        <p class="password-warning" role="note">
          {#if isFirstTime}
            Şifrenizi güvenli bir yerde saklayın. Şifre sıfırlama yoktur; unutursanız tüm verilerinize erişimi kaybedersiniz.
          {:else}
            Şifrenizi mi unuttunuz? Güvenlik nedeniyle şifre kurtarma mümkün değildir.
          {/if}
        </p>

        <label class="sr-only" for="lock-password">Şifre</label>
        <input id="lock-password" type="password" placeholder="Şifre" bind:value={password} autocomplete="current-password" />

        {#if isFirstTime}
          <label class="sr-only" for="lock-password-confirm">Şifre (tekrar)</label>
          <input id="lock-password-confirm" type="password" placeholder="Şifre (tekrar)" bind:value={confirmPassword} autocomplete="new-password" />
        {/if}

        {#if isFirstTime && password.length > 0}
          {@const score = computeStrength(password)}
          <div class="strength-meter">
            <div class="strength-bar" style="width: {score}%; background: {strengthColor(score)};"></div>
            <span class="strength-label" style="color: {strengthColor(score)};">
              {strengthLabel(score)} ({score}%)
            </span>
          </div>
          <div class="strength-requirements">
            <span class="req" class:met={password.length >= 12}>En az 12 karakter</span>
            <span class="req" class:met={/[A-Z]/.test(password)}>Büyük harf</span>
            <span class="req" class:met={/[a-z]/.test(password)}>Küçük harf</span>
            <span class="req" class:met={/[0-9]/.test(password)}>Rakam</span>
            <span class="req" class:met={/[^A-Za-z0-9]/.test(password)}>Özel karakter</span>
          </div>
        {/if}

        <button type="submit" disabled={submitting || !password}>
          {submitting ? 'Kontrol ediliyor...' : (isFirstTime ? 'Uygulamayı Başlat' : 'Giriş Yap')}
        </button>
      </form>
    {/if}
  </div>
</div>

<style>
  .lock-screen {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    background: var(--bg-primary);
  }

  .lock-card {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 40px;
    width: 380px;
    text-align: center;
    box-shadow: var(--shadow);
  }

  .logo {
    margin-bottom: 16px;
  }

  .logo-icon {
    width: 56px;
    height: 56px;
    background: var(--accent);
    border-radius: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 28px;
    font-weight: 700;
    margin: 0 auto;
    color: white;
  }

  h1 {
    font-size: 24px;
    font-weight: 700;
    margin-bottom: 4px;
  }

  .subtitle {
    color: var(--text-secondary);
    font-size: 13px;
    margin-bottom: 24px;
  }

  .checking {
    color: var(--text-secondary);
    font-size: 14px;
    padding: 20px 0;
  }

  .info {
    color: var(--text-secondary);
    font-size: 13px;
    margin-bottom: 8px;
  }

  .password-warning {
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1.5;
    margin-bottom: 16px;
    padding: 10px 12px;
    background: var(--bg-tertiary);
    border-radius: var(--radius);
    border: 1px solid var(--border);
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 10px;
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
    transition: background 0.2s;
    margin-top: 6px;
  }

  button:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .error {
    background: rgba(239, 68, 68, 0.1);
    color: var(--danger);
    padding: 8px 12px;
    border-radius: var(--radius);
    font-size: 13px;
  }

  .lockout-message {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    color: var(--danger);
    padding: 24px 0;
  }

  .lockout-message p {
    font-size: 14px;
  }

  .lockout-timer {
    font-size: 20px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .strength-meter {
    height: 6px;
    background: var(--bg-tertiary);
    border-radius: 3px;
    overflow: hidden;
    position: relative;
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

  .strength-requirements {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 4px;
  }

  .req {
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 3px;
    background: var(--bg-tertiary);
    color: var(--text-muted);
  }

  .req.met {
    background: rgba(74, 222, 128, 0.15);
    color: #4ade80;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
</style>
