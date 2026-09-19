import { AppConfig, NoteFile, TerminalOutputLine } from '../types';
import { encryptWithGpg, decryptWithGpg } from './crypto';
import { readNote, saveNote, loadAllNotes, saveAppConfig } from './storage';
import { listGistFiles, fetchFromGist, syncToGist } from './gist';

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

  if (rootCommand === 'help') {
    addLine('🛡️ Cyber-Forge (Project a) 指令說明手冊', 'white', true);
    addLine('------------------------------------------------------------', 'gray');
    addLine('  a [靈感內容...]            寫入筆記：自動拼接並以 GPG 公鑰加密封存', 'cyan');
    addLine('  a -a / a --all            解密並雙色交替檢視今年本地筆記', 'green');
    addLine('  a -a [年份/檔名]           檢視指定年份或雲端/本地檔案', 'cyan');
    addLine('  a -s / a --sync           推送當前年度加密筆記至 GitHub Gist', 'green');
    addLine('  a -s [檔名] --raw         明文模式外傳檔案至 Gist (-u)', 'yellow');
    addLine('  a -l / a --list           掃描並列出 GitHub Gist 雲端倉庫檔案清單', 'cyan');
    addLine('  a -d --all                批量下載 GitHub Gist 雲端倉庫全部檔案', 'green');
    addLine('  a -d [檔名] -o [目標路徑]   自訂檔名或加 ./ 下載文件到本地工作目錄', 'green');
    addLine('  a -d [檔名] [-x]          下載雲端檔案 (-x 為自動破甲解密還原)', 'green');
    addLine('  a --new [檔名] [內容]       在雲端 Gist 創建並寫入新檔案', 'purple');
    addLine('  a --delete [檔名]          指定刪除遠端 Gist 倉庫中的指定檔案', 'purple');
    addLine('  a -r [關鍵字/倒數行/區間]   行級刪除：過濾指定內容重新加密存盤', 'yellow');
    addLine('  a -w / a --web [status/stop] 調度 JS 網頁管理引擎 (可開可關，預設關閉)', 'cyan');
    addLine('  a --set-dir [路徑]         修改本地存儲錨定目錄', 'purple');
    addLine('  a --init                  啟動 3-步驟智慧配置精靈', 'purple');
    return lines;
  }

  // 1.5 Handle web command a -w / a --web directly
  if (args[0] === '-w' || args[0] === '--web') {
    const sub = args[1];
    if (sub === 'stop') {
      try {
        await fetch('/api/web/state', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ state: 'standby' }),
        });
      } catch { /* ignore */ }
      addLine('🛑 [Web Engine] 網頁端管理引擎已切換為待機休眠狀態 (STANDBY)。', 'yellow', true);
      addLine('ℹ️  Rust 後台原生命令與背景服務正常運作中。在終端輸入 "a -w" 可隨時重新喚醒。', 'gray');
      return lines;
    } else if (sub === 'status') {
      let webState = 'active';
      try {
        const res = await fetch('/api/rust/status');
        if (res.ok) {
          const data = await res.json();
          webState = data.webState || 'active';
        }
      } catch { /* ignore */ }
      addLine('┌────────────────────────────────────────────────────────────┐', 'cyan');
      addLine('│ 🌐  Cyber-Forge 網頁端管理引擎狀態 (Web Engine)             │', 'cyan', true);
      addLine('├────────────────────────────────────────────────────────────┤', 'cyan');
      addLine(`│ 狀態模式 : ${(webState === 'active' ? '🟢 運行中 (ACTIVE)' : '💤 待機休眠 (STANDBY - 預設關閉)').padEnd(44)} │`, 'white');
      addLine(`│ 訪問位址 : ${('http://0.0.0.0:3000').padEnd(44)} │`, 'green');
      addLine(`│ 守護核心 : ${('Rust Native Daemon (/usr/local/bin/a)').padEnd(44)} │`, 'cyan');
      addLine('└────────────────────────────────────────────────────────────┘', 'cyan');
      return lines;
    } else {
      try {
        await fetch('/api/web/state', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ state: 'active' }),
        });
      } catch { /* ignore */ }
      addLine('🚀 [Web Engine] 網頁端管理引擎已啟動！(ACTIVE)', 'green', true);
      addLine('🌐 訪問入口: http://0.0.0.0:3000 (可隨時在儀表板或終端輸入 "a -w stop" 休眠)', 'cyan');
      return lines;
    }
  }

  if (rootCommand !== 'a') {
    addLine(`❌ 未知命令: ${rootCommand}。請輸入 'a' 或 'help' 檢視可用指令。`, 'red');
    return lines;
  }

  const currentYear = new Date().getFullYear().toString();
  const defaultFileName = `${currentYear}.note.gpg`;

  // 1. a (no arguments) -> Show system dashboard
  if (args.length === 0) {
    addLine('┌────────────────────────────────────────────────────────────┐', 'cyan');
    addLine('│ 🛡️  Cyber-Forge 賽博靈感管家 · 系統儀表板                   │', 'cyan', true);
    addLine('├────────────────────────────────────────────────────────────┤', 'cyan');
    addLine(`│ 📂 存儲目錄 : ${(config.noteDir || '未配置').padEnd(44)} │`, 'white');
    addLine(`│ 🔑 GPG 金鑰 : ${(config.gpgKeyId || '未配置').padEnd(44)} │`, 'green');
    addLine(`│ 🌐 Gist ID  : ${(config.gistId || '未配置').padEnd(44)} │`, 'cyan');
    addLine(`│ 🛡️ 憑證狀態 : ${(config.tokenDecrypted ? '已就緒 (Decrypted)' : '未配置').padEnd(44)} │`, 'yellow');
    addLine('└────────────────────────────────────────────────────────────┘', 'cyan');
    addLine("用法: a [您的靈感創意/支援多行]   #自動解密拼接並整檔公鑰加密", 'gray');
    addLine("      a -a 或 a --all             #解密並列印今年本地筆記", 'gray');
    addLine("      a -s 或 a --sync            #推送今年加密筆記至雲端 Gist", 'gray');
    addLine("      a -l 或 a --list            #列出雲端 Gist 所有檔案清單", 'gray');
    addLine("      a -d --all                  #批量下載雲端 Gist 全部檔案", 'gray');
    addLine("      a -d [檔名] -o [目標路徑]    #指定檔名/路徑下載 (例: a -d 1.txt -o ./1.txt)", 'gray');
    addLine("      a -d [年份/檔名] [-x]        #下載雲端密文 (-x 自動解密)", 'gray');
    addLine("      a -r [關鍵字或行號]          #行級過濾剔除並重密存盤", 'gray');
    addLine("      a --init                    #配置引導精靈", 'gray');
    return lines;
  }

  // 2. a --init / a -i
  if (args[0] === '--init' || args[0] === '-i' || args[0] === 'init') {
    triggerInitWizard();
    addLine('🧙‍♂️ 正在啟動 Cyber-Forge 智慧互動式配置精靈...', 'cyan', true);
    return lines;
  }

  // 3. a --set-dir [path]
  if (args[0] === '--set-dir' || args[0] === '-dir' || args[0] === '--dir') {
    if (args.length < 2) {
      addLine("❌ 錯誤：請提供目標目錄路徑。範例: a --set-dir ~/MyNotes", 'red');
      return lines;
    }
    const newDir = args[1];
    const updated = { ...config, noteDir: newDir };
    saveAppConfig(updated);
    onConfigChange(updated);
    addLine(`✨ 筆記存儲目錄已成功切換為: "${newDir}"`, 'green', true);
    return lines;
  }

  // 4. a -a / a --all
  if (args[0] === '-a' || args[0] === '--all') {
    const target = args[1];
    let contentToDisplay = '';

    if (!target) {
      // Default: current year note
      const note = readNote(defaultFileName);
      if (!note || !note.content) {
        addLine('📂 今年本地還沒有任何靈感記錄哦！輸入: a 寫下你的第一筆靈感', 'yellow');
        return lines;
      }
      try {
        contentToDisplay = await decryptWithGpg(note.content, config.gpgKeyId);
      } catch (err) {
        addLine(`⚠️ [保密局] 解密失敗: ${err instanceof Error ? err.message : String(err)}`, 'red');
        return lines;
      }
    } else if (target.length === 4 && /^\d+$/.test(target)) {
      // Specified year
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
      // Local note or Gist file
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

    // Render with alternating Green and Cyan colors (matching color.rs in Rust!)
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

  // 5. a -s / a --sync
  if (args[0] === '-s' || args[0] === '--sync') {
    if (!config.gistId) {
      addLine('❌ 錯誤：未配置雲端 Gist ID。請執行 a --init 或在設定中填入 Gist ID。', 'red');
      return lines;
    }
    if (!config.tokenDecrypted) {
      addLine('❌ 錯誤：未配置 GitHub Token。請執行 a --init 設定具備 gist 權限之 Token。', 'red');
      return lines;
    }

    const isRaw = args.includes('--raw') || args.includes('-u');
    const customTarget = args.slice(1).find((a) => a !== '--raw' && a !== '-u' && !a.startsWith('-'));

    let remoteFileName = defaultFileName;
    let payload = '';

    if (customTarget) {
      const note = readNote(customTarget);
      if (!note) {
        addLine(`❌ 錯誤：找不到本地檔案【${customTarget}】`, 'red');
        return lines;
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
        addLine('📂 本地空空如也，沒有什麼好同步的。請先寫入靈感: a [內容]', 'yellow');
        return lines;
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
    return lines;
  }

  // 6. a -l / a --list
  if (args[0] === '-l' || args[0] === '--list') {
    if (!config.gistId) {
      addLine('❌ 錯誤：未配置雲端 Gist ID。請執行 a --init 進行設定。', 'red');
      return lines;
    }
    addLine('📡 [雲端雷達] 正在掃描 GitHub Gist 倉庫物資清單...', 'cyan');
    try {
      const files = await listGistFiles(config.gistId, config.tokenDecrypted || '');
      const cleanGistId = config.gistId.split('/').pop() || config.gistId;
      const gistUrl = `https://gist.github.com/${cleanGistId}`;
      addLine(`🌐 倉庫網址 : ${gistUrl}`, 'cyan', true);
      addLine(`📋 雲端現有密文包裹清單 (共 ${files.length} 個)：`, 'white', true);
      addLine('------------------------------------', 'gray');
      files.forEach((f) => {
        addLine(`📦 ${f.filename} (${((f.size || 0) / 1024).toFixed(2)} KB)`, 'cyan');
      });
      addLine('------------------------------------', 'gray');
      addLine("💡 可使用 'a -d [檔名]' 下載，或 'a -d [檔名] -x' 下載並解密還原。", 'green');
    } catch (e) {
      addLine(`⚠️ 獲取清單失敗: ${e instanceof Error ? e.message : String(e)}`, 'red');
    }
    return lines;
  }

  // 7. a -d / a --download (支援 --all 批量下載、-o 自訂輸出路徑，加 ./ 下載到本地)
  if (args[0] === '-d' || args[0] === '--download') {
    if (!config.gistId) {
      addLine('❌ 錯誤：未配置雲端 Gist ID。請先執行 a --init。', 'red');
      return lines;
    }
    const shouldDecrypt = args.includes('-x') || args.includes('--decrypt');
    const isAll = args.includes('--all') || args.includes('-a');

    // 解析 -o / --out / --output 參數
    let outPath: string | undefined = undefined;
    for (let i = 1; i < args.length; i++) {
      if (args[i] === '-o' || args[i] === '--out' || args[i] === '--output') {
        if (i + 1 < args.length) {
          outPath = args[i + 1];
        }
        break;
      }
    }

    // 🌟 支援 a -d --all 批量下載全部檔案
    if (isAll) {
      addLine('📡 [雲端檢索] 正在掃描 GitHub Gist 倉庫檔案清單以進行批量下載...', 'cyan');
      try {
        const fileItems = await listGistFiles(config.gistId, config.tokenDecrypted || '');
        if (fileItems.length === 0) {
          addLine('ℹ️ 雲端 Gist 倉庫目前無任何檔案。', 'yellow');
          return lines;
        }

        const outDir = outPath || './';
        addLine(`☁️ [雲端同步] 開始批量下載全部 ${fileItems.length} 個檔案至本地 ${outDir === './' ? '本地工作目錄' : outDir}...`, 'cyan', true);

        let successCount = 0;
        for (let idx = 0; idx < fileItems.length; idx++) {
          const item = fileItems[idx];
          const remoteFileName = item.filename;

          try {
            const remoteContent = await fetchFromGist(config.gistId, remoteFileName, config.tokenDecrypted || '');
            let finalContent = remoteContent;
            let localFileName = remoteFileName;

            if (shouldDecrypt && remoteFileName.endsWith('.gpg')) {
              try {
                finalContent = await decryptWithGpg(remoteContent, config.gpgKeyId);
                localFileName = remoteFileName.replace(/\.gpg$/, '');
              } catch (e) {
                // 如果解密失敗則保留原樣
              }
            }

            // 本地存儲目的地
            const targetFilePath = outDir.endsWith('/')
              ? `${outDir}${localFileName}`
              : `${outDir}/${localFileName}`;

            // 寫入本地磁碟（伺服器工作目錄）
            try {
              await fetch('/api/notes/save-local', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                  filePath: targetFilePath,
                  content: finalContent,
                }),
              });
            } catch {}

            // 同步加入前端筆記庫
            const noteFile: NoteFile = {
              filename: localFileName,
              year: /^\d{4}/.exec(localFileName)?.[0],
              isEncrypted: !shouldDecrypt && finalContent.includes('-----BEGIN PGP MESSAGE-----'),
              content: remoteContent,
              decryptedContent: shouldDecrypt ? finalContent : undefined,
              lastModified: Date.now(),
            };
            saveNote(noteFile);

            addLine(
              `  [${idx + 1}/${fileItems.length}] ✨ 已下載: ${targetFilePath} (${(finalContent.length / 1024).toFixed(2)} KB)${
                shouldDecrypt && remoteFileName.endsWith('.gpg') ? ' [已解密還原]' : ''
              }`,
              'green'
            );
            successCount++;
          } catch (err) {
            addLine(`  [${idx + 1}/${fileItems.length}] ⚠️ 下載失敗 (${remoteFileName}): ${err instanceof Error ? err.message : String(err)}`, 'red');
          }
        }

        onNotesChange(loadAllNotes());
        addLine(`\n✨ 全部下載完成！共成功下載 ${successCount}/${fileItems.length} 個檔案至本地。`, 'green', true);
      } catch (e) {
        addLine(`⚠️ 批量獲取清單失敗: ${e instanceof Error ? e.message : String(e)}`, 'red');
      }
      return lines;
    }

    // 🌟 單檔案下載（支援 a -d 1.txt -o ./1.txt，-o 參數自訂檔名，加 ./ 下載文件到本地）
    let rawTarget: string | undefined = undefined;
    for (let i = 1; i < args.length; i++) {
      if (args[i] === '-o' || args[i] === '--out' || args[i] === '--output') {
        i++; // 跳過 -o 後面的參數值
        continue;
      }
      if (args[i] === '-x' || args[i] === '--decrypt' || args[i] === '-v' || args[i] === '--verbose') {
        continue;
      }
      if (!args[i].startsWith('-') && !rawTarget) {
        rawTarget = args[i];
      }
    }

    const target = rawTarget || currentYear;
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

      // 決定目標路徑：
      // 若有 -o 參數（如 -o ./1.txt），以 -o 為準；或原 target 含有 ./ / 等路徑
      let targetLocalPath = outPath;
      if (!targetLocalPath) {
        if (target.startsWith('./') || target.startsWith('../') || target.startsWith('/')) {
          targetLocalPath = target;
        } else {
          targetLocalPath = `./${localFileName}`;
        }
      } else if (targetLocalPath.endsWith('/') || targetLocalPath === '.') {
        targetLocalPath = `${targetLocalPath.replace(/\/$/, '')}/${localFileName}`;
      }

      // 寫入本地磁碟 (伺服器本地工作目錄，加 ./ 下載文件到本地)
      let savedToDisk = false;
      try {
        const saveRes = await fetch('/api/notes/save-local', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            filePath: targetLocalPath,
            content: finalContent,
          }),
        });
        if (saveRes.ok) {
          savedToDisk = true;
        }
      } catch (err) {
        console.warn('寫入本地磁碟失敗:', err);
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

      if (savedToDisk || outPath) {
        const isLocalCwd = (outPath && outPath.startsWith('./')) || target.startsWith('./');
        addLine(
          `✨ 檔案已成功下載並精確儲存至${isLocalCwd ? '當前終端機所在的本地工作目錄' : '本地路徑'}: ${targetLocalPath} (大小: ${(finalContent.length / 1024).toFixed(2)} KB)${
            shouldDecrypt ? ' [已完成解密還原]' : ''
          }`,
          'green',
          true
        );
      } else {
        addLine(`✨ 檔案已成功下載至本地庫房：${localFileName} (大小: ${(finalContent.length / 1024).toFixed(2)} KB)${shouldDecrypt ? ' [已完成解密還原]' : ''}`, 'green', true);
      }
    } catch (e) {
      addLine(`⚠️ 下載失敗: ${e instanceof Error ? e.message : String(e)}`, 'red');
    }
    return lines;
  }

  // 8. a -r / a --remove
  if (args[0].startsWith('-r') || args[0] === '--remove') {
    let targetExpr = '';
    if (args[0] === '-r' || args[0] === '--remove') {
      if (args.length < 2) {
        addLine("❌ 錯誤：請指定要刪除的倒數行號、區間或關鍵字。範例: a -r1, a -r1-5, a -r 買咖啡", 'red');
        return lines;
      }
      targetExpr = args[1];
    } else {
      targetExpr = args[0].substring(2);
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

    // Check if range: e.g. 1-5
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
      // Keyword match
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
