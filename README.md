# mhtml

[English](README.en.md) | **繁體中文**

把 Markdown 轉成漂亮、**單一檔案、可離線開啟**的 HTML 頁面。以 Rust 撰寫，單一原生執行檔，Windows／macOS／Linux 皆可使用，並提供 npm 安裝方式。

```sh
mhtml README.md                    # 轉成暫存 HTML 並在預設瀏覽器開啟，快速預覽
mhtml README.md README.html        # 輸出成 HTML 檔案
mhtml README.md out.html --theme dark --css my.css
```

## 特色

- **預覽模式**：只給輸入檔，就會產生暫存 HTML 並用預設瀏覽器開啟（相對路徑的圖片仍可正常顯示）。
- **深色／淺色主題**：`--theme auto|light|dark`，`auto` 會跟隨系統設定，不需要 JavaScript。
- **容易客製化樣式**：用 CSS 變數、快捷旗標、自訂 `.css` 檔或設定檔調整外觀。
- **設定檔**：把常用偏好寫在 OS 標準設定資料夾，命令列旗標可隨時覆寫。
- **常見語法幾乎都支援**：CommonMark、GFM（表格、任務清單、刪除線、腳註、Alerts）、定義清單、front matter、程式碼語法上色、LaTeX 數學公式（MathML，離線）、目錄。
- **預設安全**：輸出會經過 sanitize，移除 `<script>`、`onerror`、`javascript:` 等；需要時可用 `--unsafe-html` 關閉。
- **輸出自成一體**：CSS 內嵌，沒有外部相依，可直接寄送或存檔。

## 安裝

### npm

```sh
npm install -g mhtml-cli     # 之後使用 `mhtml` 指令
npx mhtml-cli README.md      # 或不安裝直接執行
```

> npm 上的 `mhtml` 名稱已被其他套件使用，因此套件名為 `mhtml-cli`，指令名稱仍是 `mhtml`。
> 套件本身只是極簡的包裝層（零相依），安裝時會從 GitHub Releases 下載對應平台的原生執行檔並驗證 SHA-256。
> 可用環境變數 `MHTML_DOWNLOAD_BASE`（鏡像站）、`MHTML_BINARY_PATH`（使用本機執行檔）、`MHTML_SKIP_DOWNLOAD` 調整。

### 預先編譯的執行檔

到 [Releases](https://github.com/doggy8088/mhtml/releases) 下載 `mhtml-<平台>.gz`（macOS／Linux／Windows，x64／arm64），解壓縮後放進 `PATH`。

### 從原始碼

```sh
cargo install --git https://github.com/doggy8088/mhtml   # 需要 Rust 1.88+
```

## 使用方式

```text
mhtml [OPTIONS] <INPUT> [OUTPUT]
```

| 用法 | 說明 |
| --- | --- |
| `mhtml a.md` | 預覽模式：寫入暫存檔並用瀏覽器開啟 |
| `mhtml a.md a.html` | 輸出到檔案（不開瀏覽器，加 `--open` 才會開） |
| `mhtml - -` | 從 stdin 讀、寫到 stdout，例如 `cat a.md \| mhtml - -` |
| `mhtml a.md --no-open` | 預覽模式但不開瀏覽器，只印出暫存檔路徑 |

### 常用旗標

| 旗標 | 說明 |
| --- | --- |
| `-t, --theme <auto\|light\|dark>` | 色彩主題（預設 `auto`） |
| `-c, --css <FILE\|URL>` | 加入自訂樣式，可重複；本機檔案內嵌，URL 以 `<link>` 引用 |
| `--no-default-style` | 不使用內建樣式，完全自己來 |
| `--accent <COLOR>` | 連結／強調色，例如 `"#e11d48"` |
| `--width <LENGTH>` | 內容欄最大寬度，例如 `960px`、`80ch` |
| `--font <FONT>` / `--font-size <LENGTH>` | 字型堆疊與基準字級 |
| `--title <TEXT>` | 網頁標題（預設：front matter `title` → 第一個 H1 → 檔名） |
| `--lang <LANG>` | `<html lang>`（預設 `en`） |
| `--toc` / `--toc-title <TEXT>` | 在內容前加入目錄與其標題 |
| `--no-highlight` | 關閉程式碼語法上色 |
| `--no-math` | 關閉 `$…$` / `$$…$$` 數學公式 |
| `--smart-punctuation` | 彎引號、破折號、刪節號 |
| `--unsafe-html` | 保留 Markdown 內的原始 HTML（含 script），僅用於可信任的內容 |
| `--base <URL>` | 相對連結／圖片的 `<base href>`（預覽模式預設為 Markdown 所在資料夾） |
| `--config <FILE>` / `--no-config` | 指定或略過設定檔 |
| `--init-config` / `--print-config-path` | 建立範本設定檔／印出設定檔位置 |

開關類旗標都有成對的反向版本（例如 `--toc` / `--no-toc`），用來在單次執行時覆蓋設定檔。

## 設定檔

不想每次都帶一長串參數，可以把偏好寫進設定檔。**設定檔與命令列旗標並存**，優先順序為：

> 內建預設 < 設定檔 < 命令列旗標

預設位置（依作業系統的慣例）：

| 系統 | 路徑 |
| --- | --- |
| macOS | `~/Library/Application Support/mhtml/config.toml` |
| Linux | `$XDG_CONFIG_HOME/mhtml/config.toml`（通常是 `~/.config/mhtml/config.toml`） |
| Windows | `%APPDATA%\mhtml\config.toml` |

也可以用 `--config <FILE>` 或環境變數 `MHTML_CONFIG` 指定其他位置。執行 `mhtml --init-config` 會在預設位置建立一份附說明、全部註解掉的範本（不會覆寫既有檔案）。

```toml
theme = "dark"                 # "auto" | "light" | "dark"
lang = "zh-TW"
accent = "#e11d48"
width = "900px"
font = "'Noto Sans TC', system-ui, sans-serif"
font_size = "17px"

toc = true
toc_title = "目錄"

highlight = true               # 程式碼語法上色
math = true                    # 數學公式
smart_punctuation = false
unsafe_html = false
default_style = true           # false = 不使用內建樣式
open_browser = true            # 預覽模式是否自動開啟瀏覽器

# 額外樣式：相對路徑以「設定檔所在資料夾」為基準，支援 "~/" 與 http(s) URL
css = ["my-style.css"]
custom_css = """
h1 { letter-spacing: .02em; }
"""
```

鍵名打錯時會直接報錯並列出所有合法鍵，不會被默默忽略。樣式的套用順序為：設定檔 `css` → `custom_css` → 命令列 `--css`，越後面越優先。

## 自訂樣式

內建樣式完全由 CSS 變數驅動，你的 CSS 通常只需要覆寫幾個變數：

```css
:root {
  --mhtml-accent: #e11d48;
  --mhtml-max-width: 70ch;
  --mhtml-font: Georgia, "Noto Serif TC", serif;
}

/* 針對深色模式 */
@media (prefers-color-scheme: dark) {
  :root { --mhtml-bg: #000; }
}
```

| 變數 | 用途 |
| --- | --- |
| `--mhtml-bg` / `--mhtml-fg` / `--mhtml-muted` | 背景／文字／次要文字色 |
| `--mhtml-accent` | 連結與強調色 |
| `--mhtml-border` | 分隔線與邊框 |
| `--mhtml-code-bg` / `--mhtml-code-fg` / `--mhtml-inline-code-bg` | 程式碼區塊與行內程式碼 |
| `--mhtml-quote-border` / `--mhtml-table-stripe` / `--mhtml-mark` | 引用線、表格斑馬紋、`<mark>` |
| `--mhtml-note` / `-tip` / `-important` / `-warning` / `-caution` | GFM Alerts 顏色 |
| `--mhtml-font` / `--mhtml-mono` / `--mhtml-font-size` / `--mhtml-line-height` | 字型與字級 |
| `--mhtml-max-width` / `--mhtml-radius` | 內容欄寬與圓角 |

`<html>` 帶有 `data-theme="auto|light|dark"` 屬性，可用 `html[data-theme="dark"]` 撰寫僅在特定主題生效的規則。程式碼上色的 token 使用 `.hl-*` class，同樣可以覆寫。

## 支援的 Markdown 語法

標題（含 `{#自訂-id}` 與自動錨點）、段落、粗體／斜體／刪除線、行內與區塊程式碼（語法上色）、連結與圖片、清單與巢狀清單、**任務清單**、**表格**（含對齊）、引用、**GFM Alerts**（`> [!NOTE]`）、分隔線、**腳註**、**定義清單**、**數學公式**（`$…$`、`$$…$$`、` ```math `）、YAML front matter、內嵌 HTML（`<details>`、`<kbd>`、`<img>` 等，經 sanitize）。

## 安全性

Markdown 允許夾帶原始 HTML。為了安全地預覽不可信的檔案，預設會以白名單 sanitize 輸出（移除 `<script>`、事件屬性、`javascript:` 連結等）。僅在內容可信時才使用 `--unsafe-html`。

## 開發

```sh
make help      # 列出所有常用任務
make check     # fmt + clippy + 全部測試（等同 CI）
make preview   # 在瀏覽器預覽 examples/example.md
make release   # 最佳化建置 target/release/mhtml
make tag       # 檢查版本一致後建立本機 vX.Y.Z tag
```

發佈：同步更新 `Cargo.toml` 與 `npm/package.json` 的版本號，然後 `git tag vX.Y.Z && git push --tags`；GitHub Actions 會建置六個平台的執行檔、建立 Release 並發佈 npm 套件（需要儲存庫密鑰 `NPM_TOKEN`）。

## 授權

[MIT](LICENSE) © 2026 Will 保哥
