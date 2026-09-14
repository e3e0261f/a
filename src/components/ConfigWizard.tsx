import React, { useState } from 'react';
import {
  Shield,
  Key,
  Cloud,
  Folder,
  Lock,
  CheckCircle2,
  ArrowRight,
  ArrowLeft,
  X,
  Sparkles,
} from 'lucide-react';
import { AppConfig } from '../types';
import { saveAppConfig } from '../lib/storage';
import { extractCleanGistId } from '../lib/gist';
import { encryptWithGpg } from '../lib/crypto';

interface ConfigWizardProps {
  config: AppConfig;
  onSave: (newConfig: AppConfig) => void;
  onClose: () => void;
}

export const ConfigWizard: React.FC<ConfigWizardProps> = ({ config, onSave, onClose }) => {
  const defaultStandardDir = '~/.local/share/cyber-note/notes';
  const [step, setStep] = useState<number>(1);
  const [noteDir, setNoteDir] = useState<string>(
    config.noteDir && config.noteDir !== '~/BOok/NOte' ? config.noteDir : defaultStandardDir
  );
  const [gpgKeyId, setGpgKeyId] = useState<string>(config.gpgKeyId || '');
  const [rawGist, setRawGist] = useState<string>(config.gistId || '');
  const [tokenInput, setTokenInput] = useState<string>(config.tokenDecrypted || '');
  const [isEncryptingToken, setIsEncryptingToken] = useState<boolean>(false);
  const [isCompleted, setIsCompleted] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const isSshKey = (val: string) => {
    const lower = val.trim().toLowerCase();
    return (
      lower.startsWith('ssh-') ||
      lower.startsWith('ecdsa-') ||
      lower.includes('id_rsa') ||
      lower.includes('id_ed25519') ||
      lower.includes('id_ecdsa') ||
      lower.includes('.ssh/') ||
      lower.includes('begin openssh') ||
      lower.includes('begin rsa private key')
    );
  };

  const handleNext = () => {
    setErrorMessage(null);
    if (step === 2 && gpgKeyId.trim()) {
      if (isSshKey(gpgKeyId)) {
        setErrorMessage('安全策略合規失敗：系統嚴格鎖定 GPG 金鑰，禁止使用 SSH 金鑰！請輸入有效的 GPG Key ID 或公鑰指紋。');
        return;
      }
    }
    if (step < 4) {
      setStep(step + 1);
    } else {
      handleFinalSave();
    }
  };

  const handleBack = () => {
    if (step > 1) {
      setStep(step - 1);
    }
  };

  const handleFinalSave = async () => {
    setIsEncryptingToken(true);
    const cleanGistId = extractCleanGistId(rawGist);

    let encryptedTokenArmor = config.tokenGpg;
    if (tokenInput.trim()) {
      try {
        encryptedTokenArmor = await encryptWithGpg(
          tokenInput.trim(),
          gpgKeyId.trim() || 'CyberNOte-Key'
        );
      } catch (err) {
        console.warn('Failed to encrypt token with GPG:', err);
      }
    }

    const updatedConfig: AppConfig = {
      noteDir: noteDir.trim() || defaultStandardDir,
      gpgKeyId: gpgKeyId.trim() || 'CyberNOte-Key',
      gistId: cleanGistId,
      tokenGpg: encryptedTokenArmor,
      tokenDecrypted: tokenInput.trim(),
      configured: true,
    };

    saveAppConfig(updatedConfig);
    onSave(updatedConfig);
    setIsEncryptingToken(false);
    setIsCompleted(true);
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4 backdrop-blur-sm">
      <div className="bg-[#0e131d] border border-cyan-800/60 rounded-2xl max-w-xl w-full overflow-hidden shadow-2xl font-mono">
        {/* Wizard Header */}
        <div className="px-6 py-4 bg-[#141a26] border-b border-gray-800 flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="p-2 bg-cyan-950/80 border border-cyan-700/50 rounded-lg text-cyan-400">
              <Shield className="w-5 h-5" />
            </div>
            <div>
              <h2 className="text-sm font-bold text-gray-100">
                Cyber-NOte 核心配置導引 (a --init)
              </h2>
              <p className="text-[11px] text-gray-400">
                {isCompleted ? '配置已完成' : `步驟 ${step} / 4: 系統核心環境錨定`}
              </p>
            </div>
          </div>

          <button
            onClick={onClose}
            className="text-gray-400 hover:text-white p-1 rounded hover:bg-gray-800 transition"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Wizard Progress Bar */}
        {!isCompleted && (
          <div className="w-full bg-gray-900 h-1">
            <div
              className="bg-cyan-500 h-1 transition-all duration-300"
              style={{ width: `${(step / 4) * 100}%` }}
            />
          </div>
        )}

        {/* Wizard Body */}
        <div className="p-6 text-xs space-y-4">
          {isCompleted ? (
            <div className="py-6 text-center space-y-4">
              <div className="w-12 h-12 bg-emerald-950/60 border border-emerald-500/50 rounded-full flex items-center justify-center mx-auto text-emerald-400 shadow-lg">
                <CheckCircle2 className="w-6 h-6" />
              </div>
              <div className="space-y-1">
                <h3 className="text-base font-bold text-gray-100">配置更新完成</h3>
                <p className="text-gray-400">
                  本地存儲路徑、GPG 金鑰指紋與 GitHub Gist 雲端已配置完成。
                </p>
              </div>

              <div className="p-4 bg-[#090d14] border border-gray-800 rounded-xl text-left space-y-1.5 text-gray-300">
                <div>📂 存儲路徑: <span className="text-cyan-400">{noteDir}</span></div>
                <div>🔑 GPG 指紋: <span className="text-emerald-400">{gpgKeyId || '未配置'}</span></div>
                <div>🌐 Gist ID : <span className="text-cyan-400">{extractCleanGistId(rawGist) || '未配置'}</span></div>
                <div>🛡️ 隱私存放: <span className="text-amber-400">~/.config/cyber-note/secrets/token.gpg</span></div>
              </div>

              <button
                onClick={onClose}
                className="px-6 py-2 bg-cyan-600 hover:bg-cyan-500 text-white font-semibold rounded-lg transition"
              >
                返回主介面
              </button>
            </div>
          ) : (
            <>
              {/* Error Message */}
              {errorMessage && (
                <div className="p-3 bg-red-950/60 border border-red-800 text-red-300 rounded-lg text-xs flex items-start gap-2">
                  <span className="font-bold flex-shrink-0">⚠️</span>
                  <span>{errorMessage}</span>
                </div>
              )}

              {/* Step 1: Storage Directory */}
              {step === 1 && (
                <div className="space-y-3">
                  <div className="flex items-center gap-2 text-cyan-400 font-semibold text-sm">
                    <Folder className="w-4 h-4" />
                    <span>步驟 1/4: 本地存儲目錄 (XDG 標準目錄)</span>
                  </div>
                  <p className="text-gray-400">
                    預設採用所有 Linux 發行版原生支援之標準目錄：<code>~/.local/share/cyber-note/notes</code>，亦可自訂絕對路徑。
                  </p>
                  <div>
                    <label className="block text-gray-300 mb-1">存儲目錄路徑:</label>
                    <input
                      type="text"
                      value={noteDir}
                      onChange={(e) => setNoteDir(e.target.value)}
                      placeholder="~/.local/share/cyber-note/notes"
                      className="w-full px-3 py-2 bg-[#090d14] border border-gray-800 rounded-lg text-gray-100 focus:border-cyan-500 focus:outline-none"
                    />
                  </div>
                </div>
              )}

              {/* Step 2: GPG Key ID */}
              {step === 2 && (
                <div className="space-y-3">
                  <div className="flex items-center gap-2 text-emerald-400 font-semibold text-sm">
                    <Key className="w-4 h-4" />
                    <span>步驟 2/4: GPG 公鑰指紋配置 (Key Identifier)</span>
                  </div>
                  <p className="text-gray-400">
                    輸入你的 GPG 金鑰標識（指紋、子金鑰 ID、或公鑰指紋）。用於單向整檔 ASCII Armor 密文封裝。
                  </p>
                  <div>
                    <label className="block text-gray-300 mb-1">GPG Key ID / 指紋:</label>
                    <input
                      type="text"
                      value={gpgKeyId}
                      onChange={(e) => setGpgKeyId(e.target.value)}
                      placeholder="例如: 9A4C73E1 或 你的GPG金鑰指紋"
                      className="w-full px-3 py-2 bg-[#090d14] border border-gray-800 rounded-lg text-gray-100 focus:border-emerald-500 focus:outline-none"
                    />
                  </div>
                </div>
              )}

              {/* Step 3: Gist ID */}
              {step === 3 && (
                <div className="space-y-3">
                  <div className="flex items-center gap-2 text-cyan-400 font-semibold text-sm">
                    <Cloud className="w-4 h-4" />
                    <span>步驟 3/4: 雲端 Gist 倉庫配置 (Gist Repository)</span>
                  </div>
                  <p className="text-gray-400">
                    輸入 GitHub Gist 的 ID 或完整網址。系統將透過局部 PATCH 進行版本維護，絕不互相覆蓋。
                  </p>
                  <div>
                    <label className="block text-gray-300 mb-1">Gist ID 或 URL:</label>
                    <input
                      type="text"
                      value={rawGist}
                      onChange={(e) => setRawGist(e.target.value)}
                      placeholder="https://gist.github.com/username/gist_id 或直接填入 gist_id"
                      className="w-full px-3 py-2 bg-[#090d14] border border-gray-800 rounded-lg text-gray-100 focus:border-cyan-500 focus:outline-none"
                    />
                  </div>
                  {rawGist && (
                    <div className="text-[11px] text-gray-500">
                      萃取乾淨 ID: <span className="text-cyan-400">{extractCleanGistId(rawGist)}</span>
                    </div>
                  )}
                </div>
              )}

              {/* Step 4: GitHub Token */}
              {step === 4 && (
                <div className="space-y-3">
                  <div className="flex items-center gap-2 text-yellow-400 font-semibold text-sm">
                    <Lock className="w-4 h-4" />
                    <span>步驟 4/4: GitHub Token 憑證加密 (token.gpg)</span>
                  </div>
                  <p className="text-gray-400">
                    輸入 GitHub Personal Access Token (需勾選 <code>gist</code> 權限)。
                    Token 將直接以 GPG 公鑰加密為 <code>token.gpg</code> 儲存，杜絕明文洩漏。
                  </p>
                  <div>
                    <label className="block text-gray-300 mb-1">GitHub Personal Access Token:</label>
                    <input
                      type="password"
                      value={tokenInput}
                      onChange={(e) => setTokenInput(e.target.value)}
                      placeholder="ghp_xxxxxxxxxxxxxxxxxxxx"
                      className="w-full px-3 py-2 bg-[#090d14] border border-gray-800 rounded-lg text-gray-100 focus:border-yellow-500 focus:outline-none"
                    />
                  </div>
                </div>
              )}

              {/* Navigation Buttons */}
              <div className="flex items-center justify-between pt-4 border-t border-gray-800">
                <button
                  type="button"
                  onClick={handleBack}
                  disabled={step === 1}
                  className="px-4 py-2 bg-[#171c26] hover:bg-[#202735] disabled:opacity-30 text-gray-300 rounded-lg transition flex items-center gap-1.5"
                >
                  <ArrowLeft className="w-3.5 h-3.5" />
                  <span>上一步</span>
                </button>

                <button
                  type="button"
                  onClick={handleNext}
                  disabled={isEncryptingToken}
                  className="px-5 py-2 bg-cyan-600 hover:bg-cyan-500 text-white font-medium rounded-lg transition flex items-center gap-1.5 shadow-md"
                >
                  <span>{step === 4 ? (isEncryptingToken ? '封裝中...' : '封裝存盤') : '下一步'}</span>
                  <ArrowRight className="w-3.5 h-3.5" />
                </button>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
};
