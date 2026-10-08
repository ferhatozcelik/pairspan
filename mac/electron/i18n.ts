import { app } from 'electron';

const dict: Record<'en' | 'tr', Record<'phones' | 'code' | 'down' | 'open' | 'quit' | 'enabled' | 'disabled', string>> = {
  en: { phones: '{count} phone(s) connected', enabled: 'Enabled', disabled: 'Disabled', code: 'Code: {code}', down: 'Service unavailable', open: 'Open Pairspan', quit: 'Quit' },
  tr: { phones: '{count} telefon bağlı', enabled: 'Etkin', disabled: 'Devre dışı', code: 'Kod: {code}', down: 'Servise ulaşılamıyor', open: 'Pairspan’ı aç', quit: 'Çıkış' },
};

type Key = keyof typeof dict.en;

export function t(key: Key, vars: Record<string, string> = {}): string {
  const lang = /^tr/i.test(app.getLocale()) ? 'tr' : 'en';
  let text = dict[lang][key];
  for (const [name, value] of Object.entries(vars)) text = text.replace(`{${name}}`, value);
  return text;
}
