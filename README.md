# Kukla — İçtihat Arama Asistanı

Kukla, avukatlar için yerel çalışan bir AI destekli içtihat arama masaüstü uygulamasıdır. Dava özetinizi yazarak Yargıtay/Danıştay kararlarını arayabilir, PDF yükleyebilir ve YZ destekli özet alabilirsiniz.

## Özellikler

- Hibrit arama: Bedesten (çevrimiçi) + yerel semantic arama, anahtar kelime re-ranking ile birleştirilir
- PDF indeksleme ve kişisel karar arşivi (KVKK onaylı)
- **Sohbet asistanı (Ctrl+J): yerel arşivden RAG destekli çok turlu sohbet; oturumlar şifreli veritabanında saklanır**
- **İnternet modu (opsiyonel): sohbette açıldığında sorular Tavily ile internette de aranır; ilk kullanımda açık onay istenir**
- **Ücretsiz bulut YZ sağlayıcıları: sohbetin arka ucu yerel opencode yerine OpenRouter (`:free`), Groq, Gemini veya özel OpenAI uyumlu uç nokta olabilir (canlı yazım/streaming destekli)**
- Seçili kararı sohbete bağlam olarak gönderme ("Sohbette Sor")
- Terminal üzerinden `opencode` oturumu ile YZ özet, ratio decidendi ve hallucination kontrolü
- Şifre koruması, oturum kilidi ve şifreli veri depolama
- TXT / DOCX / PDF rapor dışa aktarma (rapor ve her bölüm ayrı ayrı indirilebilir)
- Araçlar sekmesinde dosya dönüştürücü: PDF · DOCX · TXT · UYAP `.udf` → TXT / DOCX / PDF

## Gereksinimler

| Araç | Sürüm |
|------|-------|
| [Node.js](https://nodejs.org/) | 20+ |
| [Rust](https://rustup.rs/) | 1.80+ |
| [opencode](https://opencode.ai/) | (YZ özet ve dilekçe üretimi için) |

YZ özellikleri harici bir LLM sunucusu yerine **opencode** üzerinden çalışır.
Sunucuyu siz başlatmak zorunda değilsiniz: uygulama ilk YZ isteğinde (veya
açılışta arka planda) sunucuyu kontrol eder, ulaşılamıyorsa **gizli bir alt
süreç olarak kendisi başlatır** ve uygulama kapanınca durdurur. Çalışan bir
sunucunuz varsa (kendi terminalinizden başlattıysanız) ona dokunulmaz.
Modeli **Ayarlar → YZ Altyapısı** bölümünden seçersiniz; seçmezseniz
sunucudaki ilk çalışan model kullanılır.

```bash
opencode                     # TUI: opencode'un kendi varsayılan modelini değiştirmek için
```

- Farklı port: `set KUKLA_OPENCODE_URL=http://127.0.0.1:5000` ortam değişkeni
  (veya 4096 doluysa uygulama 4097-4099'u otomatik dener).
- Sunucu durumunu Ayarlar → YZ Altyapısı bölümünden izleyebilir, "Sunucuyu
  Başlat" ile elle de tetikleyebilirsiniz.
- `opencode` kurulu değilse uygulama çalışmaya devam eder; yalnızca YZ özet,
  dilekçe ve sohbet pasif kalır, çevrimiçi/yerel arama etkilenmez. Sohbet için
  alternatif olarak ücretsiz bulut sağlayıcıları (Ayarlar → YZ Altyapısı)
  yapılandırılabilir.

### Yerel Embedding Modeli

Semantik arama ve indeksleme yerel modelle çalışır: **all-MiniLM-L6-v2** (384 boyut,
Candle ile CPU üzerinde). İlk kullanımda Ayarlar → YZ Altyapısı → "Embedding Modelini
İndir" ile ~87 MB indirilir:

```
%LOCALAPPDATA%\kukla\models\all-MiniLM-L6-v2\   # config.json, tokenizer.json, model.safetensors
```

Model boyutu 1024'ten 384'e düştüğü için eski vektörler otomatik olarak atılır
(ilk aramada semantik sonuçlar geçici olarak boş görünebilir, kararları yeniden
indeksleyin).

## Kurulum

```bash
# Bağımlılıklar
npm install

# Geliştirme modu (Tauri + Vite)
npm run tauri:dev

# Yalnızca frontend
npm run dev
```

## Derleme

```bash
npm run build          # Frontend
npm run tauri:build    # Masaüstü installer (.msi + .exe)
```

## Otomatik Güncelleme (Updater)

Uygulama Tauri Updater ile imzalı güncellemeleri destekler. Ayarlar → Güncellemeler bölümünden manuel kontrol yapılabilir.

### İlk Kurulum

İmzalama anahtarları `src-tauri/keys/` altında:

| Dosya | Açıklama |
|-------|----------|
| `kukla.key` | Özel anahtar — **asla paylaşmayın veya commit etmeyin** |
| `kukla.key.pub` | Genel anahtar — `tauri.conf.json` içinde `pubkey` olarak kullanılır |

Yeni anahtar üretmek için:

```bash
npx tauri signer generate -w src-tauri/keys/kukla.key
```

### Release Derleme

Windows PowerShell:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = "src-tauri/keys/kukla.key"
npm run tauri:build
```

Derleme sonrası `src-tauri/target/release/bundle/` altında `.exe` + `.exe.sig` dosyaları oluşur.

### Yayınlama

1. GitHub Release oluşturun (tag: `v0.2.0`)
2. `Kukla_*_x64-setup.exe` ve `.sig` dosyasını yükleyin
3. `updater/latest.json.example` şablonunu kopyalayıp `latest.json` olarak düzenleyin:
   - `version`: yeni sürüm numarası
   - `platforms.windows-x86_64.url`: installer URL'si
   - `platforms.windows-x86_64.signature`: `.sig` dosyasının **içeriği** (yol değil)
4. `latest.json`'ı release'e `latest.json` adıyla ekleyin

Endpoint `tauri.conf.json` → `plugins.updater.endpoints` içinde yapılandırılır. GitHub kullanıyorsanız URL'yi kendi reponuza göre güncelleyin:

```
https://github.com/ekuragli/Kukla/releases/latest/download/latest.json
```

### GitHub Actions ile Otomatik Release

`v*` tag'i push edildiğinde `.github/workflows/release.yml` çalışır:

1. Test + clippy doğrulaması
2. İmzalı Windows build (NSIS `.exe` + `.sig`)
3. `latest.json` oluşturma ve yükleme
4. **Draft** GitHub Release oluşturma

**Gerekli repository secrets:**

| Secret | Açıklama |
|--------|----------|
| `TAURI_SIGNING_PRIVATE_KEY` | `kukla.key` dosyasının tam içeriği |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Anahtar şifresi (boşsa boş bırakın) |

**Repository ayarı:** Settings → Actions → General → Workflow permissions → **Read and write permissions**

**Release adımları:**

```bash
# Sürümü güncelle (package.json + tauri.conf.json)
git tag v0.2.0
git push origin v0.2.0
```

GitHub Actions tamamlandıktan sonra draft release'i inceleyip **Publish release** ile yayınlayın. Updater yalnızca yayınlanmış release'leri görür.

## Test ve Kalite

```bash
npm run test:rust      # Rust integration testleri (102 test)
npm run check:rust     # Clippy lint
npm run check          # Svelte şablon kontrolleri (svelte-check)
npm run build          # Frontend derleme

# Canlı ağ testleri (sunucu/API gerektirir, kapalıysa zarifçe atlanır):
cargo test --tests --manifest-path src-tauri/Cargo.toml -- --ignored
```

CI: GitHub Actions — `cargo test --tests`, `clippy`, `cargo audit`, `npm run build`

## Veri Konumu

Windows: `%APPDATA%\Kukla\`

| Dosya | Açıklama |
|-------|----------|
| `auth.json` | Şifre hash + şifreleme tuzu |
| `kukla.db.enc` | Şifreli SQLite metadata (kilitliyken) |
| `vectors.bin.enc` | Şifreli vector store (kilitliyken) |
| `audit.log` | Denetim kayıtları |

Sohbet verileri (`chat_sessions`, `chat_messages`) şifreli SQLite içinde tutulur;
kilitliyken `kukla.db.enc` ile birlikte şifrelenir. Tavily ve bulut sağlayıcı API
anahtarları aynı şifreli veritabanındaki `ai_secrets` tablosunda saklanır.

## Mimari

```
Svelte 5 (UI)  ←→  Tauri 2 (IPC)  ←→  Rust backend
                                         ├── Bedesten API
                                         ├── opencode serve (YZ özet / dilekçe)
                                         ├── Candle + all-MiniLM-L6-v2 (yerel embedding, 384 boyut)
                                         ├── SQLite + vector store
                                         └── ChaCha20 şifreleme
```

## Güvenlik Notları

- Tüm hassas API command'ları oturum doğrulaması gerektirir
- 15 dakika hareketsizlikte otomatik kilit
- Pencere kapatıldığında oturum açıksa veriler otomatik şifrelenir (kilit/çıkış)
- Şifre değiştirildiğinde veriler yeni anahtarla yeniden şifrelenir
- Sohbet asistanı "Yerel" modda çalışır: sorgu yalnızca kullanıcının kendi
  `opencode serve` sunucusuna ve yerel embedding modeline gönderilir
- "İnternet" modu yalnızca kullanıcı açıkça etkinleştirdiğinde çalışır; bu modda
  soru metni Tavily'ye gönderilir (ücretsiz katman: 1.000 arama/ay). Yüklenen
  belgeler hiçbir koşulda otomatik paylaşılmaz
- API anahtarları şifreli `kukla.db` içinde tutulur ve arayüze hiçbir zaman
  döndürülmez (yalnızca tanımlı mı + son 4 karakter gösterilir)
- Bulut sağlayıcıları yalnızca sohbet oturumunda "İnternet" modu açıkken
  kullanılır; kapalıyken sorular cihazdan hiç çıkmaz ve sohbet yerel
  `opencode serve` sunucusuyla çalışır
- Kişisel arşiv varsayılan kapalı (Ayarlar'dan açılır)
- Üretim build'de DevTools kapalı, CSP etkin

## MVP Sınırlamaları

- PDF export Windows sistem fontu (Arial/Segoe) kullanır; font bulunamazsa ASCII karakterlerle sınırlı olabilir
- LanceDB yerine in-memory vector store + bincode persist
- YZ özet, dilekçe ve sohbet için `opencode` kurulu olmalıdır; sunucuyu uygulama
  kendisi başlatır. Kurulu değilse bu özellikler pasif kalır (çevrimiçi/yerel arama
  ve raporlama çalışmaya devam eder)
- Çok büyük kararlarda (~500 sayfa+) indeksleme uzun sürebilir

## Lisans

MIT