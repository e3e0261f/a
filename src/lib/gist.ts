import { GistFileItem } from '../types';

export const GIST_BASE_API = 'https://api.github.com/gists';

export function extractCleanGistId(rawInput: string): string {
  return rawInput
    .trim()
    .replace(/\/+$/, '')
    .split('/')
    .pop()
    ?.trim() || rawInput.trim();
}

/**
 * List files in Gist.
 */
export async function listGistFiles(gistId: string, token: string): Promise<GistFileItem[]> {
  const cleanId = extractCleanGistId(gistId);
  if (!cleanId) {
    throw new Error('未指定有效 Gist ID');
  }

  const url = `${GIST_BASE_API}/${cleanId}`;
  const headers: Record<string, string> = {
    Accept: 'application/vnd.github+json',
    'X-GitHub-Api-Version': '2022-11-28',
  };

  if (token) {
    headers.Authorization = `Bearer ${token.trim()}`;
  }

  const res = await fetch(url, { headers });
  if (!res.ok) {
    if (res.status === 404) {
      throw new Error(`找不到 Gist ID: ${cleanId} (404 Not Found)`);
    }
    if (res.status === 401) {
      throw new Error('GitHub Token 授權失敗或權限不足 (401 Unauthorized)');
    }
    throw new Error(`獲取 Gist 清單失敗，伺服器狀態碼: ${res.status}`);
  }

  const data = await res.json();
  const files: Record<string, any> = data.files || {};
  
  return Object.keys(files).map((filename) => ({
    filename,
    rawUrl: files[filename]?.raw_url,
    size: files[filename]?.size,
    type: files[filename]?.type,
    truncated: files[filename]?.truncated,
    content: files[filename]?.content,
  }));
}

/**
 * Fetch a single file from Gist.
 */
export async function fetchFromGist(
  gistId: string,
  filename: string,
  token: string
): Promise<string> {
  const cleanId = extractCleanGistId(gistId);
  if (!cleanId) {
    throw new Error('未指定有效 Gist ID');
  }

  const url = `${GIST_BASE_API}/${cleanId}`;
  const headers: Record<string, string> = {
    Accept: 'application/vnd.github+json',
    'X-GitHub-Api-Version': '2022-11-28',
  };

  if (token) {
    headers.Authorization = `Bearer ${token.trim()}`;
  }

  const res = await fetch(url, { headers });
  if (!res.ok) {
    throw new Error(`雲端獲取失敗，狀態碼: ${res.status}`);
  }

  const data = await res.json();
  const fileObj = data.files?.[filename];

  if (!fileObj) {
    throw new Error(`在雲端 Gist 倉庫中找不到檔案: ${filename}`);
  }

  // If content is truncated, fetch directly from raw_url
  if (fileObj.truncated && fileObj.raw_url) {
    const rawRes = await fetch(fileObj.raw_url);
    if (rawRes.ok) {
      return await rawRes.text();
    }
  }

  return fileObj.content || '';
}

/**
 * Push / Update file on GitHub Gist using PATCH.
 */
export async function syncToGist(
  gistId: string,
  filename: string,
  content: string,
  token: string
): Promise<void> {
  const cleanId = extractCleanGistId(gistId);
  if (!cleanId) {
    throw new Error('未指定有效 Gist ID');
  }
  if (!token) {
    throw new Error('同步到 Gist 必須提供具備 gist 權限的 GitHub Personal Access Token');
  }

  const url = `${GIST_BASE_API}/${cleanId}`;
  const headers: Record<string, string> = {
    Authorization: `Bearer ${token.trim()}`,
    Accept: 'application/vnd.github+json',
    'Content-Type': 'application/json',
    'X-GitHub-Api-Version': '2022-11-28',
  };

  const body = JSON.stringify({
    description: 'Cyber-Forge 賽博靈感管家 自動雲端加密備份法典',
    files: {
      [filename]: {
        content,
      },
    },
  });

  const res = await fetch(url, {
    method: 'PATCH',
    headers,
    body,
  });

  if (!res.ok) {
    const errText = await res.text().catch(() => '');
    throw new Error(`雲端拒絕更新，狀態碼: ${res.status} ${errText ? `(${errText})` : ''}`);
  }
}

/**
 * 🔄 原子替換/套殼 Gist 檔案：
 * 上傳 newFilename (含內容)，並可選將 oldFilename 設為 null 以在遠端完全刪除原始檔案
 */
export async function atomicReplaceGistFile(
  gistId: string,
  oldFilename: string | null,
  newFilename: string,
  newContent: string,
  token: string
): Promise<void> {
  const cleanId = extractCleanGistId(gistId);
  if (!cleanId) {
    throw new Error('未指定有效 Gist ID');
  }
  if (!token) {
    throw new Error('操作 Gist 必須提供 GitHub Personal Access Token');
  }

  const url = `${GIST_BASE_API}/${cleanId}`;
  const headers: Record<string, string> = {
    Authorization: `Bearer ${token.trim()}`,
    Accept: 'application/vnd.github+json',
    'Content-Type': 'application/json',
    'X-GitHub-Api-Version': '2022-11-28',
  };

  const filesPayload: Record<string, any> = {
    [newFilename]: {
      content: newContent,
    },
  };

  // 在 GitHub Gist REST API 中，若欲刪除某檔案，將其值指定為 null
  if (oldFilename && oldFilename !== newFilename) {
    filesPayload[oldFilename] = null;
  }

  const res = await fetch(url, {
    method: 'PATCH',
    headers,
    body: JSON.stringify({
      description: 'Cyber-NOte 遠端檔案加密套殼與原子替換',
      files: filesPayload,
    }),
  });

  if (!res.ok) {
    const errText = await res.text().catch(() => '');
    throw new Error(`遠端原子替換失敗，狀態碼: ${res.status} ${errText ? `(${errText})` : ''}`);
  }
}

/**
 * 🗑️ 刪除 Gist 中的指定檔案
 */
export async function deleteFromGist(
  gistId: string,
  filename: string,
  token: string
): Promise<void> {
  const cleanId = extractCleanGistId(gistId);
  if (!cleanId) {
    throw new Error('未指定有效 Gist ID');
  }
  if (!token) {
    throw new Error('刪除 Gist 檔案必須提供 GitHub Personal Access Token');
  }

  const url = `${GIST_BASE_API}/${cleanId}`;
  const headers: Record<string, string> = {
    Authorization: `Bearer ${token.trim()}`,
    Accept: 'application/vnd.github+json',
    'Content-Type': 'application/json',
    'X-GitHub-Api-Version': '2022-11-28',
  };

  const res = await fetch(url, {
    method: 'PATCH',
    headers,
    body: JSON.stringify({
      files: {
        [filename]: null,
      },
    }),
  });

  if (!res.ok) {
    const errText = await res.text().catch(() => '');
    throw new Error(`刪除遠端檔案失敗，狀態碼: ${res.status} ${errText ? `(${errText})` : ''}`);
  }
}
