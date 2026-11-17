# 變更紀錄

本專案的重要變更都記錄在這份文件。格式參考 [Keep a Changelog](https://keepachangelog.com/zh-TW/1.1.0/)，版本號遵循 [Semantic Versioning](https://semver.org/lang/zh-TW/)。

每個版本都會同時發布到 [GitHub Releases](https://github.com/doggy8088/mhtml/releases)、npm 的 [`mhtml-cli`](https://www.npmjs.com/package/mhtml-cli)，以及從 0.4.0 起的 crates.io [`mhtml`](https://crates.io/crates/mhtml)。

## [0.5.0] - 2026-11-18

### 新增

- 新增 Rust 函式庫目標（crate 名稱同為 `mhtml`），命令列改為其上的薄殼，兩者共用同一條產出管道。
  - `mhtml::to_html(markdown, &Options)` 輸入 Markdown，輸出完整的單一檔案 HTML 頁面（`Document`）。
  - `Document` 提供 `html`、`body`、`title`、`lang`、`warnings` 等欄位，並實作 `Display`。
  - `Options` 對應命令列旗標與設定檔，`Options::default()` 與「無旗標、無設定檔」的預設完全一致，並提供 `with_title`、`with_theme`、`with_css` 等串接方法。
  - 低階的 `render::render`、`Page`、`style`、`theme` 維持公開，可單獨使用。
- 新增 Cargo feature `cli`（預設開啟）。以 `default-features = false` 相依時為純轉換函式庫，不會引入 clap、toml、serde、open、dirs。
- 從 0.4.0 起發布到 crates.io，之後的版本由 release workflow 以 Trusted Publishing（OIDC）自動發布。

### 變更

- 樣式捷徑（accent、width、font、font-size）的錯誤型別改為具名的 `InvalidCssValue`，函式庫對外不再外洩 `anyhow`。命令列的錯誤訊息與結束碼不變。
- crate 只打包 `src/`、`assets/`、LICENSE 與 README，不再夾帶測試、文件截圖、npm 與 CI 檔案。

### 其他

- README（中、英）新增「當成 Rust 函式庫使用」章節。
- CI 新增不含 `cli` feature 的建置檢查，以及 `cargo doc` 警告檢查。
- 新增這份變更紀錄。

## [0.4.0] - 2026-11-05

### 新增

- 可省略 INPUT 從 stdin 讀入：`cat a.md | mhtml` 不必再加 `-` 就能預覽管線輸入的 Markdown。
- stdin 有管線資料又給了檔名 INPUT 時，以結束碼 2 報錯並提示改用 `-`，避免管線被靜默忽略（僅 Unix）。
- 直接在終端機執行 `mhtml`，或 stdin 只有空白時，以結束碼 2 顯示用法說明。

### 變更

- 預覽暫存檔名由 `<名稱>-<pid>.html` 改為 `<名稱>-<mhtml 版本>-<雜湊>.html`。雜湊取最終 HTML 的 FNV-1a 64 位元，同一頁面、同一版本永遠得到同一路徑，內容或旗標不同則是不同檔案，不會覆蓋瀏覽器開著的頁面。

## [0.3.0] - 2026-11-05

### 新增

- 自適應呈現（預設開啟，`--no-enhance` 關閉）：
  - 依文件類型與結構自動裝飾：章節編號徽章、主題圖示、閱讀時間、屬性表、卡片清單、狀態徽章等。
  - 表格可排序（三態）、篩選、調整欄寬、固定表頭，窄螢幕自動堆疊成卡片。
  - 側欄目錄標示目前章節、閱讀進度條與回頂部按鈕。
  - 程式碼區塊右上角新增 hover 顯示的複製圖示。
- 表格欄位可拖曳搬移順序，也能用鍵盤 Alt/Ctrl+←/→ 搬移；排序門檻放寬為 2 個資料列。
- CJK 友好強調：`**標籤：**內容`、`**「引號」**` 等中文常見寫法可正確渲染成粗體，英文維持 CommonMark 行為。
- 未配對強調的診斷警告（含行號），以及讓文件流程把關用的 `--check` 旗標。

### 修正

- 價格不再被誤判為行內數學式，例如 `$3.69$148`、`$2.44 > glm $1.43`。

## [0.2.2] - 2026-10-28

### 修正

- 保留圖片的 `data:image/...` 內嵌圖。之前被 sanitize 移除，導致內嵌圖片無法顯示；其他屬性上的 `data:` URL 仍一律移除。

## [0.2.1] - 2026-10-26

### 修正

- 行內數學容錯：`$ L $`、`$L $`、`$ L$` 這類分隔符號內側有空白的寫法也能解析；像句子的配對與價格維持原樣，不會被誤判成公式。

## [0.2.0] - 2026-10-26

### 新增

- 支援 Mermaid 圖表：` ```mermaid ` 區塊由頁面內嵌的 Ondine（Rust/WASM）渲染器在瀏覽器即時轉成 SVG，維持單一檔案、離線可用。
  - 只有文件含 Mermaid 圖表時才內嵌渲染器，一般頁面仍不含 JavaScript。
  - 產生的 SVG 會先經過白名單清理；圖表顏色沿用主題變數。
  - 新增 `--no-mermaid`／`--mermaid` 旗標與設定檔 `mermaid` 選項。

## [0.1.2] - 2026-10-15

### 修正

- 拒絕把 HTML 輸出覆寫到 Markdown 原始檔：OUTPUT 與 INPUT 是同一個檔案（包括相對路徑、絕對路徑與符號連結等不同寫法）時，以結束碼 1 結束，原始檔不會被動到。

### 其他

- 重寫 `--help`，讓人類與 AI agent 都能直接學會使用：加入快速上手、給 agent 的使用原則、輸出約定、範例與疑難排解；`-h` 維持簡短。
- README 加入淺色、深色預覽截圖。
- 新增 `scripts/bump-version.js`、`make bump` 與 bump-and-release 專案 skill。

## [0.1.1] - 2026-10-15

### 其他

- npm 改用 Trusted Publishing（OIDC）發布，不再需要 `NPM_TOKEN`。

## [0.1.0] - 2026-10-15

第一個公開版本。

### 新增

- 把 Markdown 轉成美觀、自足的單一 HTML 檔案：`mhtml input.md output.html`，也能用 `-` 與 stdin、stdout 串接。
- 支援 CommonMark 與 GFM 擴充（表格、刪除線、任務清單、腳註、Alerts）、定義清單、標題屬性與 YAML front matter；標題自動產生 GitHub 風格錨點。
- 預設 sanitize 輸出，移除 `<script>`、事件屬性等危險內容；`--unsafe-html` 可關閉。
- 深淺色主題（`--theme auto|light|dark`）、內建樣式，以及多層次的自訂 CSS（`--css`、CSS 變數、樣式捷徑）。
- 預覽模式：省略 OUTPUT 時寫入暫存檔並以預設瀏覽器開啟，相對路徑的圖片仍能正常顯示。
- 程式碼區塊語法醒目提示（支援深淺色）。
- 數學公式（以 MathML 輸出，離線可用、不需 JavaScript）與 `--toc` 目錄。
- 依作業系統標準位置讀取設定檔，優先順序為內建預設 < 設定檔 < 命令列旗標。
- npm 包裝層 `mhtml-cli`：安裝時下載對應平台的預先編譯執行檔並以 SHA-256 驗證。
- 提供 macOS、Linux、Windows 的 x64 與 arm64 預先編譯執行檔。

[0.5.0]: https://github.com/doggy8088/mhtml/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/doggy8088/mhtml/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/doggy8088/mhtml/compare/v0.2.2...v0.3.0
[0.2.2]: https://github.com/doggy8088/mhtml/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/doggy8088/mhtml/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/doggy8088/mhtml/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/doggy8088/mhtml/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/doggy8088/mhtml/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/doggy8088/mhtml/releases/tag/v0.1.0
