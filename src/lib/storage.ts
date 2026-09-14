import { AppConfig, NoteFile } from '../types';
import { encryptWithGpg, decryptWithGpg } from './crypto';

const STORAGE_KEYS = {
  CONFIG: 'cyber_forge_config',
  NOTES: 'cyber_forge_notes',
};

export const DEFAULT_CONFIG: AppConfig = {
  noteDir: '~/BOok/NOte',
  gpgKeyId: '9A4C73E1', // Default sample key fingerprint
  gistId: '',
  tokenGpg: '',
  tokenDecrypted: '',
  configured: false,
};

// Initial starter notes for 2026 if empty
const INITIAL_STARTER_NOTES = [
  '[2026-09-12 09:15:20] 🛡️ 靈感初鳴：Cyber-Forge 賽博靈感管家已成功部署。',
  '[2026-09-12 11:32:04] 💡 架構審查：全鏈路端到端公鑰加密，本地落盤即密文。',
  '[2026-09-12 14:05:48] 🚀 雲端聯動：GitHub Gist 增量補丁同步，年年獨立歸檔。',
  '[2026-09-12 16:40:12] 🔐 金鑰合約：Token 經 GPG 封裝為 token.gpg，杜絕明文洩漏。',
].join('\n');

export function loadAppConfig(): AppConfig {
  try {
    const raw = localStorage.getItem(STORAGE_KEYS.CONFIG);
    if (raw) {
      const parsed = JSON.parse(raw);
      return { ...DEFAULT_CONFIG, ...parsed };
    }
  } catch (e) {
    console.error('Failed to load config:', e);
  }
  return DEFAULT_CONFIG;
}

export function saveAppConfig(config: AppConfig): void {
  try {
    localStorage.setItem(STORAGE_KEYS.CONFIG, JSON.stringify(config));
  } catch (e) {
    console.error('Failed to save config:', e);
  }
}

export function loadAllNotes(): Record<string, NoteFile> {
  try {
    const raw = localStorage.getItem(STORAGE_KEYS.NOTES);
    if (raw) {
      return JSON.parse(raw);
    }
  } catch (e) {
    console.error('Failed to load notes:', e);
  }
  return {};
}

export function saveAllNotes(notes: Record<string, NoteFile>): void {
  try {
    localStorage.setItem(STORAGE_KEYS.NOTES, JSON.stringify(notes));
  } catch (e) {
    console.error('Failed to save notes:', e);
  }
}

/**
 * Initializes the default note files (e.g. current year 2026.note.gpg) if not existing.
 */
export async function initStorage(config: AppConfig): Promise<Record<string, NoteFile>> {
  const notes = loadAllNotes();
  const currentYear = new Date().getFullYear().toString();
  const defaultFilename = `${currentYear}.note.gpg`;

  if (!notes[defaultFilename]) {
    const encrypted = await encryptWithGpg(INITIAL_STARTER_NOTES, config.gpgKeyId);
    notes[defaultFilename] = {
      filename: defaultFilename,
      year: currentYear,
      isEncrypted: true,
      content: encrypted,
      decryptedContent: INITIAL_STARTER_NOTES,
      lastModified: Date.now(),
    };
    saveAllNotes(notes);
  }

  return notes;
}

/**
 * Read note content from storage.
 */
export function readNote(filename: string): NoteFile | null {
  const notes = loadAllNotes();
  return notes[filename] || null;
}

/**
 * Save note into storage.
 */
export function saveNote(note: NoteFile): void {
  const notes = loadAllNotes();
  notes[note.filename] = note;
  saveAllNotes(notes);
}

/**
 * Delete note file.
 */
export function deleteNote(filename: string): void {
  const notes = loadAllNotes();
  delete notes[filename];
  saveAllNotes(notes);
}
