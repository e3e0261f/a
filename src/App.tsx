import React, { useState, useEffect } from 'react';
import {
  Shield,
  FileText,
  Cloud,
  Terminal as TerminalIcon,
  Settings,
  Lock,
  Power,
  Server,
  RefreshCw,
  CheckCircle2,
} from 'lucide-react';
import { AppConfig, NoteFile, RustStatus, WebEngineMode } from './types';
import { loadAppConfig, initStorage } from './lib/storage';
import { Dashboard } from './components/Dashboard';
import { NoteViewer } from './components/NoteViewer';
import { GistRadar } from './components/GistRadar';
import { Terminal } from './components/Terminal';
import { ConfigWizard } from './components/ConfigWizard';
import { StandbyConsole } from './components/StandbyConsole';
import { KeyLedgerView } from './components/KeyLedgerView';
import { DependencyGuideModal } from './components/DependencyGuideModal';
import { HelpCircle } from 'lucide-react';

type ActiveTab = 'dashboard' | 'ledger' | 'notes' | 'radar' | 'terminal';

export function App() {
  const [config, setConfig] = useState<AppConfig>(loadAppConfig());
  const [notes, setNotes] = useState<Record<string, NoteFile>>({});
  const [activeTab, setActiveTab] = useState<ActiveTab>('dashboard');
  const [showWizard, setShowWizard] = useState<boolean>(false);
  const [showDepGuide, setShowDepGuide] = useState<boolean>(false);
  const [isInitialized, setIsInitialized] = useState<boolean>(false);
  const [rustStatus, setRustStatus] = useState<RustStatus | null>(null);
  const [webEngineMode, setWebEngineMode] = useState<WebEngineMode>('active');

  // Fetch Rust Backend & Web Engine Status
  const refreshRustStatus = async () => {
    try {
      const res = await fetch('/api/rust/status');
      if (res.ok) {
        const data: RustStatus = await res.json();
        setRustStatus(data);
        if (data.webState) {
          setWebEngineMode(data.webState);
        }
      }
    } catch {
      // Offline fallback
    }
  };

  // Initialize vault storage on mount
  useEffect(() => {
    async function boot() {
      const initialNotes = await initStorage(config);
      setNotes(initialNotes);
      setIsInitialized(true);
      await refreshRustStatus();
    }
    boot();
  }, []);

  const handleConfigUpdate = (newConfig: AppConfig) => {
    setConfig(newConfig);
    refreshRustStatus();
  };

  const handleNotesUpdate = (newNotes: Record<string, NoteFile>) => {
    setNotes(newNotes);
    refreshRustStatus();
  };

  const handleToggleWebEngine = async (mode: WebEngineMode) => {
    setWebEngineMode(mode);
    try {
      await fetch('/api/web/state', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ state: mode }),
      });
      await refreshRustStatus();
    } catch {
      // Handled
    }
  };

  return (
    <div className="min-h-screen bg-[#070a10] text-gray-100 flex flex-col selection:bg-cyan-500/20 selection:text-cyan-200 font-mono">
      {/* Top Navigation & Status Bar */}
      <header className="border-b border-cyan-950/80 bg-[#0c1018]/90 backdrop-blur sticky top-0 z-40">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 h-16 flex items-center justify-between">
          {/* Logo & Name */}
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-cyan-950 to-emerald-950 border border-cyan-500/40 flex items-center justify-center text-cyan-400 shadow-[0_0_15px_rgba(6,182,212,0.15)]">
              <Shield className="w-5 h-5" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <span className="text-base font-bold text-gray-100 tracking-wide">
                  Cyber-NOte
                </span>
                <span className="px-1.5 py-0.2 text-[10px] font-bold bg-cyan-950 text-cyan-400 border border-cyan-700/60 rounded">
                  v0.0.4
                </span>
                {/* Rust Backend Live Indicator */}
                <div className="hidden sm:flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-[#0e1622] border border-emerald-900/60 text-[10px] text-emerald-400">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
                  <span>GPG 鎖定審計</span>
                </div>
              </div>
              <p className="text-[11px] text-gray-500 hidden sm:block">
                鎖定 GPG 金鑰隔離 · 嚴禁 SSH 密鑰 · 多層巢狀加密 · S2K 防窮舉加固
              </p>
            </div>
          </div>

          {/* Navigation & Controls */}
          <div className="flex items-center gap-2 sm:gap-3 text-xs">
            {/* Engine Mode Pill / Quick Switch */}
            <button
              id="header-engine-mode-btn"
              onClick={() => handleToggleWebEngine(webEngineMode === 'active' ? 'standby' : 'active')}
              className={`px-2.5 py-1.5 rounded-lg border text-[11px] flex items-center gap-1.5 transition ${
                webEngineMode === 'active'
                  ? 'bg-emerald-950/60 border-emerald-700/50 text-emerald-300 hover:bg-rose-950/60 hover:text-rose-300 hover:border-rose-700/50'
                  : 'bg-yellow-950/60 border-yellow-700/50 text-yellow-300 hover:bg-emerald-950/60 hover:text-emerald-300'
              }`}
              title="點擊切換網頁引擎狀態 (Active / Standby 待機關閉)"
            >
              <Power className="w-3 h-3" />
              <span className="hidden md:inline">網頁引擎:</span>
              <span>{webEngineMode === 'active' ? '運行中' : '待機關閉'}</span>
            </button>

            {webEngineMode === 'active' && (
              <nav className="flex items-center gap-1 sm:gap-1.5 text-xs">
                <button
                  id="tab-dashboard-btn"
                  onClick={() => setActiveTab('dashboard')}
                  className={`px-3 py-2 rounded-lg transition flex items-center gap-1.5 ${
                    activeTab === 'dashboard'
                      ? 'bg-cyan-950/90 text-cyan-300 border border-cyan-600/50 shadow-sm'
                      : 'text-gray-400 hover:text-gray-200 hover:bg-gray-800/60'
                  }`}
                >
                  <Shield className="w-3.5 h-3.5" />
                  <span className="hidden md:inline">系統狀態</span>
                </button>

                <button
                  id="tab-ledger-btn"
                  onClick={() => setActiveTab('ledger')}
                  className={`px-3 py-2 rounded-lg transition flex items-center gap-1.5 ${
                    activeTab === 'ledger'
                      ? 'bg-cyan-950/90 text-cyan-300 border border-cyan-600/50 shadow-sm'
                      : 'text-gray-400 hover:text-gray-200 hover:bg-gray-800/60'
                  }`}
                >
                  <Lock className="w-3.5 h-3.5" />
                  <span>金鑰歸檔 (a -p)</span>
                </button>

                <button
                  id="tab-notes-btn"
                  onClick={() => setActiveTab('notes')}
                  className={`px-3 py-2 rounded-lg transition flex items-center gap-1.5 ${
                    activeTab === 'notes'
                      ? 'bg-cyan-950/90 text-cyan-300 border border-cyan-600/50 shadow-sm'
                      : 'text-gray-400 hover:text-gray-200 hover:bg-gray-800/60'
                  }`}
                >
                  <FileText className="w-3.5 h-3.5" />
                  <span>機密文檔</span>
                </button>

                <button
                  id="tab-radar-btn"
                  onClick={() => setActiveTab('radar')}
                  className={`px-3 py-2 rounded-lg transition flex items-center gap-1.5 ${
                    activeTab === 'radar'
                      ? 'bg-cyan-950/90 text-cyan-300 border border-cyan-600/50 shadow-sm'
                      : 'text-gray-400 hover:text-gray-200 hover:bg-gray-800/60'
                  }`}
                >
                  <Cloud className="w-3.5 h-3.5" />
                  <span>雲端同步</span>
                </button>

                <button
                  id="tab-terminal-btn"
                  onClick={() => setActiveTab('terminal')}
                  className={`px-3 py-2 rounded-lg transition flex items-center gap-1.5 ${
                    activeTab === 'terminal'
                      ? 'bg-cyan-950/90 text-cyan-300 border border-cyan-600/50 shadow-sm'
                      : 'text-gray-400 hover:text-gray-200 hover:bg-gray-800/60'
                  }`}
                >
                  <TerminalIcon className="w-3.5 h-3.5" />
                  <span>終端控制台</span>
                </button>

                <button
                  id="open-dep-guide-header-btn"
                  onClick={() => setShowDepGuide(true)}
                  className="p-2 text-gray-400 hover:text-cyan-300 hover:bg-cyan-950/40 rounded-lg border border-transparent hover:border-cyan-800/40 transition"
                  title="系統依賴科普與安裝 (tsx / express)"
                >
                  <HelpCircle className="w-4 h-4" />
                </button>

                <button
                  id="open-wizard-btn"
                  onClick={() => setShowWizard(true)}
                  className="p-2 text-gray-400 hover:text-cyan-300 hover:bg-cyan-950/40 rounded-lg border border-transparent hover:border-cyan-800/40 transition"
                  title="開啟系統配置 (a --init)"
                >
                  <Settings className="w-4 h-4" />
                </button>
              </nav>
            )}
          </div>
        </div>
      </header>

      {/* Main Content Viewport */}
      <main className="flex-1 max-w-7xl w-full mx-auto px-4 sm:px-6 lg:px-8 py-6">
        {webEngineMode === 'standby' ? (
          <StandbyConsole
            rustStatus={rustStatus}
            onActivate={() => handleToggleWebEngine('active')}
          />
        ) : (
          <>
            {activeTab === 'dashboard' && (
              <Dashboard
                config={config}
                notes={notes}
                rustStatus={rustStatus}
                onNavigate={setActiveTab}
                onOpenWizard={() => setShowWizard(true)}
                onToggleWebEngine={handleToggleWebEngine}
                onOpenDependencyGuide={() => setShowDepGuide(true)}
              />
            )}

            {activeTab === 'ledger' && (
              <KeyLedgerView
                rustStatus={rustStatus}
                onRunCommand={(cmd) => {
                  setActiveTab('terminal');
                }}
              />
            )}

            {activeTab === 'notes' && (
              <NoteViewer
                config={config}
                notes={notes}
                onNotesChange={handleNotesUpdate}
              />
            )}

            {activeTab === 'radar' && (
              <GistRadar
                config={config}
                onNotesChange={handleNotesUpdate}
                onOpenWizard={() => setShowWizard(true)}
              />
            )}

            {activeTab === 'terminal' && (
              <Terminal
                config={config}
                onConfigChange={handleConfigUpdate}
                onNotesChange={handleNotesUpdate}
                onOpenWizard={() => setShowWizard(true)}
              />
            )}
          </>
        )}
      </main>

      {/* Security Status Footer */}
      <footer className="border-t border-gray-900 bg-[#090d14] text-xs py-4 text-gray-500">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 flex flex-col sm:flex-row items-center justify-between gap-3">
          <div className="flex items-center gap-3">
            <span className="flex items-center gap-1 text-emerald-400">
              <Lock className="w-3.5 h-3.5" />
              <span>Zero-Knowledge at Rest</span>
            </span>
            <span className="text-gray-700">|</span>
            <span>目錄: {config.noteDir}</span>
            <span className="text-gray-700 hidden md:inline">|</span>
            <span className="hidden md:inline">二進制: /usr/local/bin/a</span>
          </div>

          <div className="text-gray-600 flex items-center gap-2">
            <span>Rust 原生守護 (PID Managed)</span>
            <span>•</span>
            <span>JS Web 引擎 (可開可關，a -w 調度)</span>
          </div>
        </div>
      </footer>

      {/* Interactive Config Wizard Modal (a --init) */}
      {showWizard && (
        <ConfigWizard
          config={config}
          onSave={handleConfigUpdate}
          onClose={() => setShowWizard(false)}
        />
      )}

      {/* Dependency Guide & Diagnostic Modal (tsx / express) */}
      <DependencyGuideModal
        isOpen={showDepGuide}
        onClose={() => setShowDepGuide(false)}
      />
    </div>
  );
}
export default App;
