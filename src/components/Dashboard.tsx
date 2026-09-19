import React, { useState } from 'react';
import {
  Shield,
  Key,
  Cloud,
  Folder,
  Lock,
  Terminal as TerminalIcon,
  FileText,
  Zap,
  ArrowRight,
  Power,
  Server,
  Activity,
  CheckCircle2,
  RefreshCw,
  HelpCircle,
  FolderLock,
  History,
} from 'lucide-react';
import { AppConfig, NoteFile, RustStatus } from '../types';

interface DashboardProps {
  config: AppConfig;
  notes: Record<string, NoteFile>;
  rustStatus: RustStatus | null;
  onNavigate: (tab: 'dashboard' | 'ledger' | 'notes' | 'radar' | 'terminal') => void;
  onOpenWizard: () => void;
  onToggleWebEngine: (targetState: 'active' | 'standby') => void;
  onOpenDependencyGuide?: () => void;
}

export const Dashboard: React.FC<DashboardProps> = ({
  config,
  notes,
  rustStatus,
  onNavigate,
  onOpenWizard,
  onToggleWebEngine,
  onOpenDependencyGuide,
}) => {
  const currentYear = new Date().getFullYear().toString();
  const defaultFileName = `${currentYear}.note.gpg`;
  const currentYearNote = notes[defaultFileName];
  const fileKeys = Object.keys(notes);
  const [isSwitching, setIsSwitching] = useState(false);

  const totalLines = currentYearNote?.decryptedContent
    ? currentYearNote.decryptedContent.split('\n').filter((l) => l.trim().length > 0).length
    : 0;

  const handleToggle = async (mode: 'active' | 'standby') => {
    setIsSwitching(true);
    try {
      await onToggleWebEngine(mode);
    } finally {
      setIsSwitching(false);
    }
  };

  const defaultStandardDir = '~/.local/share/cyber-note/notes';
  const displayDir = config.noteDir && config.noteDir !== '~/BOok/NOte' ? config.noteDir : defaultStandardDir;

  return (
    <div className="space-y-6 font-mono">
      {/* Dual-Engine Architecture HUD Card */}
      <div className="bg-[#0b1019] border border-cyan-900/60 rounded-2xl p-5 shadow-2xl relative overflow-hidden">
        <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4 border-b border-gray-800/80 pb-4">
          <div className="space-y-1">
            <div className="flex items-center gap-2 text-cyan-400">
              <Server className="w-5 h-5" />
              <h2 className="text-base font-bold text-gray-100 tracking-wide">
                Cyber-NOte 雙引擎架構 (Rust Native Core & JS Web Engine)
              </h2>
            </div>
            <p className="text-xs text-gray-400">
              原生 Rust 掌控終端機命令與底層 GPG 加密密封，Node.js Web 引擎承載可選式審計管理介面。
            </p>
          </div>

          {/* Web Engine Power Switch & Diagnostics Button */}
          <div className="flex items-center gap-2">
            {onOpenDependencyGuide && (
              <button
                id="open-dep-guide-btn"
                onClick={onOpenDependencyGuide}
                className="px-3 py-1.5 bg-[#141b29] hover:bg-[#1f293d] text-cyan-300 border border-cyan-800/60 rounded-xl text-xs font-semibold transition flex items-center gap-1.5 shadow-sm"
                title="查看 tsx 與 express 插件科普與依賴安裝說明"
              >
                <HelpCircle className="w-3.5 h-3.5 text-cyan-400" />
                <span>依賴科普 (tsx/express)</span>
              </button>
            )}

            <div className="flex items-center gap-3 bg-[#111724] border border-gray-700/60 rounded-xl p-2 px-3">
              <div className="text-right text-[11px]">
                <div className="text-gray-400">網頁管理引擎</div>
                <div className="text-emerald-400 font-bold flex items-center justify-end gap-1">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
                  <span>運行中 (ACTIVE)</span>
                </div>
              </div>

              <button
                id="power-off-web-btn"
                onClick={() => handleToggle('standby')}
                disabled={isSwitching}
                className="px-3 py-1.5 bg-rose-950/70 hover:bg-rose-900/80 text-rose-300 border border-rose-700/50 rounded-lg text-xs font-semibold transition flex items-center gap-1.5 shadow-sm"
                title="切換為待機模式（預設關閉）"
              >
                {isSwitching ? (
                  <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <Power className="w-3.5 h-3.5" />
                )}
                <span>設為待機</span>
              </button>
            </div>
          </div>
        </div>

        {/* Dual Core Status Grid */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mt-4 text-xs">
          {/* Engine A: Rust Native */}
          <div className="p-3.5 bg-[#0e1420] border border-emerald-900/40 rounded-xl space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-emerald-400 font-semibold flex items-center gap-1.5">
                <Shield className="w-4 h-4" />
                <span>核心程式：Rust 原生命令 (a)</span>
              </span>
              <span className="px-2 py-0.5 bg-emerald-950 text-emerald-300 border border-emerald-700/50 rounded text-[10px] font-bold">
                GPG 鎖定模式
              </span>
            </div>
            <p className="text-[11px] text-gray-400 leading-relaxed">
              嚴格只認可指定 GPG 公鑰，拒絕 SSH 密鑰加密；支援多層巢狀加密 (a -p) 與 S2K 防窮舉。
            </p>
            <div className="p-2 bg-[#080c13] rounded border border-gray-800/80 text-[11px] text-gray-300 flex items-center justify-between">
              <span>命令位置: <code className="text-cyan-400">/usr/local/bin/a</code></span>
              <span className="text-emerald-400 flex items-center gap-1">
                <CheckCircle2 className="w-3 h-3" />
                <span>原生直接調用</span>
              </span>
            </div>
          </div>

          {/* Engine B: JS Web */}
          <div className="p-3.5 bg-[#0e1420] border border-cyan-900/40 rounded-xl space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-cyan-400 font-semibold flex items-center gap-1.5">
                <Activity className="w-4 h-4" />
                <span>可選面板：Express & Vite 輕量閘道</span>
              </span>
              <span className="px-2 py-0.5 bg-cyan-950 text-cyan-300 border border-cyan-700/50 rounded text-[10px] font-bold">
                OPTIONAL (按需喚醒)
              </span>
            </div>
            <p className="text-[11px] text-gray-400 leading-relaxed">
              提供金鑰歸檔簿審計、歷史抹除遷移與多層加套加密。平時待機，執行 a -w 時喚醒。
            </p>
            <div className="p-2 bg-[#080c13] rounded border border-gray-800/80 text-[11px] text-gray-300 flex items-center justify-between">
              <span>喚醒指令: <code className="text-cyan-400">a -w</code></span>
              <span className="text-cyan-400">Port: 3000</span>
            </div>
          </div>
        </div>
      </div>

      {/* Cyber-NOte Main Dashboard Details Card */}
      <div className="bg-[#0c1017] border border-cyan-800/40 rounded-2xl p-6 shadow-2xl relative overflow-hidden">
        <div className="absolute -right-12 -bottom-12 w-64 h-64 bg-cyan-500/5 rounded-full blur-3xl pointer-events-none" />

        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 border-b border-gray-800 pb-5">
          <div className="space-y-1">
            <div className="flex items-center gap-2 text-cyan-400">
              <Shield className="w-5 h-5" />
              <h1 className="text-lg font-bold tracking-wide text-gray-100">
                Cyber-NOte 系統儀表板
              </h1>
            </div>
            <p className="text-xs text-gray-400">
              鎖定 GPG 金鑰隔離 · 嚴禁 SSH 密鑰 · 多層巢狀加密 · 倉庫抹除遷移 (Project a)
            </p>
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={() => onNavigate('ledger')}
              className="px-3.5 py-1.5 bg-[#171d29] hover:bg-[#222a3a] text-cyan-300 border border-cyan-700/50 rounded-lg text-xs transition flex items-center gap-1.5 shadow-sm"
            >
              <Lock className="w-3.5 h-3.5 text-cyan-400" />
              <span>金鑰歸檔 (a -p)</span>
            </button>
            <button
              onClick={onOpenWizard}
              className="px-3.5 py-1.5 bg-[#171d29] hover:bg-[#222a3a] text-cyan-300 border border-cyan-700/50 rounded-lg text-xs transition flex items-center gap-1.5 shadow-sm"
            >
              <Zap className="w-3.5 h-3.5 text-cyan-400" />
              <span>配置精靈 (a --init)</span>
            </button>
            <button
              onClick={() => onNavigate('terminal')}
              className="px-3.5 py-1.5 bg-cyan-600 hover:bg-cyan-500 text-white rounded-lg text-xs font-semibold transition flex items-center gap-1.5 shadow-sm"
            >
              <TerminalIcon className="w-3.5 h-3.5" />
              <span>終端控制 (CLI)</span>
            </button>
          </div>
        </div>

        {/* 4 Core Pillars Grid */}
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 mt-6">
          <div className="p-4 bg-[#111622] border border-gray-800 rounded-xl space-y-1">
            <div className="text-[11px] text-gray-400 flex items-center gap-1.5">
              <Folder className="w-3.5 h-3.5 text-cyan-400" />
              <span>📂 標準存儲目錄 (XDG)</span>
            </div>
            <div className="text-xs font-semibold text-gray-200 truncate" title={displayDir}>
              {displayDir}
            </div>
            <div className="text-[10px] text-gray-500">標準 Linux 目錄，落盤即 GPG 密文</div>
          </div>

          <div className="p-4 bg-[#111622] border border-gray-800 rounded-xl space-y-1">
            <div className="text-[11px] text-gray-400 flex items-center gap-1.5">
              <Key className="w-3.5 h-3.5 text-emerald-400" />
              <span>🔑 鎖定 GPG 金鑰 ID</span>
            </div>
            <div className="text-xs font-semibold text-emerald-300 truncate" title={config.gpgKeyId}>
              {config.gpgKeyId || '未配置'}
            </div>
            <div className="text-[10px] text-gray-500">嚴禁 SSH 密鑰，嚴格鎖定 GPG</div>
          </div>

          <div className="p-4 bg-[#111622] border border-gray-800 rounded-xl space-y-1">
            <div className="text-[11px] text-gray-400 flex items-center gap-1.5">
              <Cloud className="w-3.5 h-3.5 text-cyan-400" />
              <span>🌐 雲端 Gist ID</span>
            </div>
            <div className="text-xs font-semibold text-cyan-300 truncate" title={config.gistId}>
              {config.gistId || '未配置'}
            </div>
            <div className="text-[10px] text-gray-500">支援 a --migrate-repo 抹除歷史</div>
          </div>

          <div className="p-4 bg-[#111622] border border-gray-800 rounded-xl space-y-1">
            <div className="text-[11px] text-gray-400 flex items-center gap-1.5">
              <FolderLock className="w-3.5 h-3.5 text-amber-400" />
              <span>🛡️ 隱私數據隔離區</span>
            </div>
            <div className="text-xs font-semibold text-amber-300 truncate" title="~/.config/cyber-note/secrets/token.gpg">
              secrets/token.gpg
            </div>
            <div className="text-[10px] text-gray-500">POSIX 0700/0600 隔離加密存放</div>
          </div>
        </div>
      </div>

      {/* Metrics & Quick Links */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        {/* Card 1: Vault Overview */}
        <div className="bg-[#0c1017] border border-gray-800 rounded-xl p-5 space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-gray-200 text-sm font-semibold">
              <FileText className="w-4 h-4 text-cyan-400" />
              <span>靈感金庫概況</span>
            </div>
            <button
              onClick={() => onNavigate('notes')}
              className="text-xs text-cyan-400 hover:underline flex items-center gap-0.5"
            >
              <span>查看全部</span>
              <ArrowRight className="w-3 h-3" />
            </button>
          </div>

          <div className="space-y-3 pt-1">
            <div className="flex justify-between items-center text-xs py-1.5 border-b border-gray-800/80">
              <span className="text-gray-400">當前年度包裹:</span>
              <span className="text-gray-200 font-bold">{defaultFileName}</span>
            </div>
            <div className="flex justify-between items-center text-xs py-1.5 border-b border-gray-800/80">
              <span className="text-gray-400">今年靈感行數:</span>
              <span className="text-emerald-400 font-bold">{totalLines} 行</span>
            </div>
            <div className="flex justify-between items-center text-xs py-1.5 border-b border-gray-800/80">
              <span className="text-gray-400">本地檔案總數:</span>
              <span className="text-cyan-400 font-bold">{fileKeys.length} 個</span>
            </div>
            <div className="flex justify-between items-center text-xs py-1.5">
              <span className="text-gray-400">渲染模式:</span>
              <span className="text-emerald-400 font-bold">自適應奇偶雙色 (Green/Cyan)</span>
            </div>
          </div>

          <button
            onClick={() => onNavigate('notes')}
            className="w-full py-2 bg-[#161c28] hover:bg-[#202737] text-gray-200 rounded-lg text-xs transition border border-gray-700/60 font-medium"
          >
            開啟筆記閱讀器 (a -a)
          </button>
        </div>

        {/* Card 2: Cloud Radar */}
        <div className="bg-[#0c1017] border border-gray-800 rounded-xl p-5 space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-gray-200 text-sm font-semibold">
              <Cloud className="w-4 h-4 text-cyan-400" />
              <span>Gist 雲端安全引渡</span>
            </div>
            <button
              onClick={() => onNavigate('radar')}
              className="text-xs text-cyan-400 hover:underline flex items-center gap-0.5"
            >
              <span>掃描雷達</span>
              <ArrowRight className="w-3 h-3" />
            </button>
          </div>

          <div className="space-y-3 pt-1 text-xs text-gray-400 leading-relaxed">
            <p>
              透過 GitHub REST API 進行局部 PATCH 更新，不同年份與外部檔案獨立並存，絕不互相覆蓋。
            </p>
            <div className="p-3 bg-[#111622] rounded-lg border border-gray-800 space-y-1 text-[11px]">
              <div>
                <span className="text-cyan-400">a -s</span> : 推送今年密文包裹
              </div>
              <div>
                <span className="text-cyan-400">a -l</span> : 掃描雲端物資清單
              </div>
              <div>
                <span className="text-cyan-400">a -d [file] -x</span> : 下載並破甲解密
              </div>
            </div>
          </div>

          <button
            onClick={() => onNavigate('radar')}
            className="w-full py-2 bg-[#161c28] hover:bg-[#202737] text-gray-200 rounded-lg text-xs transition border border-gray-700/60 font-medium"
          >
            進入雲端雷達 (a -l)
          </button>
        </div>

        {/* Card 3: Command Contract */}
        <div className="bg-[#0c1017] border border-gray-800 rounded-xl p-5 space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-gray-200 text-sm font-semibold">
              <TerminalIcon className="w-4 h-4 text-emerald-400" />
              <span>指令合約快速查閱</span>
            </div>
            <button
              onClick={() => onNavigate('terminal')}
              className="text-xs text-emerald-400 hover:underline flex items-center gap-0.5"
            >
              <span>終端控制台</span>
              <ArrowRight className="w-3 h-3" />
            </button>
          </div>

          <div className="space-y-2 text-xs">
            <div className="flex items-center justify-between p-1.5 bg-[#111622] rounded border border-gray-800">
              <span className="text-cyan-400 font-bold">a [內容]</span>
              <span className="text-gray-400 text-[11px]">寫入並加密封存</span>
            </div>
            <div className="flex items-center justify-between p-1.5 bg-[#111622] rounded border border-gray-800">
              <span className="text-emerald-400 font-bold">a -a</span>
              <span className="text-gray-400 text-[11px]">雙色交替列印今年</span>
            </div>
            <div className="flex items-center justify-between p-1.5 bg-[#111622] rounded border border-gray-800">
              <span className="text-cyan-300 font-bold">a -w</span>
              <span className="text-gray-400 text-[11px]">啟動網頁端管理引擎</span>
            </div>
            <div className="flex items-center justify-between p-1.5 bg-[#111622] rounded border border-gray-800">
              <span className="text-amber-400 font-bold">a -r [關鍵字]</span>
              <span className="text-gray-400 text-[11px]">行級過濾重密存盤</span>
            </div>
          </div>

          <button
            onClick={() => onNavigate('terminal')}
            className="w-full py-2 bg-cyan-600 hover:bg-cyan-500 text-white rounded-lg text-xs transition font-semibold shadow-md"
          >
            啟動互動式終端
          </button>
        </div>
      </div>
    </div>
  );
};
