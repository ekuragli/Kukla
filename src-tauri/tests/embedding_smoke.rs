use kukla_lib::core::embedding::Embedder;

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (na * nb)
}

#[tokio::test]
#[ignore = "huggingface'den ~90MB model indirir"]
async fn downloads_model_and_embeds_to_384_dims() {
    let dir = Embedder::download_missing().await.expect("model indirilemedi");
    assert!(Embedder::files_ready());

    let embedder = tokio::task::spawn_blocking(move || Embedder::load(&dir))
        .await
        .expect("yükleme görevi başarısız")
        .expect("model yüklenemedi");

    assert_eq!(embedder.dim(), 384);

    let vector = embedder.embed("iş kazası tazminatı talebi").expect("embedding üretilemedi");
    assert_eq!(vector.len(), 384);
    let norm: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 1e-3,
        "vektör normalize değil: {}",
        norm
    );

    let target = embedder.embed("iş kazasından doğan manevi tazminat").unwrap();
    let related = embedder.embed("işe bağlı tazminat davası").unwrap();
    let unrelated = embedder.embed("kira bedelinin uylaştırılması").unwrap();

    let related_score = cosine(&target, &related);
    let unrelated_score = cosine(&target, &unrelated);
    println!(
        "benzerlik: ilgili={:.4} ilgisiz={:.4}",
        related_score, unrelated_score
    );
    assert!(
        related_score > unrelated_score,
        "anlamsal benzerlik beklenenin tersi: {} <= {}",
        related_score,
        unrelated_score
    );

    let batch = embedder
        .embed_batch(&["birinci metin".to_string(), "ikinci metin".to_string()])
        .expect("batch embedding üretilemedi");
    assert_eq!(batch.len(), 2);
    assert_eq!(batch[0].len(), 384);
}
