<script>
  let { onaccept } = $props();

  let accepted = $state(false);
  let modalEl = $state(null);

  function handleAccept() {
    if (!accepted) return;
    try {
      localStorage.setItem('kukla_legal_consent_v1', new Date().toISOString());
    } catch (_) {}
    onaccept();
  }

  $effect(() => {
    modalEl?.querySelector('.btn-primary')?.focus();
  });
</script>

<div class="overlay" role="presentation">
  <div
    class="modal"
    bind:this={modalEl}
    role="dialog"
    aria-modal="true"
    aria-labelledby="legal-title"
    tabindex="-1"
  >
    <h2 id="legal-title">Kullanım Koşulları ve Sorumluluk Reddi</h2>

    <div class="legal-body">
      <p>
        <strong>Kukla</strong> bir yardımcı araçtır. Ürettiği özetler, benzerlik skorları ve raporlar
        tavsiye niteliğindedir; hukuki bağlayıcılığı yoktur.
      </p>
      <ul>
        <li>Tüm kararları ve YZ çıktılarını kendi mesleki bilgi ve sorumluluğunuzla doğrulamalısınız.</li>
        <li>Kukla hukuki tavsiye vermez ve dava sonucu garantisi sunmaz.</li>
        <li>Verileriniz cihazınızda şifreli saklanır; local-first çalışır (KVKK uyumlu tasarım).</li>
        <li>Kişisel arşive yüklediğiniz dosyaların yasal kullanım hakkına sahip olduğunuzu taahhüt edersiniz.</li>
        <li>Şifre sıfırlama yoktur; şifrenizi kaybederseniz verilere erişimi kaybedersiniz.</li>
      </ul>
    </div>

    <label class="consent-check">
      <input type="checkbox" bind:checked={accepted} />
      <span>Kullanım koşullarını ve sorumluluk reddini okudum, kabul ediyorum.</span>
    </label>

    <button class="btn-primary" onclick={handleAccept} disabled={!accepted}>
      Kabul Et ve Devam Et
    </button>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.75);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
    backdrop-filter: blur(4px);
  }

  .modal {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 28px;
    width: 480px;
    max-width: 92vw;
    max-height: 85vh;
    overflow-y: auto;
    box-shadow: var(--shadow-lg);
  }

  h2 {
    font-size: 18px;
    font-weight: 700;
    margin-bottom: 16px;
  }

  .legal-body {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.65;
    margin-bottom: 16px;
  }

  .legal-body ul {
    margin: 12px 0 0;
    padding-left: 18px;
  }

  .legal-body li {
    margin-bottom: 8px;
  }

  .consent-check {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    font-size: 13px;
    color: var(--text-primary);
    margin-bottom: 16px;
    cursor: pointer;
  }

  .consent-check input {
    margin-top: 3px;
    flex-shrink: 0;
  }

  .btn-primary {
    width: 100%;
    background: var(--accent);
    color: white;
    padding: 12px;
    border-radius: var(--radius);
    font-weight: 600;
    transition: background 0.15s;
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>