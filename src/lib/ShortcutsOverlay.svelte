<script>
  let { onclose, modKey = 'Ctrl' } = $props();

  let modalEl = $state(null);

  function handleKeydown(e) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onclose();
      return;
    }
    if (e.key === 'Tab') {
      const focusable = modalEl?.querySelectorAll(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );
      if (!focusable || focusable.length === 0) return;
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
  }

  $effect(() => {
    modalEl?.querySelector('.close-btn')?.focus();
  });

  const shortcuts = $derived([
    { keys: ['Enter'], desc: 'Arama yap (Shift+Enter: yeni satır)' },
    { keys: ['Esc'], desc: 'Geri / kapat' },
    { keys: [`${modKey}+K`], desc: 'Arama kutusuna odaklan' },
    { keys: [`${modKey}+E`], desc: 'Kararı dışa aktar (detay ekranı)' },
    { keys: [`${modKey}+L`], desc: 'Uygulamayı kilitle' },
    { keys: ['↑', '↓'], desc: 'Sonuçlar arasında gezin' },
    { keys: [`${modKey}+1`], desc: 'Arama görünümü' },
    { keys: [`${modKey}+2`], desc: 'PDF yükleme' },
    { keys: [`${modKey}+3`], desc: 'Kişisel arşiv' },
    { keys: [`${modKey}+4`], desc: 'Ayarlar' },
{ keys: [`${modKey}+5`], desc: 'Araçlar (dosya d\u00f6n\u015f\u00fct\u00fcr\u00fcc\u00fc)' },
    { keys: [`${modKey}+/`], desc: 'Kısayol listesi' },
  ]);
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="overlay"
  role="presentation"
  onclick={onclose}
  onkeydown={handleKeydown}
>
  <div
    class="modal"
    bind:this={modalEl}
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    aria-labelledby="shortcuts-title"
    tabindex="-1"
  >
    <h2 id="shortcuts-title">Klavye Kısayolları</h2>
    <ul class="shortcut-list">
      {#each shortcuts as item}
        <li>
          <span class="keys">
            {#each item.keys as key, i}
              {#if i > 0}<span class="sep">/</span>{/if}
              <kbd>{key}</kbd>
            {/each}
          </span>
          <span class="desc">{item.desc}</span>
        </li>
      {/each}
    </ul>
    <button type="button" class="close-btn" onclick={onclose}>Kapat (Esc)</button>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 110;
    backdrop-filter: blur(4px);
  }

  .modal {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 24px;
    width: 440px;
    max-width: 92vw;
    max-height: 80vh;
    overflow-y: auto;
    box-shadow: var(--shadow-lg);
  }

  h2 {
    font-size: 18px;
    font-weight: 700;
    margin-bottom: 16px;
  }

  .shortcut-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0 0 16px;
    padding: 0;
  }

  .shortcut-list li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    font-size: 13px;
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }

  .shortcut-list li:last-child {
    border-bottom: none;
  }

  .keys {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  kbd {
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 2px 6px;
    font-size: 11px;
    font-family: inherit;
    color: var(--text-primary);
  }

  .sep {
    color: var(--text-muted);
    font-size: 11px;
  }

  .desc {
    color: var(--text-secondary);
    text-align: right;
  }

  .close-btn {
    width: 100%;
    padding: 10px;
    border-radius: var(--radius);
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    font-size: 13px;
    font-weight: 600;
  }

  .close-btn:hover {
    background: var(--accent-muted);
    color: var(--accent);
  }
</style>