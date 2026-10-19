# Shared brief for subsystem research (Tamagotchi Paradise, White Glacier edition)

Goal: understand how the game works completely, so it can be re-implemented from scratch in Rust (deterministic fixed-tick core, data-driven). Your notes feed a spec document.

## Files (all under /Users/seith/Desktop/tama-paradise-web)

- `re/decomp-all/ram.c`   Ghidra decompilation of PRAM code (runs at 0x00000000, 64 KB, decrypted). ~16k lines.
- `re/decomp-all/XIP.c`   Decompilation of XIP code (runs at 0x10000000). ~87k lines.
  Each function starts with a line `// ==== <addr> <name>`.
- `re/index.tsv`          One row per function: addr, name, bytes, #callers, #callees, strings referenced.
- `re/names.txt`          Names already assigned (g_tama_*, g_care_*, cmd_*, mon_*).
- `docs/re-notes.md`      Prior findings: address map, UART monitor commands, pet state struct. READ THIS FIRST.
- `dumps/paradise-glacier-20261019-113049.bin`  Full 16 MiB flash (read-only reference; never modify).
- `dumps/pram.bin`        Decrypted PRAM (address 0x0).
- `dumps/xip.bin`         XIP code+rodata; vaddr 0x10000000 = xip.bin offset 0 = flash offset 0x11000.
- Flash data window: vaddr 0x60000000 + flash offset. SRAM 0x18000000–0x1803ffff.
- `dumps/*_sprites.zip`, `*_figuren.zip`  PNG exports made by Paramix Studio (community tool).
- `dumps/glacier-20261019-1923.tamasave`  Save backup (magic TAMASAVE, v3).

Address conversion: XIP vaddr → xip.bin offset = vaddr - 0x10000000. Flash-window vaddr 0x60xxxxxx → flash offset = vaddr - 0x60000000.

## Rules

- Read-only on everything above. Do NOT open or modify the Ghidra project in `re/ghidra` (another process may hold the lock).
- Do NOT download, build, or run third-party code (e.g. Capybara, Paramix). Reading web docs (curl/WebFetch of README/markdown) is fine. GMMan/tama-para-research has format docs under formats/ and hardware/.
- You may write small Python scripts (run with `python3 -I`) in your scratch area to parse data tables out of the dumps.
- Thumb code builds addresses with MOVW/MOVT; Ghidra shows them as DAT_xxxxxxxx or s_xxx strings.
- Call PRAM functions appear in XIP as thunk_EXT_FUN_0000xxxx; their bodies are in ram.c as FUN_0000xxxx.

## Output

Write your findings to the file you are assigned, in Traditional Chinese (技術名詞、函式名、程式碼保持英文). Structure:

1. 摘要（5–10 行：這個子系統怎麼運作）
2. 詳細規則：each rule with concrete numbers (timers in seconds/ticks, thresholds, probabilities, table sizes), and the evidence: function address(es) + one-line reason. Mark each rule **[確認]** (read directly in code) or **[推測]** (inferred).
3. 資料表：any tables you located (address, entry size, count, field layout), plus extracted contents if feasible (CSV under docs/spec/data/ is fine).
4. 新命名：list `addr name` pairs you are confident about (format of re/names.txt) so they can be applied later.
5. 未解問題：what remains unknown and where to look.

Be precise; a remake will be built from this. Prefer depth on the core rules over breadth. When done, reply with a ≤15-line summary of the most important findings.
