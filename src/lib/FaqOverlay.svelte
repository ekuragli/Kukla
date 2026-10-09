<script>
  let { onclose } = $props();

  let modalEl = $state(null);

  function close() {
    onclose();
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
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
    modalEl?.querySelector('.btn-close')?.focus();
  });

  const faqs = [
    {
      q: 'Verilerim güvende mi?',
      a: 'Evet. Tüm veriler cihazınızda şifreli olarak saklanır. Kukla local-first çalışır; veriler otomatik olarak internete gönderilmez.',
    },
    {
      q: 'LLM cevapları doğru mu?',
      a: 'Sistem her zaman kaynak karar referansı gösterir. LLM cevabı yalnızca getirilen karar metnine dayanır. Düşük kaynak benzerliğinde uyarı gösterilir.',
    },
    {
      q: 'Kendi kararlarımı ekleyebilir miyim?',
      a: 'Evet. Ayarlar\'dan Kişisel Karar Arşivi\'ni etkinleştirin, KVKK onayı verin ve PDF yükleyin.',
    },
    {
      q: 'Rapor nasıl dışa aktarılır?',
      a: 'Karar detayında format seçip (TXT/DOCX/PDF) Dışa Aktar butonuna tıklayın; rapor masaüstüne kaydedilir. Ayrıca Karar Metni, YZ Özeti ve Ratio Decidendi bölümlerinin her biri için ayrı biçim seçip İndir ile tek bölüm indirilebilir.',
    },
    {
      q: 'Dosya dönüştürücü ne yapar?',
      a: 'Araçlar sekmesi (Ctrl/Cmd+5) PDF, DOCX, TXT ve UYAP .udf dosyalarını TXT/DOCX/PDF biçimine çevirir. .udf bir ZIP arşividir; metin content.xml içinden okunur. E-imza (sign.sgn) PDF\'e taşınmaz, çıktı ayrıca imzalanmalıdır. Taranmış (görüntü tabanlı) PDF\'lerden metin çıkarılamaz.',
    },
    {
      q: 'Şifremi unuttum, ne yapmalıyım?',
      a: 'Güvenlik nedeniyle şifre kurtarma yoktur. Şifrenizi kaybederseniz tüm verilere erişimi kaybedersiniz.',
    },
    {
      q: 'YZ özellikleri nasıl çalışır?',
      a: 'Özet ve dilekçe üretimi, terminalde `opencode serve --port 4096` ile başlatılan sunucuya bağlanır; modeli Ayarlar → YZ Altyapısı bölümünden seçersiniz. Sunucu yoksa bu özellikler pasif kalır, arama etkilenmez. Semantik arama ise yerel embedding modeliyle çevrimdışı çalışır.',
    },
  ];
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="overlay" onclick={close} role="presentation">
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="modal" bind:this={modalEl} onkeydown={handleKeydown} onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-labelledby="faq-title" tabindex="-1">
    <h2 id="faq-title">Sık Sorulan Sorular</h2>

    <dl class="faq-list">
      {#each faqs as item}
        <dt>{item.q}</dt>
        <dd>{item.a}</dd>
      {/each}
    </dl>

    <button class="btn-close" onclick={close}>Kapat</button>
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
    padding: 28px;
    width: 520px;
    max-width: 92vw;
    max-height: 80vh;
    overflow-y: auto;
  }

  h2 {
    font-size: 18px;
    font-weight: 700;
    margin-bottom: 16px;
  }

  .faq-list {
    margin: 0 0 20px;
  }

  dt {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
    margin-top: 14px;
  }

  dt:first-child {
    margin-top: 0;
  }

  dd {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.6;
    margin: 6px 0 0;
  }

  .btn-close {
    width: 100%;
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    padding: 10px;
    border-radius: var(--radius);
    font-weight: 500;
  }

  .btn-close:hover {
    background: var(--border);
    color: var(--text-primary);
  }
</style>