use std::sync::atomic::{AtomicUsize, Ordering};

use kukla_lib::core::opencode_client::{OpencodeClient, PromptHooks, DEFAULT_BASE_URL};

#[tokio::test]
#[ignore = "opencode sunucusu gerektirir (Kukla çalışırken otomatik başlatılır)"]
async fn status_lists_models_and_session_uses_selected_model() {
    let client = OpencodeClient::new(DEFAULT_BASE_URL);

    let status = client.status().await;
    if !status.healthy {
        // Sunucu kapalıysa test anlamsızdır: sesli şekilde atlanır.
        eprintln!(
            "ATLANDI: opencode sunucusu ayakta değil ({:?}) — Kukla'yı açın veya 'opencode serve' başlatın",
            status.detail
        );
        return;
    }
    println!("durum: healthy={} son model={:?}", status.healthy, status.model);

    let models = client.list_models().await;
    assert!(!models.is_empty(), "model listesi boş geldi");
    println!(
        "model sayısı: {}, örnek: {}",
        models.len(),
        models
            .iter()
            .find(|model| model.id.contains("mimo"))
            .or_else(|| models.first())
            .map(|model| model.label())
            .unwrap_or_default()
    );

    // Seçilen model oturuma gerçekten aktarılıyor mu? (sağlayıcı çalışmasa da doğrulanabilir)
    let selection = models
        .iter()
        .find(|model| model.id == "mimo-v2.6-flash-free")
        .or_else(|| models.first())
        .map(|model| model.label())
        .expect("model listesi boş");

    let session_id = client
        .create_session(Some(&selection))
        .await
        .expect("oturum açılamadı");
    let accepted = client
        .session_model(&session_id)
        .await
        .expect("oturum modeli okunamadı");
    assert_eq!(accepted, selection, "oturumda farklı model kabul edildi");
    println!("oturum modeli: {}", accepted);

    // Varsayılan model seçilirse oturum varsayılanla açılır.
    let plain_session = client.create_session(None).await.expect("oturum açılamadı");
    if let Some(default_model) = client.session_model(&plain_session).await {
        println!("varsayılan oturum modeli: {}", default_model);
    }

    // Seçili modelle uçtan uca istek: sağlayıcı hatası (plan/downstream) kabul edilir,
    // bağlantı veya JSON çözümleme hatası başarısızlıktır.
    match client
        .chat("Tek kelime ile cevap ver: HAZIR", Some(&selection))
        .await
    {
        Ok(reply) => {
            assert!(!reply.text.trim().is_empty(), "model boş yanıt döndürdü");
            println!(
                "yanıt: model={:?} agent={:?} metin={}",
                reply.model, reply.agent, reply.text
            );
        }
        Err(message) => {
            println!("istek hatası: {}", message);
            assert!(
                message.contains("model hatası") || message.contains("boş yanıt"),
                "bağlantı/çözümleme hatası: {}",
                message
            );
        }
    }
}

/// İlerleme geri çağrısı kısmi metni bildirmeli; önceden iptal edilen istek
/// hemen `"İstek iptal edildi."` hatasıyla durmalı.
#[tokio::test]
#[ignore = "opencode sunucusu gerektirir (Kukla çalışırken otomatik başlatılır)"]
async fn streaming_hooks_emit_progress_and_cancel() {
    let client = OpencodeClient::new(DEFAULT_BASE_URL);
    let models = client.list_models().await;
    if models.is_empty() {
        eprintln!(
            "ATLANDI: model listesi boş — opencode sunucusu kapalı olabilir (Kukla çalışırken otomatik başlatılır)"
        );
        return;
    }
    let selection = models
        .iter()
        .find(|model| model.id == "mimo-v2.6-flash-free")
        .or_else(|| models.first())
        .map(|model| model.label())
        .expect("model listesi boş");

    let progress_count = AtomicUsize::new(0);
    let on_progress = |_session: &str, _text: &str| {
        progress_count.fetch_add(1, Ordering::SeqCst);
    };
    let hooks = PromptHooks {
        on_progress: Some(&on_progress),
        is_cancelled: None,
    };

    let result = client
        .chat_with_hooks(
            "Yaklaşık 300 kelimelik kısa bir hikaye yaz.",
            Some(&selection),
            &hooks,
        )
        .await;
    let count = progress_count.load(Ordering::SeqCst);
    match result {
        Ok(reply) => {
            println!("akış yanıtı: {} ({} ilerleme bildirimi)", reply.text.trim(), count);
            assert!(count >= 1, "ilerleme geri çağrısı hiç tetiklenmedi");
        }
        Err(message) => {
            println!("akış hatası: {} ({} ilerleme bildirimi)", message, count);
            assert!(
                message.contains("model hatası") || message.contains("boş yanıt"),
                "bağlantı/çözümleme hatası: {}",
                message
            );
        }
    }

    // Önceden iptal: ilk yoklama döngüsünde hemen dönmeli.
    let pre_cancelled = || true;
    let cancel_hooks = PromptHooks {
        on_progress: None,
        is_cancelled: Some(&pre_cancelled),
    };
    let started = std::time::Instant::now();
    let cancelled = client
        .chat_with_hooks("Bu yanıt üretilmemeli.", Some(&selection), &cancel_hooks)
        .await;
    match cancelled {
        Err(message) => {
            assert!(message.contains("iptal"), "iptal mesajı gelmedi: {}", message);
            println!(
                "iptal {:.1}s içinde döndü: {}",
                started.elapsed().as_secs_f64(),
                message
            );
        }
        Ok(reply) => panic!("iptal edilen istek yanıt döndürdü: {}", reply.text),
    }
}
