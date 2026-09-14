// src/components/KeyLedgerView.tsx
// Cyber-NOte 金鑰歸檔審計簿與檔案加密工作台 (a -p & a -k)

import React, { useState, useEffect } from 'react';
import {
  ShieldCheck,
  Lock,
  Layers,
  FileCode,
  Key,
  Database,
  RefreshCw,
  Zap,
  CheckCircle2,
  AlertCircle,
  Hash,
  Clock,
  ArrowDownToLine,
  UploadCloud,
  FileCheck,
  ShieldAlert,
} from 'lucide-react';
import { KeyLedger, KeyLedgerEntry, RustStatus } from '../types';

interface KeyLedgerViewProps {
  rustStatus: RustStatus | null;
  onRunCommand: (command: string) => void;
}

export const KeyLedgerView: React.FC<KeyLedgerViewProps> = ({
  rustStatus,
  onRunCommand,
}) => {
  const [ledger, setLedger] = useState<KeyLedger | null>(null);
  const [loading, setLoading] = useState(false);
  const [executing, setExecuting] = useState(false);
  const [execResult, setExecResult] = useState<string | null>(null);

  // Form states for a -p
  const [targetFile, setTargetFile] = useState('1.txt');
  const [encryptMode, setEncryptMode] = useState<'symmetric' | 'gpg'>('symmetric');
  const [passphrase, setPassphrase] = useState('');
  const [s2kIterations, setS2kIterations] = useState(65011712);
  const [uploadToGist, setUploadToGist] = useState(false);

  // Decrypt form states for a -x
  const [decryptTarget, setDecryptTarget] = useState('');
  const [decryptPass, setDecryptPass] = useState('');

  const fetchLedger = async () => {
    setLoading(true);
    try {
      const res = await fetch('/api/ledger');
      if (res.ok) {
        const data: KeyLedger = await res.json();
        setLedger(data);
      }
    } catch {
      // Fallback
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchLedger();
  }, []);

  const handleProtectFile = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!targetFile.trim()) return;

    setExecuting(true);
    setExecResult(null);

    const args = ['-p', targetFile.trim()];
    if (encryptMode === 'symmetric') {
      if (!passphrase.trim()) {
        setExecResult('❌ 錯誤：對稱加密模式必須提供防窮舉保護密碼！');
        setExecuting(false);
        return;
      }
      args.push('--pass', passphrase.trim());
      args.push('--iter', s2kIterations.toString());
    }
    if (uploadToGist) {
      args.push('-u');
    }

    try {
      const res = await fetch('/api/encrypt-file', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          filename: targetFile.trim(),
          passphrase: encryptMode === 'symmetric' ? passphrase.trim() : undefined,
          iterations: s2kIterations,
          useGpgKey: encryptMode === 'gpg',
          uploadToGist,
        }),
      });
      const data = await res.json();
      if (!res.ok) {
        throw new Error(data.error || '加密失敗');
      }
      setExecResult(
        `🔒 加密成功！輸出檔案: ${data.outputFileName} (層級: 第 ${data.layer} 層, 模式: ${data.cipherMode}, S2K: ${data.iterations} 輪, 金鑰 ID: ${data.keyIdUsed}${
          data.gistSynced ? ', 已同步至 Gist' : ''
        })`
      );
      await fetchLedger();
    } catch (err: any) {
      setExecResult(`❌ 執行失敗: ${err.message}`);
    } finally {
      setExecuting(false);
    }
  };

  const handleUnwrapLayer = async (filename: string) => {
    setExecuting(true);
    setExecResult(null);
    const args = ['-x', filename];
    if (decryptPass.trim()) {
      args.push('--pass', decryptPass.trim());
    }

    try {
      const res = await fetch('/api/rust/exec', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ args }),
      });
      const data = await res.json();
      setExecResult(data.stdout || data.stderr || '解密執行完成');
      await fetchLedger();
    } catch (err: any) {
      setExecResult(`❌ 解密失敗: ${err.message}`);
    } finally {
      setExecuting(false);
    }
  };

  const records = ledger?.records || [];

  return (
    <div className="space-y-6 font-mono text-sm">
      {/* Header & Policy Verification Banner */}
      <div className="bg-[#0b1019] border border-cyan-900/60 rounded-2xl p-5 shadow-2xl relative overflow-hidden">
        <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4 border-b border-gray-800/80 pb-4">
          <div className="space-y-1">
            <div className="flex items-center gap-2 text-cyan-400">
              <ShieldCheck className="w-5 h-5" />
              <h2 className="text-base font-bold text-gray-100 tracking-wide">
                Cyber-NOte 金鑰歸檔審計簿與檔案加密系統
              </h2>
            </div>
            <p className="text-xs text-gray-400">
              嚴格隔離 SSH 金鑰體系，鎖定 GPG 金鑰隔離架構，支援多層巢狀加密與高迭代 S2K 防窮舉加固。
            </p>
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={fetchLedger}
              disabled={loading}
              className="px-3 py-1.5 bg-[#161c28] hover:bg-[#202737] text-gray-300 border border-gray-700/60 rounded-lg text-xs font-semibold transition flex items-center gap-1.5"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
              <span>重新檢索歸檔簿</span>
            </button>
          </div>
        </div>

        {/* Security Policy Status Badges */}
        <div className="grid grid-cols-1 md:grid-cols-4 gap-3 pt-4">
          <div className="p-3 bg-[#0d131f] border border-emerald-900/50 rounded-xl flex items-center gap-3">
            <ShieldCheck className="w-6 h-6 text-emerald-400 flex-shrink-0" />
            <div>
              <div className="text-[11px] text-gray-400">金鑰隔離規範</div>
              <div className="text-xs font-bold text-emerald-400">嚴格拒絕 SSH 金鑰</div>
            </div>
          </div>

          <div className="p-3 bg-[#0d131f] border border-cyan-900/50 rounded-xl flex items-center gap-3">
            <Key className="w-6 h-6 text-cyan-400 flex-shrink-0" />
            <div>
              <div className="text-[11px] text-gray-400">鎖定 GPG 金鑰</div>
              <div className="text-xs font-bold text-cyan-300 truncate max-w-[130px]">
                {rustStatus?.keyId || '未配置 GPG'}
              </div>
            </div>
          </div>

          <div className="p-3 bg-[#0d131f] border border-purple-900/50 rounded-xl flex items-center gap-3">
            <Zap className="w-6 h-6 text-purple-400 flex-shrink-0" />
            <div>
              <div className="text-[11px] text-gray-400">S2K 防窮舉運算</div>
              <div className="text-xs font-bold text-purple-300">65,011,712 輪加固</div>
            </div>
          </div>

          <div className="p-3 bg-[#0d131f] border border-amber-900/50 rounded-xl flex items-center gap-3">
            <Database className="w-6 h-6 text-amber-400 flex-shrink-0" />
            <div>
              <div className="text-[11px] text-gray-400">歸檔簿收錄總數</div>
              <div className="text-xs font-bold text-amber-300">{records.length} 筆審計檔案</div>
            </div>
          </div>
        </div>
      </div>

      {/* Main Action Grid: Encrypt / Protect Tool & Quick Unpack */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
        {/* Left Column: a -p Interactive Tool */}
        <div className="lg:col-span-6 bg-[#0b1019] border border-cyan-900/40 rounded-2xl p-5 shadow-xl space-y-4">
          <div className="flex items-center gap-2 text-cyan-400 border-b border-gray-800 pb-3">
            <Lock className="w-4 h-4" />
            <h3 className="font-bold text-gray-200">
              檔案加密封裝工作台 (命令: a -p)
            </h3>
          </div>

          <form onSubmit={handleProtectFile} className="space-y-4 text-xs">
            <div>
              <label className="block text-gray-400 mb-1">
                目標檔案路徑 (支援普通檔案 1.txt 或既有 .gpg 巢狀多層加密):
              </label>
              <input
                type="text"
                value={targetFile}
                onChange={(e) => setTargetFile(e.target.value)}
                placeholder="例如: 1.txt 或 1.gpg"
                className="w-full bg-[#111724] border border-gray-700 rounded-lg px-3 py-2 text-gray-200 focus:outline-none focus:border-cyan-500"
              />
            </div>

            <div className="space-y-2">
              <label className="block text-gray-400">加密模式選擇:</label>
              <div className="grid grid-cols-2 gap-2">
                <button
                  type="button"
                  onClick={() => setEncryptMode('symmetric')}
                  className={`py-2 px-3 rounded-lg border text-left flex items-center gap-2 transition ${
                    encryptMode === 'symmetric'
                      ? 'bg-cyan-950/60 border-cyan-500 text-cyan-300'
                      : 'bg-[#111724] border-gray-800 text-gray-400 hover:text-gray-300'
                  }`}
                >
                  <Zap className="w-3.5 h-3.5 text-cyan-400" />
                  <div>
                    <div className="font-bold">對稱密碼防窮舉</div>
                    <div className="text-[10px] text-gray-400">S2K 65011712 輪</div>
                  </div>
                </button>

                <button
                  type="button"
                  onClick={() => setEncryptMode('gpg')}
                  className={`py-2 px-3 rounded-lg border text-left flex items-center gap-2 transition ${
                    encryptMode === 'gpg'
                      ? 'bg-cyan-950/60 border-cyan-500 text-cyan-300'
                      : 'bg-[#111724] border-gray-800 text-gray-400 hover:text-gray-300'
                  }`}
                >
                  <Key className="w-3.5 h-3.5 text-cyan-400" />
                  <div>
                    <div className="font-bold">GPG 公鑰鎖定</div>
                    <div className="text-[10px] text-gray-400">隔離 SSH 密鑰</div>
                  </div>
                </button>
              </div>
            </div>

            {encryptMode === 'symmetric' && (
              <div className="space-y-3 p-3 bg-[#0d131f] border border-gray-800 rounded-xl">
                <div>
                  <label className="block text-gray-400 mb-1">
                    防窮舉加固密碼:
                  </label>
                  <input
                    type="password"
                    value={passphrase}
                    onChange={(e) => setPassphrase(e.target.value)}
                    placeholder="請輸入高強度密碼"
                    className="w-full bg-[#111724] border border-gray-700 rounded-lg px-3 py-2 text-gray-200 focus:outline-none focus:border-cyan-500"
                  />
                </div>

                <div>
                  <label className="block text-gray-400 mb-1">
                    S2K 迭代輪數 (防 GPU/算力窮舉破解):
                  </label>
                  <input
                    type="number"
                    value={s2kIterations}
                    onChange={(e) => setS2kIterations(parseInt(e.target.value) || 65011712)}
                    className="w-full bg-[#111724] border border-gray-700 rounded-lg px-3 py-2 text-gray-200 focus:outline-none focus:border-cyan-500"
                  />
                  <span className="text-[10px] text-gray-500 block mt-1">
                    預設 65,011,712 輪，為 OpenPGP RFC 4880 標準上限。
                  </span>
                </div>
              </div>
            )}

            <div className="flex items-center gap-2 pt-1">
              <input
                type="checkbox"
                id="upload-check"
                checked={uploadToGist}
                onChange={(e) => setUploadToGist(e.target.checked)}
                className="w-4 h-4 rounded bg-[#111724] border-gray-700 text-cyan-500"
              />
              <label htmlFor="upload-check" className="text-gray-300 text-xs flex items-center gap-1">
                <UploadCloud className="w-3.5 h-3.5 text-cyan-400" />
                <span>加密完成後直接上傳至雲端 Gist 倉庫 (-u)</span>
              </label>
            </div>

            <button
              type="submit"
              disabled={executing}
              className="w-full py-2.5 bg-gradient-to-r from-cyan-600 to-emerald-600 hover:from-cyan-500 hover:to-emerald-500 text-white font-bold rounded-lg transition shadow flex items-center justify-center gap-2"
            >
              {executing ? (
                <RefreshCw className="w-4 h-4 animate-spin" />
              ) : (
                <Lock className="w-4 h-4" />
              )}
              <span>執行檔案加密並存檔歸檔簿 (a -p)</span>
            </button>
          </form>
        </div>

        {/* Right Column: Execution Output & Quick Layer Unwrapper */}
        <div className="lg:col-span-6 bg-[#0b1019] border border-cyan-900/40 rounded-2xl p-5 shadow-xl flex flex-col justify-between space-y-4">
          <div>
            <div className="flex items-center gap-2 text-emerald-400 border-b border-gray-800 pb-3">
              <FileCheck className="w-4 h-4" />
              <h3 className="font-bold text-gray-200">
                執行反饋與層級解開 (命令: a -x)
              </h3>
            </div>

            <div className="pt-3 space-y-3">
              <div className="p-3 bg-[#070b12] border border-gray-800/80 rounded-xl min-h-[140px] text-[11px] font-mono whitespace-pre-wrap text-cyan-200 overflow-x-auto">
                {execResult || (
                  <span className="text-gray-500">
                    等待加密指令觸發... 執行 a -p 或 a -x 的即時審計輸出將顯示於此。
                  </span>
                )}
              </div>

              {/* Decrypt Quick Box */}
              <div className="p-3 bg-[#0d131f] border border-gray-800 rounded-xl space-y-2">
                <div className="text-[11px] font-bold text-gray-300 flex items-center gap-1.5">
                  <ArrowDownToLine className="w-3.5 h-3.5 text-emerald-400" />
                  <span>還原一層巢狀封裝 (a -x):</span>
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                  <input
                    type="text"
                    placeholder="密文檔名 (如 1.gpg.gpg)"
                    value={decryptTarget}
                    onChange={(e) => setDecryptTarget(e.target.value)}
                    className="bg-[#111724] border border-gray-700 rounded px-2 py-1 text-xs text-gray-200 focus:outline-none focus:border-cyan-500"
                  />
                  <input
                    type="password"
                    placeholder="若為密碼加密請輸入"
                    value={decryptPass}
                    onChange={(e) => setDecryptPass(e.target.value)}
                    className="bg-[#111724] border border-gray-700 rounded px-2 py-1 text-xs text-gray-200 focus:outline-none focus:border-cyan-500"
                  />
                </div>
                <button
                  type="button"
                  onClick={() => handleUnwrapLayer(decryptTarget)}
                  disabled={!decryptTarget.trim() || executing}
                  className="w-full py-1.5 bg-[#161c28] hover:bg-[#222b3d] text-emerald-300 border border-emerald-800/60 rounded text-xs font-semibold transition"
                >
                  解密剝離一層 (.gpg)
                </button>
              </div>
            </div>
          </div>

          <div className="text-[11px] text-gray-400 border-t border-gray-800 pt-3 flex items-center justify-between">
            <span>底層驅動: Rust 原生編譯器 (/usr/local/bin/a)</span>
            <span className="text-cyan-400">合規審計: 已啟用</span>
          </div>
        </div>
      </div>

      {/* Manifest Table: Key Ledger Records */}
      <div className="bg-[#0b1019] border border-cyan-900/40 rounded-2xl p-5 shadow-2xl space-y-4">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2 border-b border-gray-800 pb-3">
          <div className="flex items-center gap-2 text-cyan-400">
            <Layers className="w-4 h-4" />
            <h3 className="font-bold text-gray-200">
              檔案金鑰歸檔審計明細清單 (Key Ledger Records)
            </h3>
          </div>
          <span className="text-[11px] text-gray-400">
            歸檔日誌存檔路徑: ~/.config/a/key_ledger.json
          </span>
        </div>

        {records.length === 0 ? (
          <div className="text-center py-10 text-gray-500 space-y-2">
            <ShieldAlert className="w-8 h-8 mx-auto text-gray-600" />
            <p>目前歸檔簿中尚無加密記錄。</p>
            <p className="text-xs text-gray-600">
              您可以透過「檔案加密工作台」或在終端執行 a -p 1.txt 開始封裝。
            </p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs border-collapse">
              <thead>
                <tr className="border-b border-gray-800 text-gray-400">
                  <th className="py-2.5 px-3">檔案名稱</th>
                  <th className="py-2.5 px-3">封裝層級</th>
                  <th className="py-2.5 px-3">鎖定金鑰 ID / 加密模式</th>
                  <th className="py-2.5 px-3">S2K 迭代運算</th>
                  <th className="py-2.5 px-3">SHA-256 校驗值</th>
                  <th className="py-2.5 px-3">大小</th>
                  <th className="py-2.5 px-3">時間戳記</th>
                  <th className="py-2.5 px-3 text-right">操作</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-800/60 text-gray-300">
                {records.map((entry) => (
                  <tr key={entry.id} className="hover:bg-[#111724] transition">
                    <td className="py-2.5 px-3 font-bold text-cyan-300 flex items-center gap-1.5">
                      <FileCode className="w-3.5 h-3.5 text-gray-400" />
                      <span>{entry.file_name}</span>
                    </td>
                    <td className="py-2.5 px-3">
                      <span className="px-2 py-0.5 rounded-full text-[10px] font-bold bg-cyan-950 text-cyan-400 border border-cyan-800">
                        Layer {entry.layer}
                      </span>
                    </td>
                    <td className="py-2.5 px-3">
                      <span className="font-mono text-gray-300 truncate block max-w-[200px]" title={entry.key_id}>
                        {entry.key_id}
                      </span>
                    </td>
                    <td className="py-2.5 px-3">
                      {entry.s2k_iterations > 0 ? (
                        <span className="text-purple-400 font-bold">
                          {entry.s2k_iterations.toLocaleString()} 輪
                        </span>
                      ) : (
                        <span className="text-gray-500">非對稱公鑰</span>
                      )}
                    </td>
                    <td className="py-2.5 px-3 font-mono text-[11px] text-gray-400">
                      {entry.sha256 ? `${entry.sha256.slice(0, 16)}...` : '-'}
                    </td>
                    <td className="py-2.5 px-3 text-gray-400">
                      {(entry.bytes_len / 1024).toFixed(2)} KB
                    </td>
                    <td className="py-2.5 px-3 text-gray-400 text-[11px]">
                      {entry.timestamp ? entry.timestamp.slice(0, 19).replace('T', ' ') : '-'}
                    </td>
                    <td className="py-2.5 px-3 text-right">
                      <button
                        onClick={() => {
                          setDecryptTarget(entry.file_name);
                          handleUnwrapLayer(entry.file_name);
                        }}
                        className="px-2 py-1 bg-[#161c28] hover:bg-[#202737] text-cyan-300 border border-cyan-800/60 rounded text-[11px] transition"
                        title="還原解密一層"
                      >
                        解開一層
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
};
