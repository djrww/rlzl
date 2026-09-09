# 極限報告：宏能把 Welzl 推到多遠

環境：`rustc 1.98.1 (48a229cea)` / x86-64 Linux / 2 vCPU / 1.9 GB RAM。
所有數字皆由 `tools/probe.py` 以「指數擴張 + 二分搜尋」實測得出，非估計值。

## 總表

| # | 維度 | 極限 | 觸頂時間 | 攔下它的是誰 |
|---|------|------|----------|--------------|
| 1 | 純宏 tt-munching（預設） | **126 點** | <0.1 s | `recursion limit reached while expanding` |
| 2 | 純宏 tt-munching（limit=10⁶） | **14,336 點** | 32.7 s | `rustc interrupted by SIGSEGV`（真實 C 堆疊爆掉） |
| 3 | 宏吐出 n 個點字面量 + 實跑 Welzl | **≥131,072 點** | 5.6 s | 未觸頂（探測上限） |
| 4 | const-eval 遞迴 `welzl(P,R)` | **120 點** | <0.1 s | `E0080: reached the configured maximum number of stack frames` |
| 4b | **自適應遞迴 `@auto`**（燃料+續傳） | **500,000 點** | 38 s | 記憶體（1M 時 OOM-kill）；堆疊限制已解除 |
| 5 | const-eval 迭代 Welzl | **59,392 點** | 1.8 s | `long_running_const_eval` lint |
| 5b | 同上，`#![allow(long_running_const_eval)]` | **4,000,000 點** | 111 s | 記憶體（8M 時 OOM-kill） |
| 6 | 自我生成宏塔層數 | **16 層** | 4.0 s | 第 16 層 = 65,536 點，const-eval 工作量 2ⁿ 爆炸 |

## 四道天花板，各自的性質

**① `recursion_limit` = 128（宏層，軟性）**
每個 `welzl_count!` 的 tt-munch 消耗一層。126 是實測可過的最大值（餘下 2 層被外圍展開佔用）。
一行 `#![recursion_limit = "1000000"]` 就能解除 —— 這是**可調參數**，不是硬牆。

**② rustc 的原生堆疊（宏層，硬性）**
把 limit 開到 10⁶ 後，宏遞迴不再受邏輯限制，改由 rustc 自己的 C 堆疊決定。
14,336 層時 **SIGSEGV** —— 編譯器自身堆疊溢位。這是宏遞迴真正的物理極限，無法用屬性繞過。

**③ const-eval 的 interpreter 堆疊（數值層，硬性）**
`welzl_rec` 深度 = |P|，120 點即 `E0080`。Miri 的堆疊上限（預設 ~100 frames）與 `recursion_limit` 是**兩套獨立機制**——
把 `recursion_limit` 開到 10⁶ 對它毫無作用。這正是把 Welzl 寫成迭代形式的理由：
同樣的算法，`welzl_arr` 能跑 4,000,000 點，是遞迴版的 **33,000 倍**。

**③b 自適應遞迴 —— 把硬性天花板轉成軟性（`src/adaptive.rs`）**
③ 之所以是硬牆，在於 const-eval 無 `catch_unwind`，`E0080` 直接終結編譯。
解法是不去撞它：遞迴攜帶燃料，在觸頂前一步 `generation += 1` 並移交給迭代續解。
因為 `resume(P, R)` 求的是同一個數學物件 `MEC(P | R ⊂ ∂D)`，交棒點不影響答案。
遞迴深度從此鉗在 `fuel ≤ 64`，|P| 不再受限：**120 → 500,000 點，提升 4,166 倍**，
且 `fuel=1` 與 `fuel=48` 的結果逐位元相同（已寫成編譯期斷言）。

**④ `long_running_const_eval`（數值層，軟性）**
59,392 點時觸發。它是 **lint 而非錯誤**，`#![allow]` 即可解除，之後只剩實際資源限制：
100k → 4.4 s，400k → 18.5 s，1M → 78 s，2M → 84 s，**4M → 111 s**，全部成功。
8,000,000 點時被 OOM killer 終結（rc=137，1.9 GB RAM），這是最後一道硬牆。

## 關鍵發現：極限取決於「寫法」而非「算法」

同一個 Welzl 算法、同一台機器，**遞迴 120 點 vs 迭代 4,000,000 點**。
差距不在算法複雜度（兩者都是期望 O(n)），而在於它撞上哪一道天花板：
遞迴形式把問題規模映射到 const-eval 的堆疊深度（最稀缺的資源），
迭代形式只映射到 step 數（最充裕的資源）。

宏塔的 16 層則展示了兩層極限的**交互**：層數線性消耗宏遞迴（充裕），
但規模 2ⁿ 消耗 const-eval（稀缺），所以先死在 ④ 而不是 ①。

## 副產物：執行期成本歸零

9 點問題編譯後反組譯：

```asm
get_r2:  movl $5125, %eax      ; r² = 5125/169，直接就是答案
         xorl %edx, %edx
         retq
```

全檔 `call`/`jmp`/條件跳轉計數 = **0**。整個 Welzl —— 洗牌、三層迴圈、
i128 行列式、GCD 約分 —— 在二進位檔中不留任何痕跡，只剩一個常數。

## 重現

```bash
cargo build --release
python3 tools/probe.py macro   # ①②③
python3 tools/probe.py const   # ④⑤
python3 tools/probe.py tower   # ⑥
```
