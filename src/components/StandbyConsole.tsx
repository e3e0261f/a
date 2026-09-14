import React, { useState } from 'react';
import { Shield, Power, Terminal, ExternalLink, CheckCircle2, RefreshCw } from 'lucide-react';
import { RustStatus } from '../types';

interface StandbyConsoleProps {
  rustStatus: RustStatus | null;
  onActivate: () => void;
}

export const StandbyConsole: React.FC<StandbyConsoleProps> = ({
  rustStatus,
  onActivate,
}) => {
  const [isActivating, setIsActivating] = useState(false);

  const handleWake = async () => {
    setIsActivating(true);
    try {
      await fetch('/api/web/state', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ state: 'active' }),
      });
      onActivate();
    } catch {
      onActivate();
    } finally {
      setIsActivating(false);
    }
  };

  return (
    <div className="min-h-[80vh] flex flex-col items-center justify-center p-6 text-center font-mono">
      <div className="max-w-xl w-full bg-[#0c1017] border border-cyan-900/40 rounded-2xl p-8 shadow-2xl space-y-6 relative overflow-hidden">
        {/* Glow effect */}
        <div className="absolute -top-16 -left-16 w-48 h-48 bg-cyan-500/10 rounded-full blur-3xl pointer-events-none" />
        <div className="absolute -bottom-16 -right-16 w-48 h-48 bg-emerald-500/10 rounded-full blur-3xl pointer-events-none" />

        {/* Status Indicator Icon */}
        <div className="w-16 h-16 mx-auto rounded-2xl bg-[#111622] border border-gray-700 flex items-center justify-center text-gray-400 shadow-inner">
          <Power className="w-8 h-8 text-cyan-400/70" />
        </div>

        {/* Title & Subtitle */}
        <div className="space-y-2">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-cyan-950/80 border border-cyan-800/60 text-cyan-300 text-xs font-semibold">
            <span className="w-2 h-2 rounded-full bg-cyan-400 animate-pulse"></span>
            <span>網頁端管理引擎已進入待機關閉狀態</span>
          </div>
          <h2 className="text-xl font-bold text-gray-100 tracking-wide">
            Cyber-Forge a · 待機休眠模式
          </h2>
          <p className="text-xs text-gray-400 leading-relaxed max-w-md mx-auto">
            遵循您的核心設計規範：網頁端管理引擎可開可不開，預設為關閉。
            Rust 後台原生守護程序持續主導核心邏輯與命令運算。
          </p>
        </div>

        {/* Rust Background Core State */}
        <div className="p-4 bg-[#090d14] rounded-xl border border-gray-800 text-left space-y-2 text-xs">
          <div className="flex items-center justify-between text-gray-400 pb-2 border-b border-gray-800/80">
            <span className="flex items-center gap-1.5 text-gray-300">
              <Shield className="w-3.5 h-3.5 text-emerald-400" />
              <span>Rust 後台核心守護進程</span>
            </span>
            <span className="text-emerald-400 font-semibold flex items-center gap-1">
              <CheckCircle2 className="w-3 h-3" />
              <span>NORMAL (健康運行)</span>
            </span>
          </div>

          <div className="grid grid-cols-2 gap-2 pt-1 text-[11px]">
            <div>
              <span className="text-gray-500">二進制檔案: </span>
              <span className="text-gray-300">{rustStatus?.binaryPath || '/usr/local/bin/a'}</span>
            </div>
            <div>
              <span className="text-gray-500">本地筆記目錄: </span>
              <span className="text-cyan-400 truncate block">{rustStatus?.noteDir || '~/BOok/NOte'}</span>
            </div>
            <div>
              <span className="text-gray-500">GPG 公鑰指紋: </span>
              <span className="text-emerald-300 truncate block">{rustStatus?.keyId || '已配置'}</span>
            </div>
            <div>
              <span className="text-gray-500">Gist 同步錨點: </span>
              <span className="text-cyan-300 truncate block">{rustStatus?.gistId || '已綁定'}</span>
            </div>
          </div>
        </div>

        {/* CLI Invocation Guide */}
        <div className="p-3.5 bg-[#121620] rounded-xl border border-cyan-900/30 text-left space-y-1.5 text-xs">
          <div className="text-gray-400 font-semibold flex items-center gap-1.5">
            <Terminal className="w-3.5 h-3.5 text-cyan-400" />
            <span>原生命令列調度方式</span>
          </div>
          <div className="p-2 bg-[#070a10] rounded border border-gray-800/80 text-cyan-300 font-mono text-[11px] select-all">
            $ a --web &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;# 喚醒並開啟網頁端管理引擎<br />
            $ a --web status &nbsp;# 檢視網頁引擎狀態<br />
            $ a --web stop &nbsp;&nbsp;&nbsp;# 關閉網頁端服務 (設回待機模式)
          </div>
        </div>

        {/* Action Button to Wake Up Web Engine */}
        <div className="pt-2 flex flex-col sm:flex-row items-center justify-center gap-3">
          <button
            id="wake-web-engine-btn"
            onClick={handleWake}
            disabled={isActivating}
            className="w-full sm:w-auto px-6 py-2.5 bg-gradient-to-r from-cyan-600 to-cyan-500 hover:from-cyan-500 hover:to-cyan-400 text-white rounded-xl font-semibold text-xs transition flex items-center justify-center gap-2 shadow-lg shadow-cyan-950/50"
          >
            {isActivating ? (
              <RefreshCw className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <Power className="w-3.5 h-3.5" />
            )}
            <span>開啟網頁端管理引擎 (a --web)</span>
          </button>
        </div>
      </div>
    </div>
  );
};
