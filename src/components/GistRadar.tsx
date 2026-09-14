import React, { useState, useEffect } from 'react';
import {
  Cloud,
  RefreshCw,
  Download,
  Unlock,
  Upload,
  AlertTriangle,
  FileCode,
  CheckCircle2,
  ExternalLink,
  Shield,
  Eye,
  Trash2,
  History,
  Sparkles,
  Lock,
  Key,
  Check,
} from 'lucide-react';
import { AppConfig, GistFileItem, NoteFile } from '../types';
import { listGistFiles, fetchFromGist, syncToGist, atomicReplaceGistFile } from '../lib/gist';
import { decryptWithGpg, encryptWithGpg } from '../lib/crypto';
import { saveNote, loadAllNotes } from '../lib/storage';

interface GistRadarProps {
  config: AppConfig;
  onNotesChange: (notes: Record<string, NoteFile>) => void;
  onOpenWizard: () => void;
}

export const GistRadar: React.FC<GistRadarProps> = ({
  config,
  onNotesChange,
  onOpenWizard,
}) => {
  const [files, setFiles] = useState<GistFileItem[]>([]);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [errorMsg, setErrorMsg] = useState<string>('');
  const [actionNotice, setActionNotice] = useState<string>('');

  // Remote file viewer modal/drawer state
  const [previewFilename, setPreviewFilename] = useState<string>('');
  const [previewContent, setPreviewContent] = useState<string>('');
  const [previewDecrypted, setPreviewDecrypted] = useState<string>('');
  const [isPreviewLoading, setIsPreviewLoading] = useState<boolean>(false);

  // Upload modal state
  const [uploadFileName, setUploadFileName] = useState<string>('');
  const [uploadContent, setUploadContent] = useState<string>('');
  const [uploadIsRaw, setUploadIsRaw] = useState<boolean>(false);
  const [isUploading, setIsUploading] = useState<boolean>(false);

  // 🚀 Clean Slate Migration State (抹除歷史/新建倉庫)
  const [showMigrateModal, setShowMigrateModal] = useState<boolean>(false);
  const [deleteOldRepo, setDeleteOldRepo] = useState<boolean>(true);
  const [isMigrating, setIsMigrating] = useState<boolean>(false);
  const [migrateResult, setMigrateResult] = useState<string | null>(null);

  // 🛡️ 套殼加密彈窗狀態 (In-Place Remote Encapsulate)
  const [showEncapsulateModal, setShowEncapsulateModal] = useState<boolean>(false);
  const [encapsulateTarget, setEncapsulateTarget] = useState<string>('');
  const [encapsulateMode, setEncapsulateMode] = useState<'gpg' | 'symmetric'>('gpg');
  const [encapsulatePassphrase, setEncapsulatePassphrase] = useState<string>('');
  const [encapsulateIterations, setEncapsulateIterations] = useState<number>(65011712);
  const [encapsulateDeleteOriginal, setEncapsulateDeleteOriginal] = useState<boolean>(true);
  const [isEncapsulating, setIsEncapsulating] = useState<boolean>(false);
  const [encapsulateError, setEncapsulateError] = useState<string>('');
  const [encapsulateResult, setEncapsulateResult] = useState<string | null>(null);

  // 🔓 解密還原彈窗狀態 (Decapsulate / Decrypt Modal)
  const [showDecapsulateModal, setShowDecapsulateModal] = useState<boolean>(false);
  const [decapsulateTarget, setDecapsulateTarget] = useState<string>('');
  const [decapsulatePassphrase, setDecapsulatePassphrase] = useState<string>('');
  const [decapsulateRestoreRemote, setDecapsulateRestoreRemote] = useState<boolean>(false);
  const [decapsulateDeleteRemoteGpg, setDecapsulateDeleteRemoteGpg] = useState<boolean>(false);
  const [isDecapsulating, setIsDecapsulating] = useState<boolean>(false);
  const [decapsulateError, setDecapsulateError] = useState<string>('');
  const [decapsulateResult, setDecapsulateResult] = useState<string | null>(null);
  const [decapsulatedContentPreview, setDecapsulatedContentPreview] = useState<string>('');

  const fetchFiles = async () => {
    if (!config.gistId) {
      setErrorMsg('未配置 GitHub Gist ID，請先執行初始化設定。');
      return;
    }
    setIsLoading(true);
    setErrorMsg('');
    try {
      const items = await listGistFiles(config.gistId, config.tokenDecrypted || '');
      setFiles(items);
    } catch (e) {
      setErrorMsg(e instanceof Error ? e.message : '獲取雲端清單失敗');
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    if (config.gistId) {
      fetchFiles();
    }
  }, [config.gistId, config.tokenDecrypted]);

  const flashNotice = (msg: string) => {
    setActionNotice(msg);
    setTimeout(() => setActionNotice(''), 4000);
  };

  // Download file from Gist (支援 -o 自訂檔名與加 ./ 下載到本地)
  const handleDownload = async (filename: string, decrypt: boolean) => {
    if (!config.gistId) return;

    const defaultOut = `./${decrypt && filename.endsWith('.gpg') ? filename.replace(/\.gpg$/, '') : filename}`;
    const customOut = window.prompt(
      `請輸入下載目標檔名或本地存放路徑 (-o 參數，加 ./ 下載至本地工作目錄):\n\n範例: ./1.txt 或 ${defaultOut}`,
      defaultOut
    );
    if (customOut === null) return;

    setIsLoading(true);
    try {
      const remoteRaw = await fetchFromGist(
        config.gistId,
        filename,
        config.tokenDecrypted || ''
      );

      let savedFilename = filename;
      let finalContent = remoteRaw;
      let finalDecrypted: string | undefined = undefined;

      if (decrypt && filename.endsWith('.gpg')) {
        savedFilename = filename.replace(/\.gpg$/, '');
        finalDecrypted = await decryptWithGpg(remoteRaw, config.gpgKeyId);
        finalContent = finalDecrypted;
      }

      const targetPath = customOut.trim() || defaultOut;

      try {
        await fetch('/api/notes/save-local', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            filePath: targetPath,
            content: finalContent,
          }),
        });
      } catch (err) {
        console.warn('寫入本地檔案失敗:', err);
      }

      const noteFile: NoteFile = {
        filename: targetPath.replace(/^\.\//, ''),
        year: /^\d{4}/.exec(savedFilename)?.[0],
        isEncrypted: !decrypt && remoteRaw.includes('-----BEGIN PGP MESSAGE-----'),
        content: remoteRaw,
        decryptedContent: finalDecrypted,
        lastModified: Date.now(),
      };

      saveNote(noteFile);
      onNotesChange(loadAllNotes());
      flashNotice(
        `✨ 檔案已成功下載至: ${targetPath} (大小: ${(finalContent.length / 1024).toFixed(2)} KB)${
          decrypt ? ' [已完成私鑰解密還原]' : ''
        }`
      );
    } catch (e) {
      alert(`下載失敗: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setIsLoading(false);
    }
  };

  // Download ALL files from Gist (a -d --all)
  const handleDownloadAll = async () => {
    if (!config.gistId || files.length === 0) return;
    const confirmDownload = window.confirm(
      `確定要批量下載雲端 Gist 倉庫所有檔案（共 ${files.length} 個檔案）至本地嗎？(a -d --all)`
    );
    if (!confirmDownload) return;

    setIsLoading(true);
    let successCount = 0;
    try {
      const res = await fetch('/api/gist/download-all', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          gistId: config.gistId,
          token: config.tokenDecrypted || '',
          outputDir: './',
          decrypt: false,
        }),
      });

      if (res.ok) {
        const data = await res.json();
        successCount = data.count || files.length;
        if (data.files) {
          for (const f of data.files) {
            const noteFile: NoteFile = {
              filename: f.filename,
              year: /^\d{4}/.exec(f.filename)?.[0],
              isEncrypted: !f.isDecrypted,
              content: '',
              lastModified: Date.now(),
            };
            saveNote(noteFile);
          }
        }
      } else {
        for (const file of files) {
          try {
            const remoteRaw = await fetchFromGist(
              config.gistId,
              file.filename,
              config.tokenDecrypted || ''
            );
            await fetch('/api/notes/save-local', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({
                filePath: `./${file.filename}`,
                content: remoteRaw,
              }),
            });
            const noteFile: NoteFile = {
              filename: file.filename,
              year: /^\d{4}/.exec(file.filename)?.[0],
              isEncrypted: remoteRaw.includes('-----BEGIN PGP MESSAGE-----'),
              content: remoteRaw,
              lastModified: Date.now(),
            };
            saveNote(noteFile);
            successCount++;
          } catch {}
        }
      }

      onNotesChange(loadAllNotes());
      flashNotice(`✨ [a -d --all] 全部下載完成！共成功下載 ${successCount}/${files.length} 個檔案至本地工作目錄。`);
    } catch (e) {
      alert(`批量下載失敗: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setIsLoading(false);
    }
  };

  // Preview remote file
  const handlePreview = async (filename: string) => {
    if (!config.gistId) return;
    setPreviewFilename(filename);
    setIsPreviewLoading(true);
    setPreviewContent('');
    setPreviewDecrypted('');

    try {
      const raw = await fetchFromGist(config.gistId, filename, config.tokenDecrypted || '');
      setPreviewContent(raw);

      if (raw.includes('-----BEGIN PGP MESSAGE-----')) {
        try {
          const dec = await decryptWithGpg(raw, config.gpgKeyId);
          setPreviewDecrypted(dec);
        } catch {
          setPreviewDecrypted('（解密失敗，可能需要不同的 GPG 密鑰）');
        }
      } else {
        setPreviewDecrypted(raw);
      }
    } catch (e) {
      alert(`獲取預覽失敗: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setIsPreviewLoading(false);
    }
  };

  // Upload file to Gist
  const handleUploadSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!uploadFileName.trim() || !uploadContent.trim() || !config.gistId) return;

    setIsUploading(true);
    try {
      let finalContent = uploadContent;
      let targetName = uploadFileName.trim();

      if (!uploadIsRaw) {
        finalContent = await encryptWithGpg(uploadContent, config.gpgKeyId);
        if (!targetName.endsWith('.gpg')) {
          targetName = `${targetName}.gpg`;
        }
      }

      await syncToGist(
        config.gistId,
        targetName,
        finalContent,
        config.tokenDecrypted || ''
      );

      flashNotice(`☁️ 檔案【${targetName}】已成功推送至 GitHub Gist 雲端！`);
      setUploadFileName('');
      setUploadContent('');
      fetchFiles();
    } catch (e) {
      alert(`上傳失敗: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setIsUploading(false);
    }
  };

  // 🚀 執行抹除歷史與遷移至全新倉庫 (a --migrate-repo [--delete-old])
  const handleMigrateRepo = async () => {
    if (!config.tokenDecrypted) {
      alert('未檢測到有效 GitHub Token 憑證，請先透過配置精靈配置。');
      return;
    }

    setIsMigrating(true);
    setMigrateResult(null);

    try {
      const res = await fetch('/api/gist/migrate-repo', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          token: config.tokenDecrypted,
          deleteOld: deleteOldRepo,
        }),
      });

      const data = await res.json();
      if (!res.ok) {
        throw new Error(data.error || '倉庫遷移失敗');
      }

      setMigrateResult(
        `✨ 遷移成功！新倉庫 ID: ${data.newGistId} (已轉移 ${data.transferredFilesCount} 個檔案，舊倉庫${
          data.oldDeleted ? '已徹底銷毀' : '已保留'
        })`
      );
      flashNotice('✨ 全新倉庫建立完成！所有歷史修訂與 Git 提交記錄已徹底切斷抹除。');
      // 自動更新 Gist ID
      config.gistId = data.newGistId;
      setTimeout(() => {
        fetchFiles();
      }, 1500);
    } catch (err: any) {
      setMigrateResult(`❌ 遷移失敗: ${err.message}`);
    } finally {
      setIsMigrating(false);
    }
  };

  // 🛡️ 開啟遠端套殼加密視窗
  const openEncapsulateModal = (filename: string) => {
    setEncapsulateTarget(filename);
    setEncapsulateMode(config.gpgKeyId ? 'gpg' : 'symmetric');
    setEncapsulatePassphrase('');
    setEncapsulateIterations(65011712);
    setEncapsulateDeleteOriginal(true);
    setEncapsulateError('');
    setEncapsulateResult(null);
    setShowEncapsulateModal(true);
  };

  // 執行遠端套殼加密
  const handleExecuteEncapsulate = async () => {
    if (!encapsulateTarget) return;
    setIsEncapsulating(true);
    setEncapsulateError('');
    setEncapsulateResult(null);

    try {
      const res = await fetch('/api/gist/encapsulate-file', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          filename: encapsulateTarget,
          deleteOriginal: encapsulateDeleteOriginal,
          encryptMode: encapsulateMode,
          passphrase: encapsulatePassphrase,
          iterations: encapsulateIterations,
          token: config.tokenDecrypted,
          gistId: config.gistId,
        }),
      });

      const data = await res.json();
      if (!res.ok) {
        throw new Error(data.error || '套殼加密失敗');
      }

      setEncapsulateResult(data.message || '套殼加密完成！');
      flashNotice(
        data.message ||
          `🔒 已將「${encapsulateTarget}」套上 GPG 密文殼${
            encapsulateDeleteOriginal ? '，原明文已於遠端徹底刪除' : ''
          }！`
      );
      // 重新整理遠端檔案清單
      setTimeout(() => {
        fetchFiles();
      }, 1000);
    } catch (err: any) {
      setEncapsulateError(err.message || '套殼加密過程中發生錯誤');
    } finally {
      setIsEncapsulating(false);
    }
  };

  // 🔓 開啟解密視窗
  const openDecapsulateModal = (filename: string) => {
    setDecapsulateTarget(filename);
    setDecapsulatePassphrase('');
    setDecapsulateRestoreRemote(false); // 預設安全：絕對不把遠端解密後上傳明文
    setDecapsulateDeleteRemoteGpg(false);
    setDecapsulateError('');
    setDecapsulateResult(null);
    setDecapsulatedContentPreview('');
    setShowDecapsulateModal(true);
  };

  // 執行解密
  const handleExecuteDecapsulate = async () => {
    if (!decapsulateTarget) return;
    setIsDecapsulating(true);
    setDecapsulateError('');
    setDecapsulateResult(null);

    try {
      const res = await fetch('/api/gist/decapsulate-file', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          filename: decapsulateTarget,
          passphrase: decapsulatePassphrase,
          restoreRemotePlaintext: decapsulateRestoreRemote,
          deleteEncryptedRemote: decapsulateDeleteRemoteGpg,
          token: config.tokenDecrypted,
          gistId: config.gistId,
        }),
      });

      const data = await res.json();
      if (!res.ok) {
        throw new Error(data.error || '解密失敗');
      }

      const outputName =
        data.outputPlaintextName ||
        (decapsulateTarget.endsWith('.gpg')
          ? decapsulateTarget.slice(0, -4)
          : decapsulateTarget);

      // 本地入庫
      const noteFile: NoteFile = {
        filename: outputName,
        year: /^\d{4}/.exec(outputName)?.[0],
        isEncrypted: false,
        content: data.plaintext,
        decryptedContent: data.plaintext,
        lastModified: Date.now(),
      };
      saveNote(noteFile);
      onNotesChange(loadAllNotes());

      setDecapsulatedContentPreview(data.plaintext);
      setDecapsulateResult(data.message || '解密成功！');
      flashNotice(
        data.message ||
          `🔓 檔案「${outputName}」已在本地解密入庫！${
            decapsulateRestoreRemote ? '（遠端已同步更新明文）' : '（遠端維持密文保護）'
          }`
      );

      if (decapsulateRestoreRemote) {
        setTimeout(() => {
          fetchFiles();
        }, 1000);
      }
    } catch (err: any) {
      setDecapsulateError(err.message || '解密過程中發生錯誤');
    } finally {
      setIsDecapsulating(false);
    }
  };

  return (
    <div className="space-y-6">
      {/* Header Info */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 p-4 bg-[#12161f] border border-gray-800 rounded-xl">
        <div>
          <div className="flex items-center gap-2">
            <Cloud className="w-5 h-5 text-cyan-400" />
            <h2 className="text-base font-semibold text-gray-100 font-mono">
              GitHub Gist 雲端雷達 (a -l / a -s / a -d)
            </h2>
          </div>
          <p className="text-xs text-gray-400 font-mono mt-1">
            目標 Gist ID:{' '}
            <span className="text-cyan-300 font-bold">{config.gistId || '（未配置）'}</span>
            {config.gistId && (
              <a
                href={`https://gist.github.com/${config.gistId}`}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-0.5 ml-2 text-cyan-400 hover:underline"
              >
                <span>在 GitHub 查看</span>
                <ExternalLink className="w-3 h-3" />
              </a>
            )}
          </p>
        </div>

        <div className="flex items-center gap-2">
          {/* Button: 創建新倉庫/轉移當前倉庫/抹除歷史記錄 */}
          <button
            id="open-migrate-modal-btn"
            onClick={() => {
              setShowMigrateModal(true);
              setMigrateResult(null);
            }}
            className="px-3 py-1.5 bg-rose-950/70 hover:bg-rose-900/80 text-rose-300 text-xs font-mono rounded-lg border border-rose-700/50 transition flex items-center gap-1.5 shadow-sm"
            title="創建新倉庫並轉移當前檔案，以完全抹除歷史修訂 (a --migrate-repo)"
          >
            <History className="w-3.5 h-3.5 text-rose-400" />
            <span>抹除歷史 / 新建倉庫</span>
          </button>

          {/* Button: 批量下載全部 (a -d --all) */}
          <button
            id="download-all-files-btn"
            onClick={handleDownloadAll}
            disabled={isLoading || !config.gistId || files.length === 0}
            className="px-3 py-1.5 bg-emerald-950/70 hover:bg-emerald-900/80 text-emerald-300 text-xs font-mono rounded-lg border border-emerald-700/50 transition flex items-center gap-1.5 disabled:opacity-40 shadow-sm"
            title="批量下載 Gist 倉庫所有檔案至本地 (a -d --all)"
          >
            <Download className="w-3.5 h-3.5 text-emerald-400" />
            <span>下載全部 (a -d --all)</span>
          </button>

          <button
            onClick={fetchFiles}
            disabled={isLoading || !config.gistId}
            className="px-3 py-1.5 bg-[#1a202c] hover:bg-[#252d3d] text-gray-200 text-xs font-mono rounded-lg border border-gray-700 transition flex items-center gap-1.5 disabled:opacity-40"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isLoading ? 'animate-spin' : ''}`} />
            <span>掃描雲端物資 (a -l)</span>
          </button>
        </div>
      </div>

      {actionNotice && (
        <div className="p-3 bg-emerald-950/50 border border-emerald-500/40 text-emerald-300 text-xs rounded-xl flex items-center gap-2 font-mono">
          <CheckCircle2 className="w-4 h-4 text-emerald-400 flex-shrink-0" />
          <span>{actionNotice}</span>
        </div>
      )}

      {errorMsg && (
        <div className="p-4 bg-amber-950/40 border border-amber-500/40 rounded-xl text-xs font-mono text-amber-200 flex items-start gap-3">
          <AlertTriangle className="w-5 h-5 text-amber-400 flex-shrink-0 mt-0.5" />
          <div className="space-y-2 flex-1">
            <p className="font-semibold">{errorMsg}</p>
            {!config.gistId && (
              <button
                onClick={onOpenWizard}
                className="px-3 py-1 bg-amber-600 hover:bg-amber-500 text-black font-semibold rounded text-xs transition"
              >
                啟動引導精靈配置 Gist ID
              </button>
            )}
          </div>
        </div>
      )}

      {/* Grid: File List & Upload Form */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Left 2 Cols: Cloud Files List */}
        <div className="lg:col-span-2 bg-[#090d14] border border-gray-800 rounded-xl overflow-hidden shadow-xl">
          <div className="px-4 py-3 bg-[#131722] border-b border-gray-800 flex items-center justify-between text-xs font-mono">
            <span className="text-gray-300 font-semibold flex items-center gap-2">
              <FileCode className="w-4 h-4 text-cyan-400" />
              雲端現有密文包裹清單 ({files.length} 件)
            </span>
            <span className="text-gray-500">支援 a -d 精準引渡</span>
          </div>

          {isLoading ? (
            <div className="p-12 text-center text-cyan-400 text-sm font-mono animate-pulse">
              📡 [雲端雷達] 正在掃描 GitHub Gist 倉庫物資清單...
            </div>
          ) : files.length === 0 ? (
            <div className="p-12 text-center text-gray-500 text-sm font-mono space-y-2">
              <p>雲端 Gist 倉庫暫無任何檔案，或尚未配置正確 Gist ID。</p>
              <p className="text-xs text-gray-600">
                可使用右側面板或終端指令 <code>a -s</code> 將本地密文包裹同步出海。
              </p>
            </div>
          ) : (
            <div className="divide-y divide-gray-900/80">
              {files.map((file) => {
                const isGpg = file.filename.endsWith('.gpg');
                return (
                  <div
                    key={file.filename}
                    className="p-3.5 hover:bg-[#111622] transition flex flex-col sm:flex-row sm:items-center justify-between gap-3 font-mono text-xs"
                  >
                    <div className="flex items-center gap-2.5 min-w-0">
                      <div
                        className={`p-2 rounded-lg ${
                          isGpg
                            ? 'bg-cyan-950/60 text-cyan-400 border border-cyan-800/40'
                            : 'bg-amber-950/60 text-amber-400 border border-amber-800/40'
                        }`}
                      >
                        {isGpg ? <Shield className="w-4 h-4" /> : <Lock className="w-4 h-4" />}
                      </div>
                      <div className="min-w-0">
                        <div className="text-gray-200 font-semibold truncate flex items-center gap-2">
                          <span>{file.filename}</span>
                          {isGpg ? (
                            <span className="px-1.5 py-0.2 bg-cyan-950 text-cyan-400 border border-cyan-700/50 rounded text-[10px]">
                              🔒 GPG 密文
                            </span>
                          ) : (
                            <span className="px-1.5 py-0.2 bg-amber-950/80 text-amber-300 border border-amber-700/60 rounded text-[10px]">
                              📄 原始明文
                            </span>
                          )}
                        </div>
                        <div className="text-gray-500 text-[11px]">
                          大小: {((file.size || 0) / 1024).toFixed(2)} KB
                        </div>
                      </div>
                    </div>

                    <div className="flex items-center gap-1.5 flex-shrink-0">
                      {/* 預覽 */}
                      <button
                        onClick={() => handlePreview(file.filename)}
                        className="px-2.5 py-1.5 bg-[#1a202c] hover:bg-[#262e3f] text-gray-300 rounded text-xs transition flex items-center gap-1"
                        title="預覽雲端檔案內容"
                      >
                        <Eye className="w-3.5 h-3.5" />
                        <span>預覽</span>
                      </button>

                      {/* 🛡️ 套殼加密按鈕 (若為明文檔強烈推薦，可將遠端明文加密並銷毀原檔) */}
                      {!isGpg ? (
                        <button
                          onClick={() => openEncapsulateModal(file.filename)}
                          className="px-3 py-1.5 bg-amber-500 hover:bg-amber-400 text-black font-bold rounded text-xs transition flex items-center gap-1.5 shadow-sm shadow-amber-950/60 border border-amber-300"
                          title="在遠端套上一層 GPG 加密殼，並銷毀原始明文檔案"
                        >
                          <Lock className="w-3.5 h-3.5" />
                          <span>套殼加密</span>
                        </button>
                      ) : (
                        <button
                          onClick={() => openEncapsulateModal(file.filename)}
                          className="px-2.5 py-1.5 bg-purple-950/60 hover:bg-purple-900/80 text-purple-300 border border-purple-800/60 rounded text-xs transition flex items-center gap-1"
                          title="為現有密文再加套一層 GPG 加密外殼 (多層防禦)"
                        >
                          <Lock className="w-3.5 h-3.5" />
                          <span>加套密殼</span>
                        </button>
                      )}

                      {/* 🔓 解密還原/下載 */}
                      {isGpg && (
                        <button
                          onClick={() => openDecapsulateModal(file.filename)}
                          className="px-2.5 py-1.5 bg-emerald-950/60 hover:bg-emerald-900/80 text-emerald-300 border border-emerald-800/60 rounded text-xs transition flex items-center gap-1"
                          title="解密查閱 (支援僅本地解密，或選擇性遠端還原明文)"
                        >
                          <Unlock className="w-3.5 h-3.5" />
                          <span>解密下載 (-x)</span>
                        </button>
                      )}

                      {/* 📥 檔案下載 */}
                      <button
                        onClick={() => handleDownload(file.filename, false)}
                        className="px-2.5 py-1.5 bg-cyan-950/60 hover:bg-cyan-900/80 text-cyan-300 border border-cyan-800/60 rounded text-xs transition flex items-center gap-1"
                        title="下載檔案副本至本地"
                      >
                        <Download className="w-3.5 h-3.5" />
                        <span>{isGpg ? '密文下載' : '下載檔案'}</span>
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Right Col: Encrypted / Raw File Dispatch Panel */}
        <div className="bg-[#090d14] border border-gray-800 rounded-xl p-4 space-y-4">
          <div className="border-b border-gray-800 pb-2">
            <h3 className="text-sm font-semibold text-gray-200 font-mono flex items-center gap-2">
              <Upload className="w-4 h-4 text-cyan-400" />
              雲端發射台 (a -s 外部檔案)
            </h3>
            <p className="text-[11px] text-gray-500 font-mono mt-1">
              以 GPG 公鑰封裝外部檔案並推送至 Gist
            </p>
          </div>

          <form onSubmit={handleUploadSubmit} className="space-y-3 font-mono text-xs">
            <div>
              <label className="block text-gray-400 mb-1">雲端檔名:</label>
              <input
                type="text"
                value={uploadFileName}
                onChange={(e) => setUploadFileName(e.target.value)}
                placeholder="例如: secret.kdbx 或 diary.md"
                required
                className="w-full px-3 py-2 bg-[#121620] border border-gray-800 rounded focus:border-cyan-500 text-gray-100 placeholder-gray-600 focus:outline-none"
              />
            </div>

            <div>
              <label className="block text-gray-400 mb-1">檔案內容 / Payload:</label>
              <textarea
                rows={4}
                value={uploadContent}
                onChange={(e) => setUploadContent(e.target.value)}
                placeholder="輸入欲發射之文字或密碼庫內容..."
                required
                className="w-full px-3 py-2 bg-[#121620] border border-gray-800 rounded focus:border-cyan-500 text-gray-100 placeholder-gray-600 focus:outline-none resize-y"
              />
            </div>

            <div className="flex items-center gap-2 pt-1">
              <input
                type="checkbox"
                id="raw-upload-checkbox"
                checked={uploadIsRaw}
                onChange={(e) => setUploadIsRaw(e.target.checked)}
                className="rounded bg-[#121620] border-gray-700 text-cyan-500 focus:ring-0"
              />
              <label htmlFor="raw-upload-checkbox" className="text-gray-300 text-xs cursor-pointer">
                明文模式 (--raw / -u，不進行 GPG 加密)
              </label>
            </div>

            <button
              type="submit"
              disabled={isUploading || !config.gistId}
              className="w-full py-2 bg-cyan-600 hover:bg-cyan-500 disabled:opacity-40 text-white font-medium rounded-lg transition flex items-center justify-center gap-1.5 shadow-md"
            >
              <Upload className="w-3.5 h-3.5" />
              <span>{isUploading ? '正在發射...' : '發射至 Gist (a -s)'}</span>
            </button>
          </form>
        </div>
      </div>

      {/* Preview Modal */}
      {previewFilename && (
        <div className="fixed inset-0 z-50 bg-black/75 flex items-center justify-center p-4">
          <div className="bg-[#0e131d] border border-cyan-800/60 rounded-xl max-w-2xl w-full max-h-[80vh] flex flex-col overflow-hidden shadow-2xl">
            <div className="px-4 py-3 bg-[#151b27] border-b border-gray-800 flex items-center justify-between text-xs font-mono">
              <span className="text-cyan-400 font-semibold flex items-center gap-1.5">
                <Eye className="w-4 h-4" />
                預覽雲端檔案: {previewFilename}
              </span>
              <button
                onClick={() => setPreviewFilename('')}
                className="text-gray-400 hover:text-white"
              >
                ✕ 關閉
              </button>
            </div>

            <div className="p-4 flex-1 overflow-y-auto font-mono text-xs space-y-4">
              {isPreviewLoading ? (
                <div className="text-center py-12 text-cyan-400 animate-pulse">正在索取物資...</div>
              ) : (
                <>
                  {previewFilename.endsWith('.gpg') && (
                    <div className="p-3 bg-emerald-950/40 border border-emerald-700/50 rounded-lg text-emerald-300">
                      <div className="font-bold text-xs mb-1">🔓 私鑰解密結果:</div>
                      <div className="whitespace-pre-wrap">{previewDecrypted}</div>
                    </div>
                  )}

                  <div className="p-3 bg-[#080b10] border border-gray-800 rounded-lg text-gray-400">
                    <div className="font-bold text-xs mb-1 text-gray-300">📦 原始雲端文本:</div>
                    <div className="whitespace-pre-wrap max-h-60 overflow-y-auto">
                      {previewContent}
                    </div>
                  </div>
                </>
              )}
            </div>
          </div>
        </div>
      )}
      {/* Migrate Repo Clean Slate Modal */}
      {showMigrateModal && (
        <div className="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="bg-[#0e131d] border border-rose-800/80 rounded-2xl max-w-xl w-full max-h-[85vh] flex flex-col overflow-hidden shadow-2xl font-mono text-xs">
            <div className="px-4 py-3 bg-[#181119] border-b border-rose-900/60 flex items-center justify-between">
              <span className="text-rose-400 font-bold flex items-center gap-2 text-sm">
                <History className="w-4 h-4" />
                <span>抹除歷史修訂 / 轉移至全新倉庫</span>
              </span>
              <button
                onClick={() => setShowMigrateModal(false)}
                className="text-gray-400 hover:text-white"
              >
                ✕
              </button>
            </div>

            <div className="p-5 space-y-4 overflow-y-auto">
              <div className="p-3 bg-rose-950/40 border border-rose-800/60 rounded-xl space-y-2 text-rose-200">
                <div className="font-bold flex items-center gap-1.5 text-rose-300">
                  <AlertTriangle className="w-4 h-4 text-rose-400 flex-shrink-0" />
                  <span>為什麼需要轉移全新倉庫？</span>
                </div>
                <p className="text-[11px] text-gray-300 leading-relaxed">
                  Git 與 GitHub Gist 擁有完整的版本時間軸，所有過往的 commit 修改歷程均永久留痕。若您曾於舊倉庫中誤提交過未完全封裝之機密，單純覆蓋或刪除檔案無法清除歷史修訂版本。
                </p>
                <p className="text-[11px] text-gray-300 leading-relaxed">
                  執行此操作後，系統將<strong>在雲端建立一個全新的乾淨 Gist 保險庫</strong>，僅將本地目前已鎖定封裝之最新檔案導入新倉庫，徹底切斷歷史版本追蹤鏈條。
                </p>
              </div>

              <div className="p-3 bg-[#111622] rounded-xl border border-gray-800 space-y-2">
                <div className="text-gray-300 font-semibold">當前倉庫錨定狀態：</div>
                <div className="text-gray-400 text-[11px]">
                  當前 Gist ID:{' '}
                  <code className="text-cyan-300">{config.gistId || '未配置'}</code>
                </div>
                <div className="text-gray-400 text-[11px]">
                  憑證狀態:{' '}
                  <span className={config.tokenDecrypted ? 'text-emerald-400' : 'text-amber-400'}>
                    {config.tokenDecrypted ? '已就緒 (token.gpg)' : '未檢測到 Token'}
                  </span>
                </div>
              </div>

              <div className="flex items-start gap-2.5 p-3 bg-amber-950/30 border border-amber-800/50 rounded-xl">
                <input
                  type="checkbox"
                  id="delete-old-repo-checkbox"
                  checked={deleteOldRepo}
                  onChange={(e) => setDeleteOldRepo(e.target.checked)}
                  className="mt-0.5 rounded bg-[#121620] border-gray-700 text-rose-500 focus:ring-0"
                />
                <label htmlFor="delete-old-repo-checkbox" className="text-gray-200 text-xs cursor-pointer">
                  <span className="font-bold text-rose-300">徹底銷毀舊倉庫 (--delete-old)</span>
                  <div className="text-[10px] text-gray-400 mt-0.5">
                    新倉庫建立並注入所有檔案後，自動調用 GitHub DELETE API 將舊 Gist 永久銷毀，杜絕歷史洩漏。
                  </div>
                </label>
              </div>

              {migrateResult && (
                <div
                  className={`p-3 rounded-xl border text-[11px] font-mono ${
                    migrateResult.startsWith('✨')
                      ? 'bg-emerald-950/60 border-emerald-700/60 text-emerald-300'
                      : 'bg-rose-950/60 border-rose-700/60 text-rose-300'
                  }`}
                >
                  {migrateResult}
                </div>
              )}
            </div>

            <div className="p-4 bg-[#141824] border-t border-gray-800 flex items-center justify-between">
              <div className="text-[10px] text-gray-500">
                對應命令列參數: <code className="text-cyan-400">a --migrate-repo --delete-old</code>
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setShowMigrateModal(false)}
                  className="px-3 py-1.5 bg-gray-800 hover:bg-gray-700 text-gray-300 rounded text-xs transition"
                >
                  取消
                </button>
                <button
                  onClick={handleMigrateRepo}
                  disabled={isMigrating || !config.tokenDecrypted}
                  className="px-4 py-1.5 bg-rose-700 hover:bg-rose-600 disabled:opacity-40 text-white font-bold rounded text-xs transition flex items-center gap-1.5 shadow-md"
                >
                  {isMigrating ? (
                    <>
                      <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                      <span>正在建立全新倉庫並抹除歷史...</span>
                    </>
                  ) : (
                    <>
                      <Sparkles className="w-3.5 h-3.5" />
                      <span>確認抹除並全新遷移</span>
                    </>
                  )}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* 🛡️ 遠端檔案在位套殼加密彈窗 (Remote In-Place Encapsulate Modal) */}
      {showEncapsulateModal && (
        <div className="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="bg-[#0c1018] border border-amber-600/70 rounded-2xl max-w-lg w-full overflow-hidden shadow-2xl font-mono animate-in fade-in zoom-in-95 duration-150">
            <div className="p-4 bg-gradient-to-r from-amber-950/80 via-[#16120c] to-[#0c1018] border-b border-amber-800/50 flex items-center justify-between">
              <div className="flex items-center gap-2 text-amber-400 font-bold text-sm">
                <Lock className="w-4 h-4 text-amber-400" />
                <span>遠端檔案在位套殼加密 (Remote In-Place Encapsulate)</span>
              </div>
              <button
                onClick={() => setShowEncapsulateModal(false)}
                className="text-gray-400 hover:text-white"
              >
                ✕
              </button>
            </div>

            <div className="p-5 space-y-4 overflow-y-auto max-h-[80vh] text-xs">
              <div className="p-3 bg-amber-950/40 border border-amber-800/60 rounded-xl space-y-1.5 text-amber-200">
                <div className="font-bold flex items-center gap-1.5 text-amber-300">
                  <Sparkles className="w-4 h-4 text-amber-400 flex-shrink-0" />
                  <span>套殼加密運作原理：</span>
                </div>
                <p className="text-[11px] text-gray-300 leading-relaxed">
                  將本來明文的遠端檔案，以強密碼學 GPG 算法加密後保存到遠端，並在遠端原子刪除原本的明文檔案。就好像在遠端自動套了一層堅不可摧的加密殼一樣！
                </p>
              </div>

              <div className="p-3 bg-[#111622] rounded-xl border border-gray-800 space-y-2">
                <div className="flex items-center justify-between text-gray-300">
                  <span className="text-gray-400">欲套殼檔案：</span>
                  <code className="text-amber-300 font-bold bg-amber-950/50 px-2 py-0.5 rounded border border-amber-800/40">
                    {encapsulateTarget}
                  </code>
                </div>
                <div className="flex items-center justify-between text-gray-300">
                  <span className="text-gray-400">套殼後密文檔名：</span>
                  <code className="text-cyan-300 font-bold bg-cyan-950/50 px-2 py-0.5 rounded border border-cyan-800/40">
                    {encapsulateTarget}.gpg
                  </code>
                </div>
              </div>

              {/* 加密模式選擇 */}
              <div className="space-y-2">
                <label className="text-gray-300 font-semibold block">加密方案選擇：</label>
                <div className="grid grid-cols-2 gap-2">
                  <button
                    type="button"
                    onClick={() => setEncapsulateMode('gpg')}
                    className={`p-2.5 rounded-xl border text-left transition ${
                      encapsulateMode === 'gpg'
                        ? 'bg-cyan-950/70 border-cyan-500 text-cyan-200'
                        : 'bg-[#111622] border-gray-800 text-gray-400 hover:border-gray-700'
                    }`}
                  >
                    <div className="font-bold flex items-center gap-1.5">
                      <Shield className="w-3.5 h-3.5 text-cyan-400" />
                      <span>鎖定 GPG 金鑰</span>
                    </div>
                    <div className="text-[10px] text-gray-500 mt-1 truncate">
                      {config.gpgKeyId || 'CyberNOte-Key (預設公鑰)'}
                    </div>
                  </button>

                  <button
                    type="button"
                    onClick={() => setEncapsulateMode('symmetric')}
                    className={`p-2.5 rounded-xl border text-left transition ${
                      encapsulateMode === 'symmetric'
                        ? 'bg-purple-950/70 border-purple-500 text-purple-200'
                        : 'bg-[#111622] border-gray-800 text-gray-400 hover:border-gray-700'
                    }`}
                  >
                    <div className="font-bold flex items-center gap-1.5">
                      <Key className="w-3.5 h-3.5 text-purple-400" />
                      <span>防窮舉密碼 (S2K)</span>
                    </div>
                    <div className="text-[10px] text-gray-500 mt-1">
                      AES256 · 65,011,712 輪
                    </div>
                  </button>
                </div>
              </div>

              {/* 對稱密碼輸入框 */}
              {encapsulateMode === 'symmetric' && (
                <div className="p-3 bg-[#131722] rounded-xl border border-purple-800/40 space-y-2">
                  <label className="text-gray-300 block font-semibold text-[11px]">
                    自訂防窮舉通行密碼 (Passphrase)：
                  </label>
                  <input
                    type="password"
                    value={encapsulatePassphrase}
                    onChange={(e) => setEncapsulatePassphrase(e.target.value)}
                    placeholder="請輸入解鎖密碼..."
                    className="w-full bg-[#090d14] border border-gray-700 rounded-lg px-3 py-2 text-white placeholder-gray-600 focus:outline-none focus:border-purple-500 text-xs"
                  />
                  <div className="text-[10px] text-purple-300">
                    S2K Mode 3 演算法以 65,011,712 輪密鑰拉伸計算，免疫量子窮舉爆破。
                  </div>
                </div>
              )}

              {/* 核心選項：刪除遠端原始明文 */}
              <div className="p-3 bg-amber-950/30 border border-amber-800/50 rounded-xl flex items-start gap-2.5">
                <input
                  type="checkbox"
                  id="delete-remote-original-checkbox"
                  checked={encapsulateDeleteOriginal}
                  onChange={(e) => setEncapsulateDeleteOriginal(e.target.checked)}
                  className="mt-0.5 rounded bg-[#121620] border-gray-700 text-amber-500 focus:ring-0 cursor-pointer"
                />
                <label htmlFor="delete-remote-original-checkbox" className="cursor-pointer">
                  <div className="font-bold text-amber-300">
                    在遠端自動刪除原始檔案 ({encapsulateTarget})
                  </div>
                  <div className="text-[10px] text-gray-400 mt-0.5 leading-relaxed">
                    遠端僅留存加密後的 <code>{encapsulateTarget}.gpg</code> 密文殼，徹底杜絕原始檔案在雲端裸露。
                  </div>
                </label>
              </div>

              <div className="text-[10px] text-gray-500 flex items-center gap-1.5">
                <Check className="w-3.5 h-3.5 text-emerald-400" />
                <span>加密完成後將自動在本地金鑰歸檔簿 (Key Ledger) 留存 SHA-256 與審計層級。</span>
              </div>

              {encapsulateError && (
                <div className="p-3 bg-rose-950/60 border border-rose-700/60 rounded-xl text-rose-300 text-[11px]">
                  ❌ {encapsulateError}
                </div>
              )}

              {encapsulateResult && (
                <div className="p-3 bg-emerald-950/60 border border-emerald-700/60 rounded-xl text-emerald-300 text-[11px]">
                  {encapsulateResult}
                </div>
              )}
            </div>

            <div className="p-4 bg-[#141824] border-t border-gray-800 flex items-center justify-between">
              <div className="text-[10px] text-gray-500">
                對應命令列: <code className="text-amber-400">a --remote-encrypt {encapsulateTarget}</code>
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setShowEncapsulateModal(false)}
                  className="px-3 py-1.5 bg-gray-800 hover:bg-gray-700 text-gray-300 rounded text-xs transition"
                >
                  取消
                </button>
                <button
                  onClick={handleExecuteEncapsulate}
                  disabled={isEncapsulating || (encapsulateMode === 'symmetric' && !encapsulatePassphrase)}
                  className="px-4 py-1.5 bg-amber-500 hover:bg-amber-400 disabled:opacity-40 text-black font-bold rounded text-xs transition flex items-center gap-1.5 shadow-md shadow-amber-950/50"
                >
                  {isEncapsulating ? (
                    <>
                      <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                      <span>正在封裝密文並原子替換...</span>
                    </>
                  ) : (
                    <>
                      <Lock className="w-3.5 h-3.5" />
                      <span>執行遠端套殼加密</span>
                    </>
                  )}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* 🔓 遠端密文檔案解密彈窗 (Decapsulate & Plaintext Policy Modal) */}
      {showDecapsulateModal && (
        <div className="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="bg-[#0c1018] border border-emerald-600/70 rounded-2xl max-w-lg w-full overflow-hidden shadow-2xl font-mono animate-in fade-in zoom-in-95 duration-150">
            <div className="p-4 bg-gradient-to-r from-emerald-950/80 via-[#0c1510] to-[#0c1018] border-b border-emerald-800/50 flex items-center justify-between">
              <div className="flex items-center gap-2 text-emerald-400 font-bold text-sm">
                <Unlock className="w-4 h-4 text-emerald-400" />
                <span>遠端密文檔案解密與還原原則</span>
              </div>
              <button
                onClick={() => setShowDecapsulateModal(false)}
                className="text-gray-400 hover:text-white"
              >
                ✕
              </button>
            </div>

            <div className="p-5 space-y-4 overflow-y-auto max-h-[80vh] text-xs">
              <div className="p-3 bg-emerald-950/40 border border-emerald-800/60 rounded-xl space-y-1.5 text-emerald-200">
                <div className="font-bold flex items-center gap-1.5 text-emerald-300">
                  <Shield className="w-4 h-4 text-emerald-400 flex-shrink-0" />
                  <span>雲端防護原則（全權由您掌控）：</span>
                </div>
                <p className="text-[11px] text-gray-300 leading-relaxed">
                  您可以直接在本地解密查閱與入庫保存，<strong>亦可選擇不把遠端的加密檔案解密後再上傳一份明文文件</strong>，保持雲端始終處於密文封鎖狀態！
                </p>
              </div>

              <div className="p-3 bg-[#111622] rounded-xl border border-gray-800 space-y-2">
                <div className="flex items-center justify-between text-gray-300">
                  <span className="text-gray-400">解密目標檔案：</span>
                  <code className="text-cyan-300 font-bold bg-cyan-950/50 px-2 py-0.5 rounded border border-cyan-800/40">
                    {decapsulateTarget}
                  </code>
                </div>
                <div className="flex items-center justify-between text-gray-300">
                  <span className="text-gray-400">還原明文檔名：</span>
                  <code className="text-emerald-300 font-bold bg-emerald-950/50 px-2 py-0.5 rounded border border-emerald-800/40">
                    {decapsulateTarget.endsWith('.gpg') ? decapsulateTarget.slice(0, -4) : `${decapsulateTarget}.txt`}
                  </code>
                </div>
              </div>

              {/* 密碼輸入 */}
              <div className="space-y-1.5">
                <label className="text-gray-300 font-semibold block">解密通行密碼 (Passphrase / 私鑰口令)：</label>
                <input
                  type="password"
                  value={decapsulatePassphrase}
                  onChange={(e) => setDecapsulatePassphrase(e.target.value)}
                  placeholder="若為對稱防窮舉加密或需密鑰口令請輸入..."
                  className="w-full bg-[#111622] border border-gray-700 rounded-lg px-3 py-2 text-white placeholder-gray-600 focus:outline-none focus:border-emerald-500 text-xs"
                />
              </div>

              {/* 遠端明文策略選擇 */}
              <div className="space-y-2">
                <label className="text-gray-300 font-semibold block">遠端明文上傳策略：</label>

                {/* 策略 1: 僅在本地解密 (推薦) */}
                <div
                  onClick={() => setDecapsulateRestoreRemote(false)}
                  className={`p-3 rounded-xl border cursor-pointer transition ${
                    !decapsulateRestoreRemote
                      ? 'bg-cyan-950/60 border-cyan-500 text-cyan-200'
                      : 'bg-[#111622] border-gray-800 text-gray-400 hover:border-gray-700'
                  }`}
                >
                  <div className="font-bold flex items-center gap-2">
                    <Shield className="w-4 h-4 text-cyan-400" />
                    <span>🛡️ 僅在本地解密查閱（推薦：遠端絕不上傳明文）</span>
                  </div>
                  <div className="text-[10px] text-gray-400 mt-1 pl-6 leading-relaxed">
                    解密後的內容安全留存於本地 <code>~/BOok/NOte</code> 目錄中。遠端 Gist 倉庫繼續維持 GPG 密文殼封存，零洩漏風險。
                  </div>
                </div>

                {/* 策略 2: 同時還原遠端明文 */}
                <div
                  onClick={() => setDecapsulateRestoreRemote(true)}
                  className={`p-3 rounded-xl border cursor-pointer transition ${
                    decapsulateRestoreRemote
                      ? 'bg-amber-950/60 border-amber-500 text-amber-200'
                      : 'bg-[#111622] border-gray-800 text-gray-400 hover:border-gray-700'
                  }`}
                >
                  <div className="font-bold flex items-center gap-2 text-amber-300">
                    <AlertTriangle className="w-4 h-4 text-amber-400" />
                    <span>⚠️ 在遠端還原一份明文檔案 (去殼發佈)</span>
                  </div>
                  <div className="text-[10px] text-gray-400 mt-1 pl-6 leading-relaxed">
                    將解密後的原始明文同步推送發佈至 GitHub Gist 雲端倉庫。
                  </div>

                  {decapsulateRestoreRemote && (
                    <div className="mt-2.5 pt-2 border-t border-amber-800/40 pl-6 flex items-center gap-2">
                      <input
                        type="checkbox"
                        id="delete-remote-gpg-check"
                        checked={decapsulateDeleteRemoteGpg}
                        onChange={(e) => setDecapsulateDeleteRemoteGpg(e.target.checked)}
                        className="rounded bg-[#121620] border-gray-700 text-rose-500 focus:ring-0"
                      />
                      <label htmlFor="delete-remote-gpg-check" className="text-[11px] text-rose-300 cursor-pointer">
                        同時自遠端刪除 <code>{decapsulateTarget}</code> 密文檔案
                      </label>
                    </div>
                  )}
                </div>
              </div>

              {decapsulateError && (
                <div className="p-3 bg-rose-950/60 border border-rose-700/60 rounded-xl text-rose-300 text-[11px]">
                  ❌ {decapsulateError}
                </div>
              )}

              {decapsulateResult && (
                <div className="p-3 bg-emerald-950/60 border border-emerald-700/60 rounded-xl text-emerald-300 text-[11px] space-y-2">
                  <div>{decapsulateResult}</div>
                  {decapsulatedContentPreview && (
                    <div className="mt-2 pt-2 border-t border-emerald-800/50">
                      <div className="text-[10px] text-gray-400 mb-1">解密明文預覽：</div>
                      <pre className="p-2 bg-black/60 rounded text-[10px] text-emerald-200 overflow-x-auto max-h-32">
                        {decapsulatedContentPreview}
                      </pre>
                    </div>
                  )}
                </div>
              )}
            </div>

            <div className="p-4 bg-[#141824] border-t border-gray-800 flex items-center justify-between">
              <div className="text-[10px] text-gray-500">
                對應命令列: <code className="text-emerald-400">a -d {decapsulateTarget} -x</code>
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setShowDecapsulateModal(false)}
                  className="px-3 py-1.5 bg-gray-800 hover:bg-gray-700 text-gray-300 rounded text-xs transition"
                >
                  關閉
                </button>
                <button
                  onClick={handleExecuteDecapsulate}
                  disabled={isDecapsulating}
                  className="px-4 py-1.5 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-40 text-black font-bold rounded text-xs transition flex items-center gap-1.5 shadow-md"
                >
                  {isDecapsulating ? (
                    <>
                      <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                      <span>正在破甲解密...</span>
                    </>
                  ) : (
                    <>
                      <Unlock className="w-3.5 h-3.5" />
                      <span>執行解密</span>
                    </>
                  )}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
