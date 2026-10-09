/// Metni arama için parçalara böler (~2000 karakter ≈ 500 token).
const DEFAULT_CHUNK_SIZE: usize = 2000;
const DEFAULT_OVERLAP: usize = 200;

pub fn chunk_text(text: &str) -> Vec<String> {
    chunk_text_with_size(text, DEFAULT_CHUNK_SIZE, DEFAULT_OVERLAP)
}

pub fn chunk_text_with_size(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let char_count = trimmed.chars().count();
    if char_count <= chunk_size {
        return vec![trimmed.to_string()];
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;

    while start < char_count {
        let end = (start + chunk_size).min(char_count);
        let chunk: String = trimmed.chars().skip(start).take(end - start).collect();
        if !chunk.trim().is_empty() {
            chunks.push(chunk);
        }
        if end >= char_count {
            break;
        }
        start = end.saturating_sub(overlap);
    }

    chunks
}

