/** Tauri invoke hatalarından okunabilir mesaj çıkarır. */
export function formatInvokeError(err) {
  if (typeof err === 'string') return err;
  if (err && typeof err === 'object') {
    if (typeof err.message === 'string') return err.message;
    if (typeof err.error === 'string') return err.error;
  }
  const text = String(err ?? '');
  return text === '[object Object]' ? '' : text;
}

/** Karar tarihini Türkçe formata çevirir (15.03.2022). */
export function formatDecisionDate(value) {
  if (value == null || value === '') return null;
  const s = String(value).trim();
  const iso = s.match(/^(\d{4})-(\d{2})-(\d{2})/);
  if (iso) return `${iso[3]}.${iso[2]}.${iso[1]}`;
  return s;
}

/** Backend (auth_set_password / auth_unlock) ile aynı şifre kuralları. */
export function validatePasswordRules(password) {
  const errors = [];
  if (password.length < 12) errors.push('Şifre en az 12 karakter olmalıdır.');
  if (!/[A-Z]/.test(password)) errors.push('Büyük harf içermelidir.');
  if (!/[a-z]/.test(password)) errors.push('Küçük harf içermelidir.');
  if (!/[0-9]/.test(password)) errors.push('Rakam içermelidir.');
  if (!/[^A-Za-z0-9]/.test(password)) errors.push('Özel karakter içermelidir.');
  return errors;
}

export function passwordRequirementsMet(password) {
  return validatePasswordRules(password).length === 0;
}

export function computeStrength(pw) {
  let score = 0;
  if (pw.length >= 12) score += 25;
  if (pw.length >= 16) score += 10;
  if (/[A-Z]/.test(pw)) score += 20;
  if (/[a-z]/.test(pw)) score += 20;
  if (/[0-9]/.test(pw)) score += 15;
  if (/[^A-Za-z0-9]/.test(pw)) score += 20;
  return Math.min(score, 100);
}

export function strengthLabel(score) {
  if (score < 40) return 'Zayıf';
  if (score < 70) return 'Orta';
  return 'Güçlü';
}

export function strengthColor(score) {
  if (score < 40) return 'var(--danger)';
  if (score < 70) return '#facc15';
  return '#4ade80';
}