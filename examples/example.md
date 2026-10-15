---
title: mhtml 範例文件
---

# mhtml 範例文件

這份文件示範 **mhtml** 支援的常見 Markdown 語法。試試看：

```sh
mhtml examples/example.md                       # 在瀏覽器預覽
mhtml examples/example.md out.html --theme dark # 輸出深色主題
```

## 文字樣式

**粗體**、*斜體*、~~刪除線~~、`行內程式碼`、[連結](https://github.com/doggy8088/mhtml)，
以及鍵盤按鍵 <kbd>Ctrl</kbd> + <kbd>C</kbd>。

> 這是一段引用。
> 可以有多行。

> [!NOTE]
> GitHub 風格的提示區塊：NOTE、TIP、IMPORTANT、WARNING、CAUTION。

> [!WARNING]
> 小心這個警告區塊。

## 清單

- 項目 A
  - 巢狀項目
- 項目 B

1. 第一步
2. 第二步

- [x] 已完成的任務
- [ ] 待辦的任務

## 表格

| 語法     | 支援 | 備註           |
| :------- | :--: | -------------: |
| 表格     |  ✅  |           GFM |
| 任務清單 |  ✅  |           GFM |
| 數學公式 |  ✅  |        MathML |

## 程式碼

```rust
fn main() {
    let name = "mhtml";
    println!("Hello, {name}!");
}
```

```typescript
interface User { id: number; name: string }
const greet = (u: User): string => `Hi ${u.name}`;
```

## 數學公式

行內公式：$E = mc^2$，區塊公式：

$$
\int_0^\infty e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}
$$

## 其他

腳註在這裡[^1]。

term
: 定義清單的內容

<details>
<summary>展開看看</summary>

被收合的內容。

</details>

---

[^1]: 這是腳註的內容。
