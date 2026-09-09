# welzl_macro — 在 Rust 宏中實現 Welzl 算法，並推到極限

在**編譯期**用 `macro_rules!` + `const fn` 求解最小包覆圓（Minimum Enclosing Circle）。
i128 精確有理數算術，零浮點誤差，執行期成本為零。

```rust
const C: Circle = welzl!((0, 0), (4, 0), (0, 3));
// 圓心 = 4/2, 3/2   半徑² = 25/4   —— 精確值，不是 6.2500000001
```

## 設計：宏層 / 數值層分工

| 層 | 負責 | 天花板 |
|----|------|--------|
| `macro_rules!` | token 解析、遞迴計數、`\|R\| ≤ 3` 基底分派、自我生成 | `recursion_limit` → rustc 原生堆疊 |
| `const fn` | i128 精確算術、Welzl 主迴圈 | const-eval 堆疊 / step limit |

把數值運算放進 `const fn`（而非硬塞進宏的 token 層）是刻意的：
純宏算術只能做 Peano 整數，且每個運算都燒一層 `recursion_limit`；
`const fn` 讓宏得以專注在它真正擅長的事 —— 語法層的展開與分派。

## 用法

```rust
welzl!((x,y), ...)            // 迭代 Welzl（預設，可撐最大 N）
welzl!(@rec (x,y), ...)       // 教科書遞迴形式 welzl(P, R)（120 點觸頂）
welzl!(@auto (x,y), ...)      // 自適應：觸頂自帶遞增後返回演算法
welzl!(@trace (x,y), ...)     // 同上，附 fuel/gens/depth/rounds 遙測
welzl!(@fuel F; (x,y), ...)   // 指定燃料單輪求解，觀察交棒點
welzl!(@brute (x,y), ...)     // O(n³) 暴力法，驗證用
welzl!(@seed S; (x,y), ...)   // 指定洗牌種子
welzl!(@checked (x,y), ...)   // 編譯期座標範圍斷言
welzl!(@radius2 (x,y), ...)   // 只取半徑²（精確有理數 tuple）
welzl!(@random N, seed)       // 編譯期生成 N 個隨機點再求解

welzl_tower!(NAME, base, + + +);  // 自我生成宏塔：每層生成下一層，規模 base<<level
```

## 自適應遞迴：觸頂自帶遞增後返回演算法

const-eval 沒有 `catch_unwind`，`E0080` 是**編譯失敗**而非可捕捉的錯誤 —— 無法事後接住。
所以改為**事前預測**：遞迴攜帶燃料 `fuel`，在觸頂前一步主動交棒。

```text
welzl(P, R, fuel):
    if P = ∅ or |R| = 3:  return trivial(R)
    if fuel = 0:                             ← 觸頂前一步
        generation += 1                      ← 自帶遞增
        return resume(P, R)                  ← 返回演算法（迭代續解）
    ...
```

`resume(P, R)` 不是近似、不是放棄，而是**同一個問題的迭代解法**：
「以 R 為強制邊界點求 MEC(P)」。遞迴版與迭代版求的是同一個數學物件
`MEC(P | R ⊂ ∂D)`，所以**在任何深度交棒都得到完全相同的答案**。

`welzl_escalate` 從 `DEFAULT_FUEL = 16` 起跳，發生續傳就把燃料加倍，
直到不再需要續傳或抵達 `MAX_SAFE_FUEL = 64`。實測 500 點時 `rounds=3`
（16→32→64）、`gens=1`、`depth=64`。

**效果：120 點的天花板消失。**

| | 純遞迴 `@rec` | 自適應 `@auto` |
|---|---|---|
| 121 點 | `E0080` 編譯失敗 | ✅ |
| 500 點 | `E0080` | ✅ |
| 100,000 點 | `E0080` | ✅（10 s） |
| 500,000 點 | `E0080` | ✅（38 s） |

不變量已寫成編譯期斷言 —— `fuel=1`（幾乎立刻交棒）與 `fuel=48`（深入遞迴）
的 r² 必須逐位元相同，否則編譯失敗：

```rust
const _: () = assert!(cmp_radius(A_F1.circle, A_F3.circle) == 0);
```

## 精確算術

圓以共分母的有理數表示，`(cx/cd, cy/cd)`、`r2/cd²`：

```text
p ∈ D  ⟺  (p.x·cd − cx)² + (p.y·cd − cy)² ≤ r2
```

同一個分母承載圓心與半徑²，使「點在圓內」退化成單次乘法比較，沒有交叉相乘的膨脹。
空圓以 `r2 = -1` 編碼，左式恆 ≥ 0，判定自然回傳 false，無需特判。
最深的中間量約 `C⁶`，故座標安全上界為 **±10⁶**（`@checked` 會在編譯期強制）。

共線三點的外接圓行列式為 0，退化路徑改取三組兩點圓中能覆蓋第三點的最小者。

## 正確性

`cargo test` — **77 項全過**，包含 **300 組隨機點集與 O(n³) 暴力法逐一比對**
（驗證的是「確實最小」，不只是「確實覆蓋」）。`examples/demo.rs` 另有編譯期斷言：

```rust
const _: () = assert!(cmp_radius(V_ITER, V_BRUTE) == 0);  // 迭代 vs 暴力
const _: () = assert!(cmp_radius(V_REC,  V_BRUTE) == 0);  // 遞迴 vs 暴力
```

三種實作若不一致，**編譯直接失敗**。

## 極限（實測）

| 維度 | 極限 | 攔下它的 |
|------|------|----------|
| 純宏遞迴（預設） | 126 點 | `recursion_limit = 128` |
| 純宏遞迴（limit=10⁶） | 14,336 點 | rustc **SIGSEGV**（原生堆疊） |
| const-eval 遞迴 `welzl(P,R)` | 120 點 | `E0080` const-eval 堆疊 |
| **自適應遞迴 `@auto`** | **500,000 點**（38 s） | 記憶體（1M 時 OOM-kill），堆疊限制已解除 |
| const-eval 迭代 Welzl | 59,392 點 | `long_running_const_eval` lint |
| 同上 + `#![allow(...)]` | **4,000,000 點**（111 s） | 記憶體（8M 時 OOM-kill） |
| 自我生成宏塔 | 16 層 | 第 16 層 = 65,536 點 |

**同一個算法，遞迴 120 點 vs 迭代 4,000,000 點 —— 差 33,000 倍。**
差別不在複雜度，而在於問題規模映射到哪種資源：遞迴映射到 const-eval 堆疊（最稀缺），
迭代只映射到 step 數（最充裕）。詳見 [`LIMITS.md`](LIMITS.md)。

## 執行期成本為零

```asm
get_r2:  movl $5125, %eax   ; r² = 5125/169
         retq
```

全檔 `call`/跳轉計數 = 0。洗牌、三層迴圈、i128 行列式、GCD 約分全部消失，只剩常數。

## 實用性

**甜蜜點 N ≤ 256**（淨編譯增量 < 30 ms）；**硬邊界是「資料必須編譯期已知」**，
而非任何技術上限。const-eval 比原生執行期慢約 **4,000×**（500k 點：38 s vs 9.4 ms）——
它的價值是執行期成本歸零、錯誤左移、`no_std` 可用，**不是速度**。

最佳舞台：嵌入式固定幾何、資產管線烘焙、**用幾何性質做編譯期斷言**。
另需注意 const-eval **無增量快取**（N=5000 時改一行無關程式碼仍需 307 ms 全量重算）。

> 附帶一提：本專案最可複用的資產其實是 `src/exact.rs` —— 一個
> **編譯期與執行期通用**的 i128 精確有理數幾何核心，零 epsilon、零誤差累積。
> 對 CAD/PCB/GIS 這類「答案必須可證明正確」的領域，它的價值與編譯期無關。

完整分析見 [`APPLICABILITY.md`](APPLICABILITY.md)。

## 延伸：精確幾何謂詞（Delaunay / Voronoi / 半平面交）

`exact.rs` 的整數核心不限於 MEC。`predicates.rs` 提供計算幾何的三個標準謂詞，
`delaunay.rs` 用它們實作 Delaunay 三角剖分、Voronoi 對偶與半平面交。

**關鍵發現：座標上界由謂詞的多項式次數決定，而 MEC 是最貴的。**

| 謂詞 | 次數 | 座標上界 |
|---|---|---|
| `orient2d` | 2 | ~4×10¹⁸ |
| `incircle`（Delaunay/Voronoi） | 4 | **~9×10⁸** |
| `circ3`（MEC） | 6 | ~10⁶ |

也就是說 **Delaunay/Voronoi 的可用範圍比 MEC 大約 900 倍**。
原因是它們的所有拓撲判定都能化約成**原始輸入座標**的行列式 —— **次數不累積**。

唯一的例外是裁剪式半平面交（次數會累積，每次 +32 bit，i128 只夠 4 次）；
`RatPt::reduce()` 的每步約分把它壓到 **127 bit → 6 bit**。

完整分析見 [`GEOMETRY.md`](GEOMETRY.md)。

## 下游應用實測

把精確核心當底層，實作三個上層算法（`segdist.rs` / `sphere.rs` / `boolops.rs`）：

| 應用 | 契合度 | 精確範圍 | 關鍵限制 |
|---|---|---|---|
| 線段最近距離 | ✅ 理想 | 相交 1e18 / 門檻 1e9 / 比較 1e6 | 交叉相乘次數 6 |
| 球面凸包面積 | ⚠️ 分層 | 組合層 1e12、`tan(Ω/2)` 精確 | **面積是超越數，數學上不可能精確** |
| 多邊形布爾 | ✅ 判定完美 | 面積/判定 1e18 | 鏈式構造需約分 |

**⚠️ 安全提醒**：實測發現 `Rat::cmp` 在座標 ≥1e8 時會**靜默給出錯誤答案**
（release 溢位是 wrapping，且 `-C debug-assertions` **不跨 crate 邊界**）。
生產請用 `Rat::cmp_checked()`。完整分析見 [`DOWNSTREAM.md`](DOWNSTREAM.md)。

## 區間濾波與事前規劃

`src/interval.rs` 提供三層架構：**Tier 0 事前規劃**（O(1)，可編譯期執行）、
**Tier 1 區間/靜態誤差界濾波**、**Tier 2 精確回退**。

**濾波永不說謊**：100 萬組驗證，0 錯誤。成功率隨機資料 100%、
近共線 99.21%、完全共線 9.39%（只取決於退化程度，幾乎與座標量級無關）。

⚠️ **但效能結論與直覺相反**：在 i128 輸入下濾波**慢 4 倍**（12.02 ns vs 53.10 ns）——
因為單次 `i128 as f64` 轉換（6.70 ns）就比整個 i128 精確謂詞（5.83 ns）還貴。
濾波只在**輸入本來就是 f64** 時獲利（**1.95–2.14×**）。

> i128 已經是極快的精確算術實作，快到讓濾波在多數情況失去意義。

意外收穫：區間運算靠 f64 的大指數範圍，**救回了 `DOWNSTREAM.md` 記錄的 i128 靜默溢位案例**
（i128 需 176 bit 而溢位，區間濾波給出正確答案）。兩者互補：**i128 提供精度，f64 提供動態範圍**。

完整數據見 [`INTERVAL.md`](INTERVAL.md)。

## 統一幾何核心

`src/kernel.rs` 把五個子系統整合成單一入口 —— 共享 i128 數域、共享謂詞、共享精度決策管線：

```rust
let mut k = Kernel::new(1_000).with_precision(Precision::Auto);
k.preflight(&ALL_SPECS)?;                    // 跑之前就知道會不會溢位
let (i, j, d) = k.closest_pair(&pts).unwrap();  // Delaunay → 最近點對
let d2 = k.polygon_distance2(&a, &b);           // 布爾判定 + 線段距離
```

三種精度策略（`Exact`/`Filtered`/`Auto`）對相同輸入給出**完全相同**的結果（20,000 組隨機驗證零分歧）。

⚠️ **整合的斷裂點**：跨算法**組合**（A 的輸出當 B 的查詢）無精度損失；
但跨算法**閉環**（A 的輸出當 B 的輸入座標）**第 1 輪就斷** ——
15 個 Voronoi 頂點個別分母只有 6–15 bit，通分後 LCM 達 **121 bit**，溢位 i128。
完整分析與商業化評估見 [`KERNEL.md`](KERNEL.md)。

## 檔案

```
src/exact.rs   i128 精確有理數核心（全 const fn）
src/welzl.rs   Welzl 遞迴版 + 迭代版 + 編譯期洗牌 + 暴力驗證
src/adaptive.rs 自適應遞迴：燃料 + 續傳 + 自動遞增（觸頂不再編譯失敗）
src/predicates.rs orient2d / incircle / 半平面謂詞（精確，const fn）
src/delaunay.rs   Delaunay 三角剖分 + Voronoi 對偶 + 半平面交
src/segdist.rs    線段最近距離（精確有理數距離²）
src/sphere.rs     球面凸包 + 立體角 tan(Ω/2) 精確有理數
src/boolops.rs    多邊形布爾運算 + Shoelace 精確面積
src/interval.rs   區間運算 + 靜態誤差界濾波 + 事前規劃 plan()
src/macros.rs  welzl! / welzl_tower! / welzl_count!
tests/correctness.rs  12 項（含 300 組隨機 vs 暴力比對）
tests/adaptive.rs     7 項（含 200 組 × 9 種 fuel 的交棒狀態全覆蓋比對）
tests/geometry.rs     13 項（Delaunay 嚴格驗證、Cassini 災難性抵消、分母膨脹）
tests/applications.rs 21 項（三個下游應用 + 溢位偵測）
tests/interval.rs     16 項（濾波永不說謊、事前規劃、區間救回溢位）
examples/demo.rs  展示（含編譯期交叉驗證斷言）
tools/probe.py    極限探測器（指數擴張 + 二分搜尋）
```

```bash
cargo test
cargo run --release --example demo
python3 tools/probe.py all
```
