export interface AppConfig {
  noteDir: string;
  gpgKeyId: string;
  gistId: string;
  tokenGpg: string; // Encrypted token ASCII armor
  tokenDecrypted?: string;
  configured: boolean;
}

export interface NoteFile {
  filename: string; // e.g. "2026.note.gpg"
  year?: string;
  isEncrypted: boolean;
  content: string; // raw content or ASCII armor
  decryptedContent?: string;
  lastModified: number;
}

export interface TerminalOutputLine {
  id: string;
  text: string;
  color?: 'green' | 'cyan' | 'yellow' | 'red' | 'gray' | 'white' | 'purple';
  isBold?: boolean;
}

export interface GistFileItem {
  filename: string;
  rawUrl?: string;
  size?: number;
  type?: string;
  truncated?: boolean;
  content?: string;
}

export interface RustStatus {
  rustAvailable: boolean;
  binaryPath: string;
  version: string;
  noteDir: string;
  keyId: string;
  gistId: string;
  hasToken: boolean;
  webState: 'active' | 'standby';
  noteFiles: Array<{ name: string; size: number; mtime: string }>;
  systemTime: string;
}

export interface KeyLedgerEntry {
  id: string;
  file_name: string;
  target_path: string;
  key_id: string;
  cipher_mode: string;
  s2k_iterations: number;
  layer: number;
  sha256: string;
  bytes_len: number;
  timestamp: string;
  notes: string;
}

export interface KeyLedger {
  version: string;
  system: string;
  updated_at: string;
  records: KeyLedgerEntry[];
}

export type WebEngineMode = 'active' | 'standby';

export interface DependencyDiagnostic {
  tsx: {
    status: string;
    name: string;
    description: string;
  };
  express: {
    status: string;
    name: string;
    description: string;
  };
  paths: {
    noteDir: string;
    defaultStandard: string;
    secretsDir: string;
    tokenFile: string;
    posixPermissions: string;
  };
  installCommands: {
    global: string;
    project: string;
  };
  securityPolicies: {
    gpgKeyLock: string;
    sshKeyRejection: string;
    s2kDefaultIterations: number;
    cleanSlateSupported: boolean;
  };
}
