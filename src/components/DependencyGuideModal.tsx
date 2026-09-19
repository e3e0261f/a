import React, { useState, useEffect } from 'react';
import {
  HelpCircle,
  X,
  Terminal,
  Copy,
  Check,
  Server,
  Code2,
  ShieldCheck,
  FolderLock,
  ExternalLink,
  Cpu,
} from 'lucide-react';
import { DependencyDiagnostic } from '../types';

interface DependencyGuideModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export const DependencyGuideModal: React.FC<DependencyGuideModalProps> = ({
  isOpen,
  onClose,
}) => {
  const [diagnostic, setDiagnostic] = useState<DependencyDiagnostic | null>(null);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      fetch('/api/system/diagnostics')
        .then((res) => res.json())
        .then((data) => setDiagnostic(data))
        .catch(() => {});
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const copyToClipboard = (text: string, key: string) => {
    navigator.clipboard.writeText(text);
    setCopiedKey(key);
    setTimeout(() => setCopiedKey(null), 2000);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm font-mono text-xs">
      <div className="bg-[#0b1017] border border-cyan-800/80 rounded-2xl w-full max-w-3xl max-h-[90vh] overflow-y-auto shadow-2xl flex flex-col">
        {/* Modal Header */}
        <div className="p-4 bg-[#111724] border-b border-gray-800 flex items-center justify-between sticky top-0 z-10">
          <div className="flex items-center gap-2.5">
            <div className="p-1.5 bg-cyan-950 border border-cyan-700/60 rounded-lg text-cyan-400">
              <HelpCircle className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-bold text-gray-100 tracking-wide">
                系統依賴科普與安裝指引 (tsx / express 插件診斷)
              </h2>
              <p className="text-[11px] text-gray-400">
                Cyber-NOte Web 管理引擎必備運作組件詳解與環境修復
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 text-gray-400 hover:text-gray-200 hover:bg-gray-800 rounded-lg transition"
            title="關閉"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Modal Body */}
        <div className="p-6 space-y-6 text-gray-300">
          {/* Status Alert Banner */}
          <div className="p-3.5 bg-cyan-950/40 border border-cyan-700/50 rounded-xl space-y-1">
            <div className="flex items-center justify-between">
              <span className="font-semibold text-cyan-300 flex items-center gap-1.5">
                <Cpu className="w-4 h-4 text-cyan-400" />
                <span>為什麼 Cyber-NOte 需要 tsx 與 express 插件？</span>
              </span>
              <span className="px-2 py-0.5 bg-emerald-950 text-emerald-400 border border-emerald-700/50 rounded text-[10px] font-bold">
                核心就緒
              </span>
            </div>
            <p className="text-[11px] text-gray-400 leading-relaxed">
              Cyber-NOte 是以 Rust 原生安全核心為基石的嚴格加密系統。為了在保有命令列絕對獨立性同時，提供視覺化金庫審計、金鑰歸檔簿查詢與 Gist 雲端雷達，系統採用 Node.js 最嚴謹的輕量級橋樑，因此需要以下兩個關鍵套件：
            </p>
          </div>

          {/* Educational Cards */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {/* Package 1: tsx */}
            <div className="p-4 bg-[#0e1420] border border-gray-800 rounded-xl space-y-2.5">
              <div className="flex items-center justify-between">
                <span className="font-bold text-emerald-400 flex items-center gap-1.5 text-xs">
                  <Code2 className="w-4 h-4" />
                  <span>1. tsx (TypeScript Execute)</span>
                </span>
                <span className="px-2 py-0.5 bg-emerald-950/80 text-emerald-300 border border-emerald-800 rounded text-[10px]">
                  執行環境
                </span>
              </div>
              <p className="text-[11px] text-gray-400 leading-relaxed">
                <strong className="text-gray-200">科普：</strong>Node.js 原生只支援執行標準 JavaScript，無法直接運行 TypeScript (.ts)。<code className="text-cyan-300">tsx</code> 基於高極速 esbuild 引擎，能在零延遲、不生成中間龐雜編譯檔案的情況下，直接執行 <code className="text-cyan-300">server.ts</code>，具備極低記憶體開銷與毫秒級啟動特性。
              </p>
              <div className="pt-1">
                <div className="text-[10px] text-gray-500 mb-1">若未安裝，啟動時將報錯：</div>
                <div className="p-2 bg-[#080c13] rounded border border-rose-900/40 text-rose-300 text-[10px] font-mono">
                  sh: 1: tsx: not found
                </div>
              </div>
            </div>

            {/* Package 2: express */}
            <div className="p-4 bg-[#0e1420] border border-gray-800 rounded-xl space-y-2.5">
              <div className="flex items-center justify-between">
                <span className="font-bold text-cyan-400 flex items-center gap-1.5 text-xs">
                  <Server className="w-4 h-4" />
                  <span>2. express (Web API 框架)</span>
                </span>
                <span className="px-2 py-0.5 bg-cyan-950/80 text-cyan-300 border border-cyan-800 rounded text-[10px]">
                  HTTP 閘道
                </span>
              </div>
              <p className="text-[11px] text-gray-400 leading-relaxed">
                <strong className="text-gray-200">科普：</strong>Express 是 Node.js 生態系中成熟可靠的輕量級 HTTP 伺服器框架。Cyber-NOte 嚴格杜絕雲端明文洩漏，Express 在本地 127.0.0.1 提供隔離式 REST API，負責調度 GPG 子行程對稱/非對稱加密，以及將密文安全 PATCH 至 GitHub Gist。
              </p>
              <div className="pt-1">
                <div className="text-[10px] text-gray-500 mb-1">若未安裝，加載時將報錯：</div>
                <div className="p-2 bg-[#080c13] rounded border border-rose-900/40 text-rose-300 text-[10px] font-mono">
                  Error: Cannot find module 'express'
                </div>
              </div>
            </div>
          </div>

          {/* Installation Commands Guide */}
          <div className="space-y-3">
            <h3 className="text-xs font-bold text-gray-200 flex items-center gap-1.5">
              <Terminal className="w-3.5 h-3.5 text-cyan-400" />
              <span>依賴插件安裝命令手冊 (終端機一鍵執行)</span>
            </h3>

            {/* Command 1: Global npm install */}
            <div className="p-3 bg-[#080c13] border border-gray-800 rounded-xl space-y-1.5">
              <div className="flex items-center justify-between text-[11px]">
                <span className="text-gray-400">方案 A：全域安裝（推薦，全系統使用者均可隨處調用 a -w）</span>
                <button
                  onClick={() => copyToClipboard('npm install -g tsx express', 'global')}
                  className="px-2 py-0.5 bg-gray-800 hover:bg-gray-700 text-gray-200 rounded text-[10px] flex items-center gap-1 transition"
                >
                  {copiedKey === 'global' ? (
                    <>
                      <Check className="w-3 h-3 text-emerald-400" />
                      <span className="text-emerald-400">已複製</span>
                    </>
                  ) : (
                    <>
                      <Copy className="w-3 h-3" />
                      <span>複製</span>
                    </>
                  )}
                </button>
              </div>
              <pre className="p-2 bg-black/60 rounded text-cyan-300 text-[11px] overflow-x-auto select-all">
                npm install -g tsx express
              </pre>
            </div>

            {/* Command 2: Local project install */}
            <div className="p-3 bg-[#080c13] border border-gray-800 rounded-xl space-y-1.5">
              <div className="flex items-center justify-between text-[11px]">
                <span className="text-gray-400">方案 B：項目本地依賴安裝（容器或開發者專用）</span>
                <button
                  onClick={() => copyToClipboard('npm install', 'project')}
                  className="px-2 py-0.5 bg-gray-800 hover:bg-gray-700 text-gray-200 rounded text-[10px] flex items-center gap-1 transition"
                >
                  {copiedKey === 'project' ? (
                    <>
                      <Check className="w-3 h-3 text-emerald-400" />
                      <span className="text-emerald-400">已複製</span>
                    </>
                  ) : (
                    <>
                      <Copy className="w-3 h-3" />
                      <span>複製</span>
                    </>
                  )}
                </button>
              </div>
              <pre className="p-2 bg-black/60 rounded text-cyan-300 text-[11px] overflow-x-auto select-all">
                npm install
              </pre>
            </div>

            {/* Command 3: System prerequisites */}
            <div className="p-3 bg-[#080c13] border border-gray-800 rounded-xl space-y-1.5">
              <div className="flex items-center justify-between text-[11px]">
                <span className="text-gray-400">系統底層必備套件（若 Linux 尚未安裝 Node.js 與 GnuPG）</span>
                <button
                  onClick={() => copyToClipboard('sudo apt update && sudo apt install -y nodejs npm gnupg', 'apt')}
                  className="px-2 py-0.5 bg-gray-800 hover:bg-gray-700 text-gray-200 rounded text-[10px] flex items-center gap-1 transition"
                >
                  {copiedKey === 'apt' ? (
                    <>
                      <Check className="w-3 h-3 text-emerald-400" />
                      <span className="text-emerald-400">已複製</span>
                    </>
                  ) : (
                    <>
                      <Copy className="w-3 h-3" />
                      <span>複製</span>
                    </>
                  )}
                </button>
              </div>
              <pre className="p-2 bg-black/60 rounded text-cyan-300 text-[11px] overflow-x-auto select-all">
                sudo apt update && sudo apt install -y nodejs npm gnupg
              </pre>
            </div>
          </div>

          {/* Architecture & Path Specifications */}
          <div className="p-4 bg-[#0d131e] border border-gray-800 rounded-xl space-y-3">
            <h3 className="text-xs font-bold text-gray-200 flex items-center gap-1.5">
              <FolderLock className="w-3.5 h-3.5 text-emerald-400" />
              <span>Cyber-NOte 系統架構與路徑規範核查</span>
            </h3>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-[11px]">
              <div className="p-2.5 bg-[#080c13] rounded border border-gray-800 space-y-1">
                <div className="text-gray-400">預設存儲目錄 (標準 Linux XDG)</div>
                <div className="text-cyan-300 font-semibold truncate">
                  ~/.local/share/cyber-note/notes
                </div>
                <div className="text-[10px] text-gray-500">所有標準 Linux 發行版原生支援</div>
              </div>
              <div className="p-2.5 bg-[#080c13] rounded border border-gray-800 space-y-1">
                <div className="text-gray-400">隱私數據隔離 (Token 憑證)</div>
                <div className="text-amber-300 font-semibold truncate">
                  ~/.config/cyber-note/secrets/token.gpg
                </div>
                <div className="text-[10px] text-gray-500">權限隔離 0700 / 0600</div>
              </div>
              <div className="p-2.5 bg-[#080c13] rounded border border-gray-800 space-y-1">
                <div className="text-gray-400">金鑰安全策略</div>
                <div className="text-emerald-400 font-semibold">嚴格鎖定 GPG，拒絕 SSH 金鑰</div>
                <div className="text-[10px] text-gray-500">全流程稽核存檔於 Key Ledger</div>
              </div>
              <div className="p-2.5 bg-[#080c13] rounded border border-gray-800 space-y-1">
                <div className="text-gray-400">倉庫歷史記錄抹除</div>
                <div className="text-cyan-300 font-semibold">a --migrate-repo [--delete-old]</div>
                <div className="text-[10px] text-gray-500">新建 Gist 倉庫徹底切斷 Git 歷史</div>
              </div>
            </div>
          </div>
        </div>

        {/* Modal Footer */}
        <div className="p-4 bg-[#111724] border-t border-gray-800 flex justify-end">
          <button
            onClick={onClose}
            className="px-4 py-2 bg-cyan-600 hover:bg-cyan-500 text-white rounded-lg text-xs font-semibold transition"
          >
            完成閱畢關閉
          </button>
        </div>
      </div>
    </div>
  );
};
