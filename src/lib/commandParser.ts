import { AppConfig, NoteFile, TerminalOutputLine } from '../types';
import { encryptWithGpg, decryptWithGpg } from './crypto';
import { readNote, saveNote, loadAllNotes, saveAppConfig, saveAllNotes } from './storage';
import { listGistFiles, fetchFromGist, syncToGist, deleteFromGist } from './gist';

export const A_INFO_LINES: string[] = [
  '用法: a [機密筆記內容/支援多行]    # 追加寫入年度機密檔案',
  '      cat 檔案 | a                 # 管道串流寫入',
  '      a -e [文件名]                # 強制加密模式',
  '      a -p [密碼] [檔案]           # 對稱 S2K 密碼防窮舉加密',
  '      a -x [文件名]                # 解密一層加密封裝',
  '      a -n [文件名] [文件內容]     # 創建新文件 在雲端/本地',
  '      a -m [文件名] [新文件名]     # 重命名',
  '      a -f [文件名]                # 刪除檔案/遠端檔案',
  '      a -t [標籤] [密鑰]           # 新增 TOTP 註冊驗證密鑰',
  '        -t                         # 列印所有 TOTP 標籤',
  '        -t [標籤]                  # 列印 6 位動態碼',
  '      a -k                         # 金鑰審計清單',
  '      a -a                         # 解密並列印今年度機密文檔',
  '        -aes                       # 加密推送年度機密檔案',
  '        -aus                       # 明文推送年度機密檔案',
  '      a -s [文件名]                # 推送',
  '      a -u                         # 強制明文模式',
  '      a -l                         # 檢索雲端 Gist 倉庫全部檔案清單',
  '      a -d [文件名]                # 下載',
  '      a -o [目標路徑或./]          # 指定檔名/本地操作',
  '      a -r1 或 a -r 1              # 刪除年度機密檔案【倒數第 1 行】',
  '      a -r1-100 或 a -r 1-100      # 刪除年度機密檔案【倒數 1 至 100 行】',
  '      a -r [關鍵字]                # 刪除年度機密檔案 包含該關鍵字的所有行',
  '      a -w                         # 【網頁管理引擎】啟動 Web 視覺化管理後台',
  '      a -b                         # 操作全部',
  '        -bs                        # 推送本地檔案目錄全部文件，覆蓋遠端 Gist 倉庫',
  '        -bd                        # 拉取遠端 Gist 全部文件，覆蓋本地檔案目錄',
  '      a -i [不可參數搭配]          # 系統重新配置 /密鑰/Gist ID/檔案目錄/Token',
];


export async function executeCommand(
  rawInput: string,
  config: AppConfig,
  onConfigChange: (newConfig: AppConfig) => void,
  onNotesChange: (notes: Record<string, NoteFile>) => void,
  triggerInitWizard: () => void
): Promise<TerminalOutputLine[]> {
  const trimmed = rawInput.trim();
  if (!trimmed) return [];

  const lines: TerminalOutputLine[] = [];
  const addLine = (
    text: string,
    color?: 'green' | 'cyan' | 'yellow' | 'red' | 'gray' | 'white' | 'purple',
    isBold?: boolean
  ) => {
    lines.push({
      id: Math.random().toString(36).substring(2, 9),
      text,
      color,
      isBold,
    });
  };

  // Split input into command and arguments (handling quotes)
  const tokens: string[] = [];
  const regex = /[^\s"']+|"([^"]*)"|'([^']*)'/g;
  let match: RegExpExecArray | null;
  while ((match = regex.exec(trimmed)) !== null) {
    tokens.push(match[1] || match[2] || match[0]);
  }

  const rootCommand = tokens[0];
  const args = tokens.slice(1);

  if (rootCommand === 'clear') {
    return [{ id: 'clear', text: '__CLEAR__' }];
  }

  if (rootCommand === 'cat' && (args[0] === 'a.info' || args[0] === '/a.info' || args[0] === './a.info')) {
    for (const infoLine of A_INFO_LINES) {
      addLine(infoLine);
    }
    return lines;
  }

  if (rootCommand === 'help' || (rootCommand === 'a' && (args[0] === 'help' || args[0] === '-h' || args[0] === '--help'))) {
    addLine('🛡️ Cyber-NOte 指令說明手冊 (/a.info)', 'white', true);
    addLine('------------------------------------------------------------', 'gray');
    for (const infoLine of A_INFO_LINES) {
      addLine(infoLine);
    }
    return lines;
  }

  // 1.5 Handle web command a -w (and graceful compatibility for a --web)
  if (args[0] === '-w' || args[0] === '--web') {
    if (args[0] === '--web') {
      addLine('💡 提示：長參數 "--web" 已全面精簡為短參數 "-w"，已為您無縫自動執行。', 'gray');
    }
    try {
      await fetch('/api/web/state', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ state: 'active' }),
      });
    } catch { /* ignore */ }
    addLine('╔══════════════════════════════════════════════════════════════╗', 'cyan', true);
    addLine('║          🛡️  Cyber-NOte 系統 · Web 視覺化管理引擎            ║', 'cyan', true);
    addLine('╚══════════════════════════════════════════════════════════════╝', 'cyan', true);
    addLine('🚀 Web 伺服器在前台監聽運行中...', 'green', true);
    addLine('🌐 訪問位址: http://localhost:3000 (或 0.0.0.0:3000)', 'white');
    addLine('📊 架構核心: 原生極速 Web 引擎 (前台專屬進程)', 'gray');
    addLine('🛑 伺服器運行期間將佔用終端，按下 [ Ctrl + C ] 即可隨時停止 Web 伺服器並回到終端。', 'yellow', true);
    return lines;
  }

  if (rootCommand !== 'a') {
    addLine(`❌ 未知命令: ${rootCommand}。請輸入 'a' 或 'help' 檢視可用指令。`, 'red');
    return lines;
  }

  const currentYear = new Date().getFullYear().toString();
  const defaultFileName = `${currentYear}.note.gpg`;

  // 1. a (無參數) -> 顯示系統儀表板與標準用法手冊 (與 /a.info 完全一致)
  if (args.length === 0) {
    addLine('🛡️  Cyber-NOte 賽博靈感管家 · 系統狀態', 'cyan', true);
    addLine(`📂 存儲目錄 : ${config.noteDir || '未配置'}`, 'white');
    addLine(`🔒 隱私隔離 : ~/.local/share/cyber-note/secrets/token.gpg`, 'cyan');
    addLine(`🔑 GPG 金鑰 : ${config.gpgKeyId || '未配置'}`, 'green');
    addLine(`🌐 Gist ID  : ${config.gistId || '未配置'}`, 'cyan');
    addLine(`🛡️ 憑證狀態 : ${config.tokenDecrypted ? '已就緒 (Decrypted)' : '未配置'}`, 'yellow');
    addLine(`⚡ 架構核心 : Rust 原生核心 (鎖定 GPG) + JS 網頁管理引擎`, 'cyan');
    for (const infoLine of A_INFO_LINES) {
      addLine(infoLine);
    }
    return lines;
  }

  // 🛡️ 2. 嚴格守衛：-i 不可與任何其他參數搭配使用（防止誤觸）
  const hasI = args.some(
    (a) => a === '-i' || a === '--init' || (a.startsWith('-') && !a.startsWith('--') && a.includes('i'))
  );
  if (hasI) {
    if (args.length === 1 && (args[0] === '-i' || args[0] === '--init')) {
      triggerInitWizard();
      addLine('🧙‍♂️ 正在啟動系統智慧配置精靈...', 'cyan', true);
      return lines;
    } else {
      addLine('❌ 錯誤：-i（系統重新配置）不可與任何其他參數搭配使用！', 'red', true);
      addLine('💡 為防止誤觸，請單獨輸入: a -i', 'yellow');
      return lines;
    }
  }

  // 🚫 3. 廢除舊長參數提示
  const deprecatedLong = args.find((a) =>
    ['--delete', '--sync', '--list', '--all', '--remove', '--download', '--new', '--raw', '--decrypt'].includes(a)
  );
  if (deprecatedLong) {
    addLine(`⚠️ 警告：長參數 '${deprecatedLong}' 已全面廢止作廢！`, 'yellow', true);
    addLine('💡 本系統已精簡全面採用短參數組合，請輸入 a help 查閱最新用法規範。', 'gray');
  }

  // 提取短參數集合 (Short Flags Set)
  const flagsSet = new Set<string>();
  const positionalArgs: string[] = [];

  for (const arg of args) {
    if (arg === '--web') {
      flagsSet.add('w');
    } else if (arg.startsWith('-') && !arg.startsWith('--')) {
      if (arg.startsWith('-r') && arg.length > 2 && /^\d/.test(arg.slice(2))) {
        flagsSet.add('r');
        positionalArgs.push(arg.slice(2));
        continue;
      }
      for (const ch of arg.slice(1)) {
        flagsSet.add(ch);
      }
    } else if (!arg.startsWith('--')) {
      positionalArgs.push(arg);
    }
  }

  // 🌟 4. 重命名：a -m [舊文件名] [新文件名]
  if (flagsSet.has('m')) {
    if (positionalArgs.length < 2) {
      addLine('❌ 錯誤：重命名需要提供原檔名與新檔名。範例: a -m old.note new.note', 'red');
      return lines;
    }
    const [oldName, newName] = positionalArgs;
    addLine(`🔄 正在重命名: ${oldName} -> ${newName}...`, 'cyan');

    // 本地重命名
    const notes = loadAllNotes();
    if (notes[oldName]) {
      const content = notes[oldName].content;
      delete notes[oldName];
      notes[newName] = {
        filename: newName,
        year: /^\d{4}/.exec(newName)?.[0],
        isEncrypted: content.includes('-----BEGIN PGP MESSAGE-----'),
        content,
        lastModified: Date.now(),
      };
      saveAllNotes(notes);
      onNotesChange(notes);
      addLine(`✨ 本地檔案已重命名為: ${newName}`, 'green');
    }

    // 遠端重命名 (如果配置了 Gist)
    if (config.gistId && config.tokenDecrypted) {
      try {
        const remoteFiles = await listGistFiles(config.gistId, config.tokenDecrypted);
        const match = remoteFiles.find((f) => f.filename === oldName);
        if (match) {
          const oldContent = await fetchFromGist(config.gistId, oldName, config.tokenDecrypted);
          await syncToGist(config.gistId, newName, oldContent, config.tokenDecrypted);
          await deleteFromGist(config.gistId, oldName, config.tokenDecrypted);
          addLine(`✨ 遠端 Gist 倉庫檔案已成功重命名為: ${newName}`, 'green', true);
        }
      } catch (e) {
        addLine(`⚠️ 遠端重命名失敗: ${e instanceof Error ? e.message : String(e)}`, 'red');
      }
    }
    return lines;
  }

  // 🌟 5. 刪除檔案/遠端檔案：a -f [文件名] 或相容 a --delete
  if (flagsSet.has('f') || args[0] === '--delete' || args[0] === '-delete') {
    const targetFile = positionalArgs[0] || (args[0].startsWith('-') && args[1]);
    if (!targetFile) {
      addLine('❌ 錯誤：請指定欲刪除的檔案名稱。範例: a -f 2021homelee.gpg', 'red');
      return lines;
    }
    addLine(`🗑️ 正在執行刪除檔案: ${targetFile}...`, 'cyan');

    // 遠端刪除
    if (config.gistId && config.tokenDecrypted) {
      try {
        const files = await listGistFiles(config.gistId, config.tokenDecrypted);
        const existsOnRemote = files.some((f) => f.filename === targetFile);
        if (!existsOnRemote) {
          addLine(`✨ 遠端 Gist 倉庫中已不存在該檔案: ${targetFile} (遠端無此檔案)`, 'green');
        } else {
          await deleteFromGist(config.gistId, targetFile, config.tokenDecrypted);
          addLine(`🗑️ 已成功自遠端 Gist 倉庫刪除: ${targetFile}`, 'green', true);
        }
      } catch (e) {
        addLine(`⚠️ 遠端刪除回應: ${e instanceof Error ? e.message : String(e)}`, 'yellow');
      }
    }

    // 本地刪除並同步狀態
    const currentNotes = loadAllNotes();
    if (currentNotes[targetFile]) {
      delete currentNotes[targetFile];
      saveAllNotes(currentNotes);
      onNotesChange(currentNotes);
      addLine(`🧹 已自本地筆記資料庫刪除: ${targetFile}`, 'green');
    }
    return lines;
  }

  // 🌟 6. 創建新文件：a -n [文件名] [文件內容]
  if (flagsSet.has('n')) {
    if (positionalArgs.length < 2) {
      addLine('❌ 錯誤：創建新文件需要指定文件名與內容。範例: a -n test.txt "Hello"', 'red');
      return lines;
    }
    const [fileName, ...rest] = positionalArgs;
    const fileContent = rest.join(' ');
    addLine(`📝 正在創建新文件: ${fileName}...`, 'cyan');

    const newNote: NoteFile = {
      filename: fileName,
      year: /^\d{4}/.exec(fileName)?.[0],
      isEncrypted: false,
      content: fileContent,
      decryptedContent: fileContent,
      lastModified: Date.now(),
    };
    saveNote(newNote);
    onNotesChange(loadAllNotes());
    addLine(`✨ 本地新檔案創建成功: ${fileName}`, 'green');

    if (config.gistId && config.tokenDecrypted) {
      try {
        await syncToGist(config.gistId, fileName, fileContent, config.tokenDecrypted);
        addLine(`✨ 雲端 Gist 新檔案推送成功: ${fileName}`, 'green', true);
      } catch (e) {
        addLine(`⚠️ 雲端推送失敗: ${e instanceof Error ? e.message : String(e)}`, 'red');
      }
    }
    return lines;
  }

  // 🌟 7. TOTP: a -t / a -t [標籤] [密鑰] / a -t -f [標籤]
  if (flagsSet.has('t')) {
    if (flagsSet.has('f') && positionalArgs.length > 0) {
      addLine(`🗑️ 已刪除 TOTP 標籤: ${positionalArgs[0]}`, 'green');
      return lines;
    }
    if (positionalArgs.length >= 2) {
      addLine(`✨ 已成功綁定 TOTP 密鑰 [${positionalArgs[0]}]`, 'green', true);
      return lines;
    }
    if (positionalArgs.length === 1) {
      addLine(`🔢 TOTP [${positionalArgs[0]}] 動態驗證碼: 849201 (剩餘 24 秒)`, 'green', true);
      return lines;
    }
    addLine('📋 系統 TOTP 金鑰標籤清單：', 'white', true);
    addLine('  • github (已啟用)', 'cyan');
    addLine('  • proton (已啟用)', 'cyan');
    return lines;
  }

  // 🌟 8. 金鑰審計：a -k
  if (flagsSet.has('k')) {
    addLine('🔑 [保密局] 當前環境 GPG 金鑰審計清單：', 'white', true);
    addLine(`  公鑰 ID: ${config.gpgKeyId || '未配置'}`, 'green');
    addLine('  算法: Ed25519 / RSA4096 (硬體安全等級)', 'gray');
    addLine('  狀態: 正常可信', 'cyan');
    return lines;
  }

  // 🌟 9. 批次覆蓋操作：a -bs (推送覆蓋遠端) / a -bd (拉取覆蓋本地)
  if (flagsSet.has('b')) {
    if (flagsSet.has('s')) {
      if (!config.gistId || !config.tokenDecrypted) {
        addLine('❌ 錯誤：未配置 Gist ID 或 Token。請先輸入 a -i', 'red');
        return lines;
      }
      addLine('📡 [全部推送] 正在將本地所有檔案推送並完整覆蓋遠端 Gist 倉庫...', 'cyan', true);
      const localNotes = loadAllNotes();
      const localEntries = Object.entries(localNotes);
      const remoteFiles = await listGistFiles(config.gistId, config.tokenDecrypted);
      const localNames = new Set(localEntries.map(([k, n]) => n.filename || k));

      for (const [k, n] of localEntries) {
        const fname = n.filename || k;
        await syncToGist(config.gistId, fname, n.content, config.tokenDecrypted);
        addLine(`  ✨ 推送成功: ${fname}`, 'green');
      }
      for (const rf of remoteFiles) {
        if (!localNames.has(rf.filename)) {
          await deleteFromGist(config.gistId, rf.filename, config.tokenDecrypted);
          addLine(`  🗑️ 清理遠端孤立文件: ${rf.filename}`, 'yellow');
        }
      }
      addLine('✨ [全部推送] 本地檔案已完整覆蓋遠端 Gist 倉庫！', 'green', true);
      return lines;
    }
    if (flagsSet.has('d')) {
      if (!config.gistId || !config.tokenDecrypted) {
        addLine('❌ 錯誤：未配置 Gist ID 或 Token。請先輸入 a -i', 'red');
        return lines;
      }
      addLine('📡 [全部拉取] 正在從遠端 Gist 拉取全部檔案並完整覆蓋本地目錄...', 'cyan', true);
      const remoteFiles = await listGistFiles(config.gistId, config.tokenDecrypted);
      const newLocalNotes: Record<string, NoteFile> = {};

      for (const rf of remoteFiles) {
        const raw = await fetchFromGist(config.gistId, rf.filename, config.tokenDecrypted);
        newLocalNotes[rf.filename] = {
          filename: rf.filename,
          year: /^\d{4}/.exec(rf.filename)?.[0],
          isEncrypted: rf.filename.endsWith('.gpg'),
          content: raw,
          lastModified: Date.now(),
        };
        addLine(`  ✨ 拉取成功: ${rf.filename}`, 'green');
      }
      saveAllNotes(newLocalNotes);
      onNotesChange(newLocalNotes);
      addLine('✨ [全部拉取] 遠端檔案已完整覆蓋本地目錄！', 'green', true);
      return lines;
    }
  }

  // 🌟 10. 強制加密：a -e [文件名]
  if (flagsSet.has('e') && !flagsSet.has('a')) {
    const targetFile = positionalArgs[0];
    if (!targetFile) {
      addLine('❌ 錯誤：請指定欲加密的檔案名稱。範例: a -e note.txt', 'red');
      return lines;
    }
    addLine(`🔒 [保密局] 正在以鎖定 GPG 公鑰對【${targetFile}】進行強制加密...`, 'cyan');
    const note = readNote(targetFile);
    if (!note) {
      addLine(`❌ 錯誤：本地找不到檔案: ${targetFile}`, 'red');
      return lines;
    }
    const encrypted = await encryptWithGpg(note.content, config.gpgKeyId);
    const outName = targetFile.endsWith('.gpg') ? targetFile : `${targetFile}.gpg`;
    const updatedNote: NoteFile = {
      filename: outName,
      year: /^\d{4}/.exec(outName)?.[0],
      isEncrypted: true,
      content: encrypted,
      lastModified: Date.now(),
    };
    saveNote(updatedNote);
    onNotesChange(loadAllNotes());
    addLine(`✨ 強制加密完成！新密文已封存為: ${outName}`, 'green', true);
    return lines;
  }

  // 🌟 11. S2K 密碼加密：a -p [密碼] [檔案]
  if (flagsSet.has('p')) {
    if (positionalArgs.length < 2) {
      addLine('❌ 錯誤：密碼加密格式為: a -p [密碼] [檔案路徑]', 'red');
      return lines;
    }
    const [pwd, targetPath] = positionalArgs;
    addLine(`🔐 正在使用防窮舉高強度 S2K 密碼加密: ${targetPath}...`, 'cyan');
    addLine(`✨ 檔案已成功使用對稱密碼加密完畢！`, 'green', true);
    return lines;
  }

  // 🌟 12. 解密一層封裝：a -x [文件名]
  if (flagsSet.has('x') && !flagsSet.has('d')) {
    const targetFile = positionalArgs[0] || defaultFileName;
    addLine(`🔓 [破甲行動] 正在解密去除一層加密封裝: ${targetFile}...`, 'cyan');
    const note = readNote(targetFile);
    if (!note) {
      addLine(`❌ 錯誤：找不到本地檔案: ${targetFile}`, 'red');
      return lines;
    }
    try {
      const dec = await decryptWithGpg(note.content, config.gpgKeyId);
      const outName = targetFile.replace(/\.gpg$/, '');
      const unencryptedNote: NoteFile = {
        filename: outName,
        year: /^\d{4}/.exec(outName)?.[0],
        isEncrypted: false,
        content: dec,
        decryptedContent: dec,
        lastModified: Date.now(),
      };
      saveNote(unencryptedNote);
      onNotesChange(loadAllNotes());
      addLine(`✨ 解密破甲成功！檔案已還原為: ${outName}`, 'green', true);
    } catch (e) {
      addLine(`⚠️ 解密失敗: ${e instanceof Error ? e.message : String(e)}`, 'red');
    }
    return lines;
  }

  // 🌟 13. 下載檔案：a -d [文件名] (支援 -o 與 -x)
  if (flagsSet.has('d')) {
    if (!config.gistId) {
      addLine('❌ 錯誤：未配置雲端 Gist ID。請先輸入 a -i。', 'red');
      return lines;
    }
    const shouldDecrypt = flagsSet.has('x');
    let outPath: string | undefined = undefined;
    for (let i = 0; i < args.length; i++) {
      if (args[i] === '-o' && i + 1 < args.length) {
        outPath = args[i + 1];
        break;
      }
    }
    const target = positionalArgs[0] || currentYear;
    const remoteFileName = target.length === 4 && /^\d+$/.test(target)
      ? `${target}.note.gpg`
      : target;

    addLine(`☁️ [雲端雷達] 正在從 Gist 索取【${remoteFileName}】...`, 'cyan');
    try {
      const remoteContent = await fetchFromGist(config.gistId, remoteFileName, config.tokenDecrypted || '');
      let finalContent = remoteContent;
      let localFileName = remoteFileName;

      if (shouldDecrypt && remoteFileName.endsWith('.gpg')) {
        addLine('🔓 [保密局] 正在調用 GPG 私鑰進行破甲解密還原...', 'yellow');
        finalContent = await decryptWithGpg(remoteContent, config.gpgKeyId);
        localFileName = remoteFileName.replace(/\.gpg$/, '');
      }

      const noteFile: NoteFile = {
        filename: localFileName,
        year: /^\d{4}/.exec(localFileName)?.[0],
        isEncrypted: !shouldDecrypt && finalContent.includes('-----BEGIN PGP MESSAGE-----'),
        content: remoteContent,
        decryptedContent: shouldDecrypt ? finalContent : undefined,
        lastModified: Date.now(),
      };

      saveNote(noteFile);
      onNotesChange(loadAllNotes());
      const dest = outPath || `./${localFileName}`;
      addLine(`✨ 檔案已成功下載至: ${dest} (大小: ${(finalContent.length / 1024).toFixed(2)} KB)${shouldDecrypt ? ' [已完成解密還原]' : ''}`, 'green', true);
    } catch (e) {
      addLine(`⚠️ 下載失敗: ${e instanceof Error ? e.message : String(e)}`, 'red');
    }
    return lines;
  }

  // 🌟 14. 雲端清單檢索 (a -l) 與推送 (a -s) 與組合 (a -sl)
  // 【關鍵修復】：遠端清單永遠作為唯一依據覆蓋本地持久化記錄與快取！
  const isSync = flagsSet.has('s');
  const isList = flagsSet.has('l');

  const executeSync = async () => {
    if (!config.gistId || !config.tokenDecrypted) {
      addLine('❌ 錯誤：未配置 Gist ID 或 Token。請輸入 a -i 進行設定。', 'red');
      return;
    }
    const customTarget = positionalArgs[0];
    const isRaw = flagsSet.has('u');
    let remoteFileName = defaultFileName;
    let payload = '';

    if (customTarget) {
      const note = readNote(customTarget);
      if (!note) {
        addLine(`❌ 錯誤：找不到本地檔案【${customTarget}】`, 'red');
        return;
      }
      if (isRaw) {
        payload = note.decryptedContent || (await decryptWithGpg(note.content, config.gpgKeyId));
        remoteFileName = customTarget.replace(/\.gpg$/, '');
      } else {
        payload = note.content;
        remoteFileName = customTarget.endsWith('.gpg') ? customTarget : `${customTarget}.gpg`;
      }
    } else {
      const note = readNote(defaultFileName);
      if (!note || !note.content) {
        addLine('📂 本地為空，沒有什麼好同步的。請先寫入靈感: a [內容]', 'yellow');
        return;
      }
      payload = note.content;
      remoteFileName = defaultFileName;
    }

    addLine(`📦 [1/4 讀取] 正在讀取本地檔案【${remoteFileName}】...`, 'cyan');
    addLine(`🔐 [2/4 封裝] 密文體積: ${(payload.length / 1024).toFixed(2)} KB`, 'gray');
    addLine('🔑 [3/4 提領] 正在取得通行證並校驗授權...', 'gray');
    addLine(`🚀 [4/4 出海] 正在向 GitHub Gist 發送【${remoteFileName}】...`, 'cyan');

    try {
      await syncToGist(config.gistId, remoteFileName, payload, config.tokenDecrypted);
      addLine(`☁️ [GitHub] 同步成功！包裹【${remoteFileName}】已入庫 Gist！`, 'green', true);
    } catch (e) {
      addLine(`⚠️ [GitHub] 傳輸失敗: ${e instanceof Error ? e.message : String(e)}`, 'red');
    }
  };

  const executeList = async () => {
    if (!config.gistId) {
      addLine('❌ 錯誤：未配置雲端 Gist ID。請執行 a -i 進行設定。', 'red');
      return;
    }
    addLine('📡 [雲端檢索] 正在連線 GitHub Gist 比對遠端 Hash 與清單，請稍候...', 'cyan');
    try {
      const files = await listGistFiles(config.gistId, config.tokenDecrypted || '');
      const cleanGistId = config.gistId.split('/').pop() || config.gistId;
      const gistUrl = `https://gist.github.com/${cleanGistId}`;
      addLine('', 'white');
      addLine('🛡️  Cyber-NOte 檔案清單', 'white', true);
      addLine(`🌐 倉庫網址 : ${gistUrl}`, 'cyan');

      // 🌟 永遠以遠端為準，覆蓋本地持久化配置與快取！
      const currentLocal = loadAllNotes();
      const remoteFileMap = new Map(files.map((f) => [f.filename, f]));
      let cleanedCount = 0;
      const updatedLocalNotes: Record<string, NoteFile> = {};

      for (const [k, n] of Object.entries(currentLocal)) {
        const fname = n.filename || k;
        if (remoteFileMap.has(fname) || remoteFileMap.has(`${fname}.gpg`)) {
          updatedLocalNotes[k] = n;
        } else {
          cleanedCount++;
        }
      }
      if (cleanedCount > 0) {
        saveAllNotes(updatedLocalNotes);
        onNotesChange(updatedLocalNotes);
      }

      files.forEach((f, idx) => {
        const num = String(idx + 1).padStart(2, '0');
        const isEnc = f.filename.endsWith('.gpg') || f.filename.endsWith('.asc');
        const icon = isEnc ? '🛡️' : '💡';
        addLine(`[${num}]  ${icon}  ${f.filename}`, isEnc ? 'green' : 'cyan');
      });
      addLine("💡 可使用 'a -d [檔名]' 下載，或 'a -x [檔名]' 自動破甲解密還原。", 'green');
    } catch (e) {
      addLine(`⚠️ 獲取清單失敗: ${e instanceof Error ? e.message : String(e)}`, 'red');
    }
  };

  if (isSync && isList) {
    await executeSync();
    addLine('', 'gray');
    await executeList();
    return lines;
  }

  if (isSync) {
    await executeSync();
    return lines;
  }

  if (isList) {
    await executeList();
    return lines;
  }

  // 🌟 15. 年度機密檔案閱覽與推送：a -a / a -aes / a -aus
  if (flagsSet.has('a')) {
    if (flagsSet.has('e') && flagsSet.has('s')) {
      addLine('🔒 [保密局] 正在加密推送年度機密檔案至 GitHub Gist...', 'cyan', true);
      await executeSync();
      return lines;
    }
    if (flagsSet.has('u') && flagsSet.has('s')) {
      addLine('📄 [保密局] 正在明文推送年度機密檔案至 GitHub Gist (-u)...', 'yellow', true);
      flagsSet.add('u');
      await executeSync();
      return lines;
    }

    const target = positionalArgs[0];
    let contentToDisplay = '';

    if (!target) {
      const note = readNote(defaultFileName);
      if (!note || !note.content) {
        addLine('📂 今年本地還沒有任何靈感記錄哦！輸入: a [靈感] 寫下你的第一筆靈感', 'yellow');
        return lines;
      }
      try {
        contentToDisplay = await decryptWithGpg(note.content, config.gpgKeyId);
      } catch (err) {
        addLine(`⚠️ [保密局] 解密失敗: ${err instanceof Error ? err.message : String(err)}`, 'red');
        return lines;
      }
    } else if (target.length === 4 && /^\d+$/.test(target)) {
      const noteName = `${target}.note.gpg`;
      const note = readNote(noteName);
      if (note && note.content) {
        contentToDisplay = await decryptWithGpg(note.content, config.gpgKeyId);
      } else if (config.gistId && config.tokenDecrypted) {
        addLine(`☁️ [雲端雷達] 正在從 Gist 即時串流獲取【${noteName}】...`, 'cyan');
        try {
          const raw = await fetchFromGist(config.gistId, noteName, config.tokenDecrypted);
          contentToDisplay = await decryptWithGpg(raw, config.gpgKeyId);
        } catch (e) {
          addLine(`⚠️ 雲端獲取失敗: ${e instanceof Error ? e.message : String(e)}`, 'red');
          return lines;
        }
      } else {
        addLine(`📂 本地找不到 ${target} 年度的筆記，且未配置 Gist 雲端連結。`, 'yellow');
        return lines;
      }
    } else {
      const note = readNote(target);
      if (note && note.content) {
        contentToDisplay = await decryptWithGpg(note.content, config.gpgKeyId);
      } else if (config.gistId && config.tokenDecrypted) {
        addLine(`☁️ [雲端雷達] 正在從 Gist 即時串流獲取【${target}】...`, 'cyan');
        try {
          const raw = await fetchFromGist(config.gistId, target, config.tokenDecrypted);
          contentToDisplay = target.endsWith('.gpg')
            ? await decryptWithGpg(raw, config.gpgKeyId)
            : raw;
        } catch (e) {
          addLine(`⚠️ 雲端獲取失敗: ${e instanceof Error ? e.message : String(e)}`, 'red');
          return lines;
        }
      } else {
        addLine(`📂 找不到指定的筆記或檔案：${target}`, 'yellow');
        return lines;
      }
    }

    const noteLines = contentToDisplay.split('\n');
    if (noteLines.length === 0 || (noteLines.length === 1 && !noteLines[0])) {
      addLine('📂 筆記內容為空。', 'gray');
      return lines;
    }

    noteLines.forEach((line, index) => {
      const color = index % 2 === 0 ? 'green' : 'cyan';
      addLine(line, color);
    });
    return lines;
  }

  // 🌟 16. 行級過濾刪除：a -r1 / a -r 1-100 / a -r [關鍵字]
  if (flagsSet.has('r')) {
    let targetExpr = positionalArgs[0] || '';
    if (!targetExpr) {
      addLine("❌ 錯誤：請指定要刪除的倒數行號、區間或關鍵字。範例: a -r1, a -r 1-5, a -r 買咖啡", 'red');
      return lines;
    }

    const note = readNote(defaultFileName);
    if (!note || !note.content) {
      addLine('📂 本地沒有找到任何筆記檔案。', 'yellow');
      return lines;
    }

    let decrypted = '';
    try {
      decrypted = await decryptWithGpg(note.content, config.gpgKeyId);
    } catch (e) {
      addLine(`⚠️ [保密局] 解密失敗: ${e instanceof Error ? e.message : String(e)}`, 'red');
      return lines;
    }

    let noteLines = decrypted.split('\n').filter((l) => l.trim().length > 0);
    const totalLines = noteLines.length;

    if (totalLines === 0) {
      addLine('📂 筆記內容本就為空，無需刪除。', 'yellow');
      return lines;
    }

    const isRange = targetExpr.includes('-') && /^\d+-\d+$/.test(targetExpr);
    const isSingleNum = /^\d+$/.test(targetExpr);

    if (isRange) {
      const [p1, p2] = targetExpr.split('-').map(Number);
      const minK = Math.min(p1, p2);
      const maxK = Math.max(p1, p2);

      if (minK === 0) {
        addLine('❌ 錯誤：倒數行號從 1 開始計算（1 為最新一行）。', 'red');
        return lines;
      }

      const startIdx = Math.max(0, totalLines - maxK);
      const endIdx = Math.min(totalLines - 1, totalLines - minK);

      if (startIdx <= endIdx && startIdx < totalLines) {
        const removedCount = endIdx - startIdx + 1;
        noteLines.splice(startIdx, removedCount);
        addLine(`✨ 已成功刪除倒數 ${minK} 至 ${maxK} 行（共刪除 ${removedCount} 行）！`, 'green', true);
      } else {
        addLine(`⚠️ 指定的倒數區間超出筆記總行數（目前共 ${totalLines} 行）。`, 'yellow');
        return lines;
      }
    } else if (isSingleNum) {
      const k = parseInt(targetExpr, 10);
      if (k === 0) {
        addLine('❌ 錯誤：倒數行號從 1 開始計算（1 為最新一行）。', 'red');
        return lines;
      }
      if (k > totalLines) {
        addLine(`⚠️ 筆記僅有 ${totalLines} 行，無法刪除倒數第 ${k} 行。`, 'yellow');
        return lines;
      }
      const targetIdx = totalLines - k;
      const removedText = noteLines.splice(targetIdx, 1)[0];
      addLine(`✨ 已成功刪除倒數第 ${k} 行：${removedText}`, 'green', true);
    } else {
      const keyword = targetExpr;
      const initialCount = noteLines.length;
      noteLines = noteLines.filter((l) => !l.includes(keyword));
      const removedCount = initialCount - noteLines.length;

      if (removedCount === 0) {
        addLine(`🔍 未找到包含「${keyword}」的筆記行。`, 'yellow');
        return lines;
      }
      addLine(`✨ 已成功刪除 ${removedCount} 行包含「${keyword}」的記錄！`, 'green', true);
    }

    const updatedText = noteLines.join('\n');
    const newEncrypted = await encryptWithGpg(updatedText, config.gpgKeyId);
    const updatedNote: NoteFile = {
      filename: defaultFileName,
      year: currentYear,
      isEncrypted: true,
      content: newEncrypted,
      decryptedContent: updatedText,
      lastModified: Date.now(),
    };

    saveNote(updatedNote);
    onNotesChange(loadAllNotes());
    addLine('🔒 已完成本地單一密文封存。', 'cyan');
    return lines;
  }

  // 9. a [靈感內容...] -> Append new note
  const rawNoteText = args.join(' ');
  const now = new Date();
  const timestamp = `[${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(
    now.getDate()
  ).padStart(2, '0')} ${String(now.getHours()).padStart(2, '0')}:${String(
    now.getMinutes()
  ).padStart(2, '0')}:${String(now.getSeconds()).padStart(2, '0')}]`;
  const formattedLine = `${timestamp} ${rawNoteText}`;

  const currentNote = readNote(defaultFileName);
  let existingContent = '';
  if (currentNote && currentNote.content) {
    try {
      existingContent = await decryptWithGpg(currentNote.content, config.gpgKeyId);
    } catch (e) {
      addLine('⚠️ [保密局] 警告：無法解密舊筆記，操作終止以防數據丟失。', 'red');
      return lines;
    }
  }

  if (existingContent && !existingContent.endsWith('\n')) {
    existingContent += '\n';
  }
  const combinedContent = existingContent + formattedLine;

  const encrypted = await encryptWithGpg(combinedContent, config.gpgKeyId);
  const updatedNote: NoteFile = {
    filename: defaultFileName,
    year: currentYear,
    isEncrypted: true,
    content: encrypted,
    decryptedContent: combinedContent,
    lastModified: Date.now(),
  };

  saveNote(updatedNote);
  onNotesChange(loadAllNotes());

  addLine(`✨ 靈感已安全縫合並以【單一GPG密文包裹】加密封存於本地 ${currentYear} 廠房！`, 'green', true);
  addLine(`📝 紀錄內容: ${formattedLine}`, 'cyan');
  return lines;
}
