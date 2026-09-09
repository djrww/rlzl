# ADR-0002：以 `macro_rules!` + `const fn` 分工實現自拓展

- **狀態**：Accepted
- **日期**：2026-09-09（記錄日期）
- **決策者**：專案作者
- **取代**：無

## Context

需求是「在 Rust 宏中實現 Welzl，並讓該宏在演算下自拓展到演算法極限或宏極限」。

「自拓展」與「極限」兩詞決定了架構必須同時觸碰**兩套獨立的編譯器限制**：

- `macro_rules!` 的遞迴展開上限（`#![recursion_limit]`）
- const-eval 的直譯器堆疊框架上限（Miri stack frame）

這兩者是**不同機制**，不會互相影響 —— 提高 `recursion_limit` 不會讓 const-eval 遞迴更深。

備選方案：

1. 純 `macro_rules!` tt-munching：宏本身做全部算術
2. 純 `const fn`：宏只是薄包裝
3. proc-macro：在編譯期跑任意 Rust
4. **混合**：宏做遞迴展開與點集分派，數值核心在 `const fn`

## Decision

**採用方案 4（混合）。**

- `macro_rules!` 負責：點集字面量的解析與分派、遞迴生成更深層的宏調用（`selfgen`）、
  生成宏塔（`welzl_tower!`）。
- `const fn` 負責：全部數值運算（`exact.rs`、`welzl.rs`）。
- 明確**拒絕 proc-macro**：會引入 `syn`/`quote` 依賴，且 proc-macro 在
  獨立行程中執行，觸碰到的是行程記憶體上限而非編譯器的宏/const-eval 極限 ——
  那不是本專案想量測的東西。

## Consequences

### 正面

- 兩套極限都能被獨立量測（見 `LIMITS.md` 與 `tools/probe.py`）。
- 零外部依賴。
- 數值核心可同時用於編譯期與執行期，無需重複實作。

### 負面

- 純宏路徑與 const-eval 路徑的極限差異極大（126 點 vs 120 點 vs 迭代版 4,000,000 點），
  使用者需要理解該用哪一條 —— 這催生了 ADR-0003 的自適應分派。
- 宏錯誤訊息品質差，除錯成本高。

### 實測結果（rustc 1.98.1 / x86-64 / 2 vCPU / 1.9 GB RAM）

| 維度 | 極限 | 攔下它的 |
|---|---|---|
| 純宏 tt-munching | 126 / 14,336 點 | recursion limit / SIGSEGV |
| const-eval 遞迴 | 120 點 | `E0080 maximum stack frames` |
| const-eval 迭代 | 59,392 / 4,000,000 點 | lint / OOM |
| 自我生成宏塔 | 16 層（65,536 點） | — |

**天花板是記憶體，不是堆疊。**
