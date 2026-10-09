use crate::models::{PetitionTemplateInfo, PetitionType};

pub fn all_templates() -> Vec<PetitionTemplateInfo> {
    vec![
        PetitionTemplateInfo {
            id: PetitionType::Genel.as_str().to_string(),
            label: "Genel Dilekçe".to_string(),
            description: "Esnek yapı; çoğu duruma uyarlanabilir genel taslak.".to_string(),
        },
        PetitionTemplateInfo {
            id: PetitionType::Dava.as_str().to_string(),
            label: "Dava Dilekçesi".to_string(),
            description: "HMK'ya uygun dava açma dilekçesi (taraf, konu, olay, delil, hukuki sebepler, sonuç).".to_string(),
        },
        PetitionTemplateInfo {
            id: PetitionType::Cevap.as_str().to_string(),
            label: "Cevap Dilekçesi".to_string(),
            description: "Davaya karşı savunma; itirazlar, karşı deliller ve hukuki değerlendirme.".to_string(),
        },
        PetitionTemplateInfo {
            id: PetitionType::Itiraz.as_str().to_string(),
            label: "İtiraz Dilekçesi".to_string(),
            description: "Karar veya işleme karşı itiraz; usul ve esas yönünden gerekçeler.".to_string(),
        },
        PetitionTemplateInfo {
            id: PetitionType::Istinaf.as_str().to_string(),
            label: "İstinaf Dilekçesi".to_string(),
            description: "İlk derece mahkemesi kararına karşı istinaf başvurusu.".to_string(),
        },
        PetitionTemplateInfo {
            id: PetitionType::Temyiz.as_str().to_string(),
            label: "Temyiz Dilekçesi".to_string(),
            description: "Bölge adliye mahkemesi kararına karşı temyiz başvurusu.".to_string(),
        },
        PetitionTemplateInfo {
            id: PetitionType::DelilBildirme.as_str().to_string(),
            label: "Delil Bildirme".to_string(),
            description: "Delil listesi, tanık, bilirkişi ve keşif talepleri.".to_string(),
        },
    ]
}

pub fn parse_petition_type(raw: &str) -> Result<PetitionType, String> {
    match raw.trim().to_lowercase().as_str() {
        "genel" | "" => Ok(PetitionType::Genel),
        "dava" => Ok(PetitionType::Dava),
        "cevap" => Ok(PetitionType::Cevap),
        "itiraz" => Ok(PetitionType::Itiraz),
        "istinaf" => Ok(PetitionType::Istinaf),
        "temyiz" => Ok(PetitionType::Temyiz),
        "delil_bildirme" | "delil" => Ok(PetitionType::DelilBildirme),
        other => Err(format!(
            "Geçersiz dilekçe şablonu: {}. genel, dava, cevap, itiraz, istinaf, temyiz veya delil_bildirme kullanın.",
            other
        )),
    }
}

pub fn structure_instructions(petition_type: PetitionType) -> &'static str {
    match petition_type {
        PetitionType::Genel => {
            "GENEL DİLEKÇE yapısı kullan:\n\
             - Başlık ve mahkeme bilgisi [DOLDURULACAK]\n\
             - Davacı / Davalı / Vekil bilgileri [DOLDURULACAK]\n\
             - Konu\n\
             - Olayların özeti\n\
             - Hukuki dayanak ve içtihat atıfları\n\
             - Sonuç ve talep"
        }
        PetitionType::Dava => {
            "DAVA DİLEKÇESİ (HMK) yapısı kullan:\n\
             - Mahkeme başlığı [DOLDURULACAK]\n\
             - Davacı ve davalı kimlik/adres bilgileri [DOLDURULACAK]\n\
             - Vekil bilgileri [DOLDURULACAK]\n\
             - Dava konusu ve dava değeri [DOLDURULACAK]\n\
             - Olayların gelişimi (kronolojik)\n\
             - Hukuki sebepler (kanun maddeleri + içtihat atıfları)\n\
             - Deliller\n\
             - Sonuç ve talep (açık talep maddeleri)"
        }
        PetitionType::Cevap => {
            "CEVAP DİLEKÇESİ yapısı kullan:\n\
             - Mahkeme ve dosya bilgisi [DOLDURULACAK]\n\
             - Davalı / vekil bilgileri [DOLDURULACAK]\n\
             - Davacının iddialarına kısa atıf\n\
             - Olaylara ilişkin savunma\n\
             - Usul ve esas itirazları (varsa)\n\
             - Hukuki değerlendirme ve içtihat atıfları\n\
             - Karşı deliller\n\
             - Sonuç ve talep (davanın reddi vb.)"
        }
        PetitionType::Itiraz => {
            "İTİRAZ DİLEKÇESİ yapısı kullan:\n\
             - İlgili makam/mahkeme [DOLDURULACAK]\n\
             - İtiraz eden taraf bilgileri [DOLDURULACAK]\n\
             - İtiraz konusu karar/işlem [DOLDURULACAK]\n\
             - İtiraz gerekçeleri (usul ve esas)\n\
             - İçtihat destekli hukuki dayanak\n\
             - Sonuç ve talep (iptal/düzeltme/yeniden değerlendirme)"
        }
        PetitionType::Istinaf => {
            "İSTİNAF DİLEKÇESİ yapısı kullan:\n\
             - Bölge adliye mahkemesi başlığı [DOLDURULACAK]\n\
             - İstinaf eden taraf ve vekil [DOLDURULACAK]\n\
             - İlk derece mahkemesi ve karar bilgisi [DOLDURULACAK]\n\
             - İstinaf sebepleri (HMK m.355 kapsamında açık gerekçeler)\n\
             - Hukuki dayanak ve Yargıtay içtihat atıfları\n\
             - Sonuç ve talep (kararın kaldırılması/bozulması)"
        }
        PetitionType::Temyiz => {
            "TEMYİZ DİLEKÇESİ yapısı kullan:\n\
             - Yargıtay başlığı ve daire [DOLDURULACAK]\n\
             - Temyiz eden taraf ve vekil [DOLDURULACAK]\n\
             - Temyize konu karar bilgisi [DOLDURULACAK]\n\
             - Temyiz sebepleri (HUMK/HMK temyiz sınırları içinde)\n\
             - Hukuki dayanak ve içtihat atıfları\n\
             - Sonuç ve talep (ONANMA / BOZMA)"
        }
        PetitionType::DelilBildirme => {
            "DELİL BİLDİRME DİLEKÇESİ yapısı kullan:\n\
             - Mahkeme ve dosya bilgisi [DOLDURULACAK]\n\
             - Taraf bilgileri [DOLDURULACAK]\n\
             - Delil listesi (belge, tanık, bilirkişi, keşif vb.)\n\
             - Her delilin hangi vakıaya ilişkin olduğu\n\
             - İçtihat destekli hukuki gerekçe (delil değerlendirme ilkeleri)\n\
             - Sonuç ve talep"
        }
    }
}