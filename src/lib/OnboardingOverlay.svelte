<script>
  let { onclose } = $props();

  let step = $state(1);
  let modalEl = $state(null);
  const totalSteps = 3;

  function next() {
    if (step < totalSteps) step++;
    else onclose();
  }

  function prev() {
    if (step > 1) step--;
  }

  function handleOverlayKeydown(e) {
    if (e.key === 'Escape') {
      onclose();
      return;
    }
    if (e.key !== 'Tab' || !modalEl) return;

    const focusable = modalEl.querySelectorAll(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    );
    if (focusable.length === 0) return;

    const first = focusable[0];
    const last = focusable[focusable.length - 1];

    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }

  $effect(() => {
    if (!modalEl) return;
    const primaryBtn = modalEl.querySelector('.btn-primary');
    primaryBtn?.focus();
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="overlay" role="presentation" onclick={onclose} onkeydown={handleOverlayKeydown}>
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="modal"
    bind:this={modalEl}
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    aria-labelledby="onboarding-title"
    tabindex="-1"
  >
    <div class="steps" aria-hidden="true">
      {#each Array(totalSteps) as _, i}
        <span class="dot" class:active={i + 1 === step} class:done={i + 1 < step}></span>
      {/each}
    </div>

    {#if step === 1}
      <h2 id="onboarding-title">Kukla'ya Hoş Geldiniz</h2>
      <p>Kukla, yapay zeka destekli bir içtihat arama asistanıdır. Yargıtay, Danıştay ve BAM kararlarını hızlıca arayabilir, özetleyebilir ve dışa aktarabilirsiniz.</p>
      <div class="illustration">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="1.5" aria-hidden="true">
          <circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>
        </svg>
      </div>
    {:else if step === 2}
      <h2 id="onboarding-title">Nasıl Çalışır?</h2>
      <ul>
        <li>Dava özetinizi yazın veya PDF yükleyin</li>
        <li>Kukla, en benzer Yargıtay kararlarını bulur</li>
        <li>İstediğiniz kararı özetletin veya dışa aktarın</li>
        <li>Tüm veriler yerel kalır, KVKK uyumludur</li>
      </ul>
    {:else if step === 3}
      <h2 id="onboarding-title">Hazırsınız!</h2>
      <p>Dava özetinizi yazarak hemen başlayabilirsiniz. Klavye kısayolları için <kbd>Ctrl+/</kbd>, sık sorulan sorular için sol menüdeki <strong>SSS</strong> butonunu kullanın.</p>
      <p class="disclaimer">Kukla hukuki tavsiye vermez. Kararları her zaman kendiniz doğrulayın.</p>
    {/if}

    <div class="nav">
      {#if step > 1}
        <button class="btn-secondary" onclick={prev}>Geri</button>
      {:else}
        <div></div>
      {/if}
      <button class="btn-primary" onclick={next}>
        {step === totalSteps ? 'Başla' : 'İleri'}
      </button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    backdrop-filter: blur(4px);
  }

  .modal {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 32px;
    width: 420px;
    max-width: 90vw;
    box-shadow: var(--shadow-lg);
  }

  .steps {
    display: flex;
    justify-content: center;
    gap: 8px;
    margin-bottom: 24px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--bg-tertiary);
    transition: all 0.3s;
  }

  .dot.active {
    background: var(--accent);
    width: 24px;
    border-radius: 4px;
  }

  .dot.done {
    background: var(--accent-muted);
  }

  h2 {
    font-size: 20px;
    font-weight: 700;
    margin-bottom: 12px;
    text-align: center;
  }

  p {
    font-size: 14px;
    color: var(--text-secondary);
    line-height: 1.6;
    margin-bottom: 16px;
    text-align: center;
  }

  .illustration {
    text-align: center;
    margin: 16px 0;
  }

  ul {
    list-style: none;
    padding: 0;
    margin: 0 0 16px;
  }

  li {
    padding: 8px 0 8px 24px;
    position: relative;
    font-size: 14px;
    color: var(--text-primary);
  }

  li::before {
    content: '';
    position: absolute;
    left: 4px;
    top: 12px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }

  kbd {
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 12px;
  }

  .disclaimer {
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
    padding: 12px;
    background: var(--bg-tertiary);
    border-radius: var(--radius);
    margin-bottom: 0;
  }

  .nav {
    display: flex;
    justify-content: space-between;
    margin-top: 24px;
  }

  .btn-primary {
    background: var(--accent);
    color: white;
    padding: 10px 24px;
    border-radius: var(--radius);
    font-weight: 600;
    transition: background 0.15s;
  }

  .btn-primary:hover {
    background: var(--accent-hover);
  }

  .btn-secondary {
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    padding: 10px 24px;
    border-radius: var(--radius);
    font-weight: 500;
    transition: all 0.15s;
  }

  .btn-secondary:hover {
    background: var(--border);
    color: var(--text-primary);
  }
</style>