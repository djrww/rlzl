# ADR-0004：對靜默溢位一律提供 checked 版本

- **狀態**：Accepted
- **日期**：2026-09-09（記錄日期）
- **決策者**：專案作者
- **取代**：無

## Context

下游應用（`segdist.rs` 的 `Rat::cmp`）在座標 ≥1e8 時**給出錯誤答案且不 panic**。

根因有兩層：

1. Rust release 模式的整數溢位行為是 wrapping，不 panic。
2. **`-C debug-assertions` 不跨 crate 邊界傳播** —— 即使測試以 debug 模式編譯，
   函式庫 crate 內部的溢位檢查仍然是關閉的。

這代表：**測試通過不能證明沒有溢位**。

驗證方式也是個問題 —— 用 i128 去驗證 i128 的溢位是循環論證。
可信方法是用 Python 的任意精度整數反查真實位元需求：

| 座標上界 C | `Rat::cmp` 實際需要 |
|---|---|
| 1e6 | 116 bit |
| 1e8 | 156 bit |
| 1e9 | 176 bit |
| 1e12 | 231 bit |

（i128 有效範圍為 127 bit）

## Decision

1. **凡是可能溢位的比較與判定，一律額外提供 `_checked` 版本**，
   溢位時回傳 `None` 而非錯誤答案。已實作 `Rat::cmp_checked()`。
2. **位元需求一律以外部任意精度工具（Python）反查驗證**，不以 i128 自證。
3. 在文件中明確標示每個 API 的安全座標上界。

## Consequences

### 正面

- 使用者能區分「答案是 0」與「算不出來」。
- `ratcmp_will_overflow` 讓呼叫端可以事前判斷。

### 負面

- API 表面積加倍。
- checked 版本有額外成本，且**預設 API 仍然是不安全的那個** ——
  這是為了效能與 const-eval 相容性的取捨，屬於已知風險。

### 未解決

- 目前沒有 lint 或型別系統機制強制使用者選 checked 版本。
  若要商業化（見 ROADMAP.md Phase 7），應考慮把 unchecked 版本改名為
  `_unchecked` 並標記 `unsafe` 或加 `#[must_use]` 提示。
