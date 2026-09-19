import React, { useState, useRef, useEffect } from 'react';
import { Terminal as TerminalIcon, Play, Trash2, ArrowUpRight, HelpCircle } from 'lucide-react';
import { AppConfig, NoteFile, TerminalOutputLine } from '../types';
import { executeCommand } from '../lib/commandParser';

interface TerminalProps {
  config: AppConfig;
  onConfigChange: (newConfig: AppConfig) => void;
  onNotesChange: (notes: Record<string, NoteFile>) => void;
  onOpenWizard: () => void;
}

export const Terminal: React.FC<TerminalProps> = ({
  config,
  onConfigChange,
  onNotesChange,
  onOpenWizard,
}) => {
  const [history, setHistory] = useState<TerminalOutputLine[]>([
    {
      id: 'welcome-1',
      text: '🛡️  Cyber-Forge 賽博靈感管家 · 終端控制台 (Project a CLI)',
      color: 'cyan',
      isBold: true,
    },
    {
      id: 'welcome-2',
      text: '端到端 GPG 公鑰加密，落盤即密文。輸入 "help" 或 "a" 開始，或直接鍵入: a [靈感記錄]',
      color: 'gray',
    },
  ]);
  const [inputVal, setInputVal] = useState('a -a');
  const [commandHistory, setCommandHistory] = useState<string[]>(['a -a', 'a', 'help']);
  const [historyIdx, setHistoryIdx] = useState(-1);
  const [isExecuting, setIsExecuting] = useState(false);

  const bottomRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [history]);

  const handleRun = async (cmdToRun?: string) => {
    const cmd = (cmdToRun !== undefined ? cmdToRun : inputVal).trim();
    if (!cmd || isExecuting) return;

    // Add user command line
    setHistory((prev) => [
      ...prev,
      {
        id: Math.random().toString(36),
        text: `$ ${cmd}`,
        color: 'white',
        isBold: true,
      },
    ]);

    setCommandHistory((prev) => [...prev.filter((c) => c !== cmd), cmd]);
    setHistoryIdx(-1);
    setInputVal('');
    setIsExecuting(true);

    try {
      const output = await executeCommand(
        cmd,
        config,
        onConfigChange,
        onNotesChange,
        onOpenWizard
      );

      if (output.length === 1 && output[0].text === '__CLEAR__') {
        setHistory([]);
      } else {
        setHistory((prev) => [...prev, ...output]);
      }
    } catch (err) {
      setHistory((prev) => [
        ...prev,
        {
          id: Math.random().toString(36),
          text: `⚠️ 執行錯誤: ${err instanceof Error ? err.message : String(err)}`,
          color: 'red',
        },
      ]);
    } finally {
      setIsExecuting(false);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleRun();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (commandHistory.length === 0) return;
      const nextIdx = historyIdx < commandHistory.length - 1 ? historyIdx + 1 : historyIdx;
      setHistoryIdx(nextIdx);
      setInputVal(commandHistory[commandHistory.length - 1 - nextIdx]);
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (historyIdx > 0) {
        const nextIdx = historyIdx - 1;
        setHistoryIdx(nextIdx);
        setInputVal(commandHistory[commandHistory.length - 1 - nextIdx]);
      } else if (historyIdx === 0) {
        setHistoryIdx(-1);
        setInputVal('');
      }
    }
  };

  const quickCommands = [
    { label: 'a -a (今年筆記)', cmd: 'a -a' },
    { label: 'a (系統儀表)', cmd: 'a' },
    { label: 'a -w (啟動 Web)', cmd: 'a -w' },
    { label: 'a -l (雲端清單)', cmd: 'a -l' },
    { label: 'a -s (雲端同步)', cmd: 'a -s' },
    { label: 'a --init (精靈)', cmd: 'a --init' },
    { label: 'help (指令手冊)', cmd: 'help' },
  ];

  return (
    <div id="cyber-terminal-card" className="flex flex-col h-[calc(100vh-14rem)] min-h-[500px] bg-[#0c1017] border border-cyan-900/40 rounded-xl shadow-2xl overflow-hidden">
      {/* Terminal Title Bar */}
      <div className="flex items-center justify-between px-4 py-2.5 bg-[#161b22] border-b border-gray-800 text-xs">
        <div className="flex items-center gap-2">
          <div className="flex gap-1.5">
            <span className="w-3 h-3 rounded-full bg-red-500/80 inline-block"></span>
            <span className="w-3 h-3 rounded-full bg-yellow-500/80 inline-block"></span>
            <span className="w-3 h-3 rounded-full bg-emerald-500/80 inline-block"></span>
          </div>
          <span className="ml-2 text-gray-400 font-mono flex items-center gap-1.5">
            <TerminalIcon className="w-3.5 h-3.5 text-cyan-400" />
            cyber-forge@vault:~ {config.noteDir}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <button
            id="terminal-clear-btn"
            onClick={() => setHistory([])}
            className="p-1 text-gray-400 hover:text-gray-200 hover:bg-gray-800 rounded transition"
            title="清空螢幕 (clear)"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Quick command buttons */}
      <div className="px-4 py-2 bg-[#12161f] border-b border-gray-800/80 flex flex-wrap gap-2 items-center text-xs">
        <span className="text-gray-500 text-[11px] font-mono">快速調度:</span>
        {quickCommands.map((qc) => (
          <button
            key={qc.cmd}
            onClick={() => handleRun(qc.cmd)}
            className="px-2.5 py-1 rounded bg-[#1c212c] hover:bg-cyan-950/60 hover:text-cyan-300 text-gray-300 border border-gray-700/60 hover:border-cyan-500/40 transition font-mono flex items-center gap-1"
          >
            <span>{qc.label}</span>
            <ArrowUpRight className="w-3 h-3 opacity-60" />
          </button>
        ))}
      </div>

      {/* Output Console Screen */}
      <div className="flex-1 p-4 overflow-y-auto font-mono text-sm space-y-1 bg-[#090d14]">
        {history.map((line) => {
          let colorClass = 'text-gray-300';
          if (line.color === 'green') colorClass = 'text-emerald-400';
          if (line.color === 'cyan') colorClass = 'text-cyan-400';
          if (line.color === 'yellow') colorClass = 'text-amber-300';
          if (line.color === 'red') colorClass = 'text-rose-400';
          if (line.color === 'purple') colorClass = 'text-purple-300';
          if (line.color === 'gray') colorClass = 'text-gray-500';
          if (line.color === 'white') colorClass = 'text-white';

          const renderTextWithLinks = (text: string) => {
            const urlRegex = /(https?:\/\/[^\s]+)/g;
            const parts = text.split(urlRegex);
            return parts.map((part, i) => {
              if (part.match(urlRegex)) {
                return (
                  <a
                    key={i}
                    href={part}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-cyan-300 underline hover:text-cyan-200 transition break-all inline-flex items-center gap-1"
                    onClick={(e) => e.stopPropagation()}
                    title="點擊在預設瀏覽器中開啟"
                  >
                    {part}
                  </a>
                );
              }
              return part;
            });
          };

          return (
            <div
              key={line.id}
              className={`leading-relaxed break-words whitespace-pre-wrap ${colorClass} ${
                line.isBold ? 'font-semibold' : 'font-normal'
              }`}
            >
              {renderTextWithLinks(line.text)}
            </div>
          );
        })}
        {isExecuting && (
          <div className="flex items-center gap-2 text-cyan-400 text-xs py-1 animate-pulse">
            <span className="w-2 h-2 rounded-full bg-cyan-400 animate-ping"></span>
            <span>指令調度處理中...</span>
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      {/* Input prompt area */}
      <div className="p-3 bg-[#11151e] border-t border-cyan-950/80 flex items-center gap-2">
        <span className="text-emerald-400 font-mono font-bold select-none text-sm">$</span>
        <input
          id="terminal-cli-input"
          ref={inputRef}
          type="text"
          value={inputVal}
          onChange={(e) => setInputVal(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="例如: a 記錄靈感、a -a、a -s、a -r1 或 a --init..."
          disabled={isExecuting}
          className="flex-1 bg-transparent text-gray-100 placeholder-gray-600 font-mono text-sm focus:outline-none"
          autoFocus
        />
        <button
          id="terminal-submit-btn"
          onClick={() => handleRun()}
          disabled={isExecuting || !inputVal.trim()}
          className="px-3 py-1.5 bg-cyan-600 hover:bg-cyan-500 disabled:opacity-40 disabled:hover:bg-cyan-600 text-white rounded font-medium text-xs flex items-center gap-1.5 transition shadow-sm"
        >
          <Play className="w-3 h-3" />
          <span>執行</span>
        </button>
      </div>
    </div>
  );
};
