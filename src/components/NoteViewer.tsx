import React, { useState, useEffect } from 'react';
import {
  FileText,
  Lock,
  Unlock,
  Plus,
  Trash2,
  RefreshCw,
  Search,
  Eye,
  Calendar,
  CloudUpload,
  Download,
  AlertCircle,
  Copy,
  Check,
} from 'lucide-react';
import { AppConfig, NoteFile } from '../types';
import { decryptWithGpg, encryptWithGpg } from '../lib/crypto';
import { saveNote, loadAllNotes } from '../lib/storage';
import { syncToGist } from '../lib/gist';

interface NoteViewerProps {
  config: AppConfig;
  notes: Record<string, NoteFile>;
  onNotesChange: (notes: Record<string, NoteFile>) => void;
  onSyncToGist?: (filename: string) => void;
}

export const NoteViewer: React.FC<NoteViewerProps> = ({
  config,
  notes,
  onNotesChange,
}) => {
  const currentYear = new Date().getFullYear().toString();
  const fileKeys = Object.keys(notes).sort().reverse();
  const [selectedFilename, setSelectedFilename] = useState<string>(
    fileKeys[0] || `${currentYear}.note.gpg`
  );

  const [decryptedText, setDecryptedText] = useState<string>('');
  const [showRawArmor, setShowRawArmor] = useState<boolean>(false);
  const [isDecrypting, setIsDecrypting] = useState<boolean>(false);
  const [decryptError, setDecryptError] = useState<string>('');

  // Add Note state
  const [newNoteInput, setNewNoteInput] = useState<string>('');
  const [isAdding, setIsAdding] = useState<boolean>(false);

  // Line removal state
  const [filterKeyword, setFilterKeyword] = useState<string>('');
  const [deleteLineIndex, setDeleteLineIndex] = useState<string>('');
  const [isDeleting, setIsDeleting] = useState<boolean>(false);
  const [actionSuccess, setActionSuccess] = useState<string>('');
  const [copied, setCopied] = useState<boolean>(false);

  // Gist sync state
  const [isSyncing, setIsSyncing] = useState<boolean>(false);

  const activeFile = notes[selectedFilename];

  // Auto-decrypt when activeFile or config changes
  useEffect(() => {
    let isMounted = true;

    async function loadContent() {
      if (!activeFile) {
        setDecryptedText('');
        return;
      }
      if (!activeFile.content.includes('-----BEGIN PGP MESSAGE-----')) {
        setDecryptedText(activeFile.content);
        setDecryptError('');
        return;
      }

      setIsDecrypting(true);
      setDecryptError('');
      try {
        const text = await decryptWithGpg(activeFile.content, config.gpgKeyId);
        if (isMounted) {
          setDecryptedText(text);
        }
      } catch (err) {
        if (isMounted) {
          setDecryptError(
            err instanceof Error ? err.message : '解密失敗，可能需要正確的 GPG 密鑰或密碼'
          );
        }
      } finally {
        if (isMounted) setIsDecrypting(false);
      }
    }

    loadContent();
    return () => {
      isMounted = false;
    };
  }, [selectedFilename, activeFile?.content, config.gpgKeyId]);

  // Flash action notification
  const flashSuccess = (msg: string) => {
    setActionSuccess(msg);
    setTimeout(() => setActionSuccess(''), 3500);
  };

  // Add new note line
  const handleAddNote = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newNoteInput.trim() || isAdding) return;

    setIsAdding(true);
    const now = new Date();
    const timestamp = `[${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(
      now.getDate()
    ).padStart(2, '0')} ${String(now.getHours()).padStart(2, '0')}:${String(
      now.getMinutes()
    ).padStart(2, '0')}:${String(now.getSeconds()).padStart(2, '0')}]`;

    const formattedNote = `${timestamp} ${newNoteInput.trim()}`;
    const newDecrypted = decryptedText ? `${decryptedText}\n${formattedNote}` : formattedNote;

    try {
      const encryptedArmor = await encryptWithGpg(newDecrypted, config.gpgKeyId);
      const updatedNoteFile: NoteFile = {
        filename: selectedFilename,
        year: /^\d{4}/.exec(selectedFilename)?.[0] || currentYear,
        isEncrypted: true,
        content: encryptedArmor,
        decryptedContent: newDecrypted,
        lastModified: Date.now(),
      };

      saveNote(updatedNoteFile);
      onNotesChange(loadAllNotes());
      setDecryptedText(newDecrypted);
      setNewNoteInput('');
      flashSuccess('✨ 靈感已安全縫合並以 GPG 公鑰加密封存！');
    } catch (err) {
      alert(`寫入失敗: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setIsAdding(false);
    }
  };

  // Remove single line or range
  const handleDeleteLine = async (lineIdx: number) => {
    const lines = decryptedText.split('\n');
    if (lineIdx < 0 || lineIdx >= lines.length) return;

    const confirmed = window.confirm(`確認刪除該行？\n"${lines[lineIdx]}"`);
    if (!confirmed) return;

    lines.splice(lineIdx, 1);
    const updated = lines.join('\n');
    await saveNewContent(updated, '✨ 已成功剔除該行記錄並重新加密存盤！');
  };

  // Filter & Delete by keyword
  const handleDeleteByKeyword = async () => {
    if (!filterKeyword.trim()) return;
    const lines = decryptedText.split('\n');
    const matched = lines.filter((l) => l.includes(filterKeyword.trim()));
    if (matched.length === 0) {
      alert(`未找到包含「${filterKeyword}」的記錄行`);
      return;
    }

    const confirmed = window.confirm(
      `找到 ${matched.length} 筆包含「${filterKeyword}」的記錄，確認全數剔除？`
    );
    if (!confirmed) return;

    const remaining = lines.filter((l) => !l.includes(filterKeyword.trim()));
    await saveNewContent(
      remaining.join('\n'),
      `✨ 已成功過濾刪除 ${matched.length} 行包含「${filterKeyword}」的記錄！`
    );
    setFilterKeyword('');
  };

  // Helper to re-encrypt and save
  const saveNewContent = async (newText: string, successMsg: string) => {
    setIsDeleting(true);
    try {
      const encryptedArmor = await encryptWithGpg(newText, config.gpgKeyId);
      const updatedNoteFile: NoteFile = {
        filename: selectedFilename,
        year: /^\d{4}/.exec(selectedFilename)?.[0] || currentYear,
        isEncrypted: true,
        content: encryptedArmor,
        decryptedContent: newText,
        lastModified: Date.now(),
      };

      saveNote(updatedNoteFile);
      onNotesChange(loadAllNotes());
      setDecryptedText(newText);
      flashSuccess(successMsg);
    } catch (err) {
      alert(`更新失敗: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setIsDeleting(false);
    }
  };

  // Direct sync to GitHub Gist
  const handleQuickSync = async () => {
    if (!config.gistId) {
      alert('請先在設定中配置 GitHub Gist ID');
      return;
    }
    if (!config.tokenDecrypted) {
      alert('請先配置 GitHub Token');
      return;
    }

    setIsSyncing(true);
    try {
      await syncToGist(
        config.gistId,
        selectedFilename,
        activeFile?.content || '',
        config.tokenDecrypted
      );
      flashSuccess(`☁️ 包裹【${selectedFilename}】已成功同步出海至 GitHub Gist！`);
    } catch (err) {
      alert(`同步失敗: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setIsSyncing(false);
    }
  };

  const copyToClipboard = () => {
    navigator.clipboard.writeText(showRawArmor ? activeFile?.content || '' : decryptedText);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Download file locally
  const handleDownloadFile = () => {
    const text = showRawArmor ? activeFile?.content || '' : decryptedText;
    const blob = new Blob([text], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = showRawArmor ? selectedFilename : selectedFilename.replace(/\.gpg$/, '');
    link.click();
    URL.revokeObjectURL(url);
  };

  const noteLines = decryptedText ? decryptedText.split('\n') : [];

  return (
    <div className="space-y-6">
      {/* Top Banner with File Switcher and Actions */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 p-4 bg-[#12161f] border border-gray-800 rounded-xl">
        <div className="flex items-center gap-3 overflow-x-auto pb-1 sm:pb-0">
          <div className="flex items-center gap-2 text-cyan-400 font-mono text-sm font-semibold pr-2 border-r border-gray-700">
            <Calendar className="w-4 h-4" />
            <span>筆記倉庫</span>
          </div>

          <div className="flex items-center gap-1.5">
            {fileKeys.length === 0 ? (
              <span className="text-gray-500 text-xs font-mono">尚無存檔</span>
            ) : (
              fileKeys.map((fName) => (
                <button
                  key={fName}
                  onClick={() => setSelectedFilename(fName)}
                  className={`px-3 py-1.5 rounded-lg text-xs font-mono transition flex items-center gap-1.5 ${
                    selectedFilename === fName
                      ? 'bg-cyan-950/80 text-cyan-300 border border-cyan-500/50 shadow-sm'
                      : 'bg-[#181d28] text-gray-400 hover:text-gray-200 border border-gray-800'
                  }`}
                >
                  <FileText className="w-3.5 h-3.5" />
                  <span>{fName}</span>
                </button>
              ))
            )}
          </div>
        </div>

        <div className="flex items-center gap-2 flex-wrap">
          <button
            onClick={() => setShowRawArmor(!showRawArmor)}
            className={`px-3 py-1.5 rounded-lg text-xs font-mono transition flex items-center gap-1.5 border ${
              showRawArmor
                ? 'bg-amber-950/40 text-amber-300 border-amber-500/50'
                : 'bg-[#181d28] text-gray-300 border-gray-700 hover:bg-gray-800'
            }`}
            title="切換 ASCII Armor 密文 / 解密明文視圖"
          >
            {showRawArmor ? (
              <>
                <Unlock className="w-3.5 h-3.5 text-amber-400" />
                <span>檢視解密文本</span>
              </>
            ) : (
              <>
                <Lock className="w-3.5 h-3.5 text-cyan-400" />
                <span>檢視 GPG 密文 (Armor)</span>
              </>
            )}
          </button>

          <button
            onClick={handleQuickSync}
            disabled={isSyncing || !config.gistId}
            className="px-3 py-1.5 rounded-lg text-xs font-mono bg-cyan-900/40 hover:bg-cyan-800/60 text-cyan-300 border border-cyan-700/50 transition flex items-center gap-1.5 disabled:opacity-40"
            title="推送至 GitHub Gist (a -s)"
          >
            <CloudUpload className={`w-3.5 h-3.5 ${isSyncing ? 'animate-bounce' : ''}`} />
            <span>{isSyncing ? '同步中...' : '同步至 Gist'}</span>
          </button>

          <button
            onClick={copyToClipboard}
            className="p-1.5 rounded-lg text-gray-400 hover:text-white bg-[#181d28] border border-gray-700 hover:bg-gray-800 transition"
            title="複製內容"
          >
            {copied ? <Check className="w-4 h-4 text-emerald-400" /> : <Copy className="w-4 h-4" />}
          </button>

          <button
            onClick={handleDownloadFile}
            className="p-1.5 rounded-lg text-gray-400 hover:text-white bg-[#181d28] border border-gray-700 hover:bg-gray-800 transition"
            title="下載檔案至本地"
          >
            <Download className="w-4 h-4" />
          </button>
        </div>
      </div>

      {actionSuccess && (
        <div className="p-3 bg-emerald-950/50 border border-emerald-500/40 text-emerald-300 text-xs rounded-xl flex items-center gap-2 animate-fadeIn font-mono">
          <Check className="w-4 h-4 text-emerald-400 flex-shrink-0" />
          <span>{actionSuccess}</span>
        </div>
      )}

      {/* Note Input Box (a [content...]) */}
      <form
        onSubmit={handleAddNote}
        className="p-4 bg-[#12161f] border border-gray-800 rounded-xl space-y-3"
      >
        <div className="flex items-center justify-between">
          <label className="text-xs font-mono font-semibold text-emerald-400 flex items-center gap-2">
            <Plus className="w-4 h-4" />
            <span>寫入靈感記錄 (a [靈感內容...])</span>
          </label>
          <span className="text-[11px] text-gray-500 font-mono">
            目標檔案: {selectedFilename} · 公鑰加密封裝
          </span>
        </div>

        <div className="flex gap-2">
          <textarea
            id="new-note-textarea"
            rows={2}
            value={newNoteInput}
            onChange={(e) => setNewNoteInput(e.target.value)}
            placeholder="在此輸入靈感筆記... 系統自動附帶時間戳並調用 GPG 公鑰單向封裝"
            className="flex-1 px-3 py-2 bg-[#090d14] border border-gray-800 focus:border-cyan-500 rounded-lg text-sm font-mono text-gray-100 placeholder-gray-600 focus:outline-none resize-y"
          />
          <button
            id="submit-new-note-btn"
            type="submit"
            disabled={isAdding || !newNoteInput.trim()}
            className="px-4 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-40 text-white font-medium text-xs rounded-lg transition flex flex-col items-center justify-center gap-1 self-stretch shadow-md font-mono"
          >
            <Lock className="w-4 h-4" />
            <span>{isAdding ? '加密中...' : '落盤封存'}</span>
          </button>
        </div>
      </form>

      {/* Main Content Display (with Signature alternating Green / Cyan rendering) */}
      <div className="bg-[#090d14] border border-cyan-900/30 rounded-xl overflow-hidden shadow-xl">
        <div className="flex items-center justify-between px-4 py-2.5 bg-[#141923] border-b border-gray-800 text-xs">
          <div className="flex items-center gap-2 font-mono text-gray-400">
            <Eye className="w-4 h-4 text-cyan-400" />
            <span className="text-gray-200 font-semibold">{selectedFilename}</span>
            <span className="text-gray-600">|</span>
            <span className="text-gray-500">
              {showRawArmor ? 'ASCII Armor 密文' : `共 ${noteLines.length} 行記錄 (雙色高對比渲染)`}
            </span>
          </div>

          {!showRawArmor && (
            <div className="flex items-center gap-2">
              <div className="relative">
                <input
                  type="text"
                  value={filterKeyword}
                  onChange={(e) => setFilterKeyword(e.target.value)}
                  placeholder="搜尋 / 行級剔除關鍵字..."
                  className="pl-7 pr-2 py-1 bg-[#0d1118] border border-gray-700/60 rounded text-xs font-mono text-gray-200 placeholder-gray-600 focus:outline-none focus:border-cyan-500 w-44 sm:w-56"
                />
                <Search className="w-3.5 h-3.5 text-gray-500 absolute left-2 top-2" />
              </div>

              {filterKeyword && (
                <button
                  onClick={handleDeleteByKeyword}
                  className="px-2 py-1 bg-rose-950/60 hover:bg-rose-900/80 text-rose-300 border border-rose-800 rounded text-xs font-mono transition flex items-center gap-1"
                  title="剔除包含此關鍵字的所有行 (a -r [keyword])"
                >
                  <Trash2 className="w-3 h-3" />
                  <span>剔除關鍵字行</span>
                </button>
              )}
            </div>
          )}
        </div>

        {decryptError && (
          <div className="p-4 bg-rose-950/30 border-b border-rose-900/40 text-rose-300 text-xs flex items-center gap-2 font-mono">
            <AlertCircle className="w-4 h-4 text-rose-400 flex-shrink-0" />
            <span>{decryptError}</span>
          </div>
        )}

        {isDecrypting ? (
          <div className="p-12 text-center text-cyan-400 text-sm font-mono animate-pulse">
            🔓 [保密局] 正在調用 GPG 密鑰進行破甲解密...
          </div>
        ) : showRawArmor ? (
          /* Raw ASCII Armor View */
          <div className="p-4 font-mono text-xs text-amber-200/90 whitespace-pre-wrap leading-relaxed overflow-x-auto selection:bg-amber-500/20">
            {activeFile?.content || '（無密文內容）'}
          </div>
        ) : noteLines.length === 0 ? (
          <div className="p-12 text-center text-gray-500 text-sm font-mono">
            📂 暫無靈感記錄，使用上方輸入框或指令列加入第一筆記錄。
          </div>
        ) : (
          /* Alternating Green / Cyan Terminal Colored Lines (from color.rs) */
          <div className="divide-y divide-gray-900/60 font-mono text-sm">
            {noteLines.map((line, idx) => {
              const isEven = idx % 2 === 0;
              const isMatch = filterKeyword && line.toLowerCase().includes(filterKeyword.toLowerCase());
              // Green for even lines, Cyan for odd lines (exact rust color.rs match)
              const textColor = isEven ? 'text-emerald-400' : 'text-cyan-400';

              return (
                <div
                  key={idx}
                  className={`flex items-start justify-between group px-4 py-2 hover:bg-[#111622] transition ${
                    isMatch ? 'bg-cyan-950/40 ring-1 ring-cyan-500/30' : ''
                  }`}
                >
                  <div className="flex items-start gap-3 flex-1 min-w-0">
                    <span className="text-gray-600 text-xs select-none w-8 text-right flex-shrink-0 pt-0.5">
                      {idx + 1}
                    </span>
                    <span className={`break-words whitespace-pre-wrap leading-relaxed ${textColor}`}>
                      {line}
                    </span>
                  </div>

                  <button
                    onClick={() => handleDeleteLine(idx)}
                    className="opacity-0 group-hover:opacity-100 p-1 text-gray-500 hover:text-rose-400 hover:bg-rose-950/40 rounded transition ml-2 flex-shrink-0"
                    title={`刪除第 ${idx + 1} 行 (倒數第 ${noteLines.length - idx} 行)`}
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};
