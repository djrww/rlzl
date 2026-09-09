# 統一核心、商業化路線與研發方向

回答三個問題。第一問我沒有用講的 —— 我把統一核心**實際做出來了**（`src/kernel.rs`，8 項新測試，全 suite 77 項通過），
所以下面的結論是實測結果，不是設計構想。

---

## Q1：能整合在同一個演算法／引擎內嗎？

### 短答

**能整合成單一「引擎」，不能整合成單一「演算法」。而且我實測出了整合會在哪裡斷裂。**

### 已實作：`src/kernel.rs`

```rust
let mut k = Kernel::new(1_000).with_precision(Precision::Auto);
k.preflight(&ALL_SPECS)?;              // Tier 0：跑之前就知道會不會溢位
let circle = k.min_enclosing_circle(&pts);
let tri    = k.delaunay(&pts);
let (i,j,d)= k.closest_pair(&pts).unwrap();   // Delaunay → 最近點對，跨算法組合
let d2     = k.polygon_distance2(&a, &b);     // 布爾判定 + 線段距離，跨算法組合
```

實測通過的性質：

| 測試 | 驗證的事 |
|---|---|
| `precision_modes_agree` | Exact / Filtered / Auto 三種策略對 MEC 與 Delaunay 給出**完全相同**結果 |
| `predicate_dispatch_agrees` | 20,000 組隨機資料，濾波與精確路徑 `orient2d`/`incircle` **零分歧** |
| `preflight_catches_unsafe_coords` | C=1e6 對 MEC（次數 6）**事前**回報錯誤；對 orient2d 放行 |
| `closest_pair_via_delaunay` | 60 組隨機點集，Delaunay 邊上的最近點對 == 暴力法答案 |
| `polygon_bounds_combined` | 同一份頂點同時算包覆圓與面積，兩者皆精確 |

### 真正被統一的只有三樣東西

1. **同一個數域** —— i128 有理數（`Rat` / `RatPt` / `Circle`）
2. **同一套謂詞** —— 所有拓撲判定化約成原始座標的行列式
3. **同一條決策管線** —— Tier 0 規劃 → Tier 1 濾波 → Tier 2 精確

這正是 CGAL 的架構：**精確幾何核心 + 演算法層**。統一的層級是「資料表示與謂詞」，不是控制流。

### 為什麼「單一演算法」是錯誤目標

Delaunay 是分治／增量、布爾是掃描線、最近距離是空間索引查詢。
三者的控制流沒有共同結構，強行合併只會得到又慢又難維護的巨獸。
**次數不累積原則**（`DOWNSTREAM.md`）保證了它們共用謂詞就夠了 —— 不需要共用迴圈。

### ⚠️ 整合的斷裂點（實測）

這是本次實驗最有價值的發現。**「把一個算法的輸出餵回另一個算法」在第 1 輪就會斷。**

閉環實驗：12 個點（座標上界 100，7 bit）→ Delaunay → Voronoi 頂點 → 想再做 Delaunay。

```
round 1: 15 頂點 | max|num|=21 bit  max den=15 bit  共同分母 LCM=121 bit
   → 通分後座標溢位 i128 —— 閉環斷裂於 round 1
```

根因很反直覺：

```
15 個 Voronoi 頂點「各自」的分母位元: [6,6,6,7,8,8,10,11,12,12,13,13,13,14,15]
```

**個別分母都 ≤15 bit，小得很。但要把它們放進同一個整數座標系就必須通分，
LCM 是它們的最小公倍數 → 121 bit，一輪就爆。**

這不是實作缺陷，是有理數幾何的本質障礙（文獻：Goodrich–Pollack–Sturmfels 證明過
某些點線排列在整數格上的實現必須有**指數級**位元複雜度）。

### 工業界怎麼繞過：snap rounding（有損）

對照組 B，每輪把有理頂點四捨五入回整數：

```
round 1:  24 頂點 → 24 個相異點, 座標上界 12 bit
round 2:  43 頂點 → 42 個相異點, 15 bit
round 3:  79 頂點 → 79 個相異點, 18 bit
round 4: 153 頂點 → 151 個相異點, 21 bit
round 5: 297 頂點 → 292 個相異點, 24 bit
round 6: 581 頂點 → 563 個相異點, 25 bit
round 7: 1131 頂點 → 1083 個相異點, 27 bit
```

**可以無限迭代，座標位元只線性成長。代價是每輪引入 ≤0.5 的誤差，拓撲可能改變**
（注意 round 2 的 43→42、round 6 的 581→563 —— 那些消失的點就是被 snap 合併掉的）。

### Q1 結論

| 整合層級 | 可行性 |
|---|---|
| 共用謂詞與數域 | ✅ 完全可行，已實作驗證 |
| 共用精度決策管線 | ✅ 完全可行，三模式結果一致 |
| 跨算法**組合**（A 的輸出當 B 的**查詢**） | ✅ 可行，介面無精度損失 |
| 跨算法**閉環**（A 的輸出當 B 的**輸入座標**） | ❌ **1 輪就斷**，除非 snap rounding（有損）或改 bignum |

「判定與構造分開」不只是建議，是**整合可行性的邊界線**。

---

## Q2：轉為賺錢導向，路線與產品形態？

### 先講不中聽的：純謂詞庫沒有商業空間

已查證的競爭態勢：

- `robust` crate（georust，Shewchuk 移植）：**全時 1,201 萬下載**，MIT/Apache，已含 orient3d/insphere，feature-complete
- `spade` 8.57M、`parry2d/3d` 470 萬、`geo` 依賴 `robust`+`i_overlay`+`spade`
- `geometry-predicates`、`geogram_predicates`（含 SOS 符號擾動）

**這一層被免費 MIT 方案佔滿了。** 本專案的 `predicates.rs`/`interval.rs` 技術上正確，
但商業價值為零 —— 沒人會付錢買一個 `robust` 已經免費給的東西。

而且 `INTERVAL.md` 已經誠實記錄：**i128 輸入下區間濾波是負收益（0.23×）**，
唯一獲利場景是 f64 輸入（1.95–2.14×）。這不是可以拿去賣的賣點。

### 唯一被驗證過的「精確幾何」商業模式：CGAL

雙授權。核心 kernel 是 LGPL，但**幾乎所有高階演算法是 GPL** → 閉源產品必須付費。
實際報價：單一功能被報 >$25K、Industrial Research License €6,000/年。

**關鍵洞察：錢不在謂詞，在綁定謂詞的演算法庫。** 客戶付的是「我不用自己實作 Polygon Mesh Processing」。

### 錢在哪裡（已查證市價）

| 方向 | 市價 | 證據 |
|---|---|---|
| **CAM nesting** | SigmaNEST **$8K–35K/seat** 永久 + 18–22%/年維護；首年 $20K–40K | slabwise, selecthub |
| **nesting（獨立開發者）** | PolygonLogix **$6,500 / $9,500** 永久，**單人開發** | polygonlogix.com |
| 幾何核心授權 | Parasolid 授權給 350+ 應用（含 SolidWorks/Onshape） | Siemens |
| 租核心做產品 | Plasticity（**獨立開發者 Nick Kallen**）$149–299 永久 | 用戶稱 $99 版「勉強只夠付 kernel 授權費」 |
| CAD 訂閱 | Fusion 360 $680/年；SolidWorks $4K–8K/年/seat | — |

### 建議產品形態：**true-shape nesting 引擎**

理由是本專案的技術資產與這個市場**精確對齊**：

1. **Minkowski sum / NFP 是 nesting 的核心幾何**，而 NFP 的公開痛點就是數值魯棒性。
   已查證的文獻原話：*「the orbital piece may lose contact with the edges of the stationary
   due to numerical precision errors」*、*「all state-of-the-art nesting algorithms rely on
   either NFPs or a raster... these two approaches have serious limitations regarding
   robustness and precision, respectively」*（arXiv 2509.13329, EJOR 2025）。

2. **本專案已有的東西直接可用**：多邊形布爾（`boolops.rs`）、線段最近距離（`segdist.rs`）、
   精確重疊判定、Tier 0 事前規劃。nesting 的內圈就是「這個位置會不會碰撞」—— 純謂詞問題，
   **次數不累積原則完全適用**（判定用原始座標，不做鏈式構造）。

3. **零售價已被單人開發者驗證在 $6.5K–9.5K**。這是少數不需要 40 年積累就能進入的高單價幾何市場。

4. **切入角度**：現存方案在 exact-fit（完美嵌合）與退化情形上妥協。
   jagua-rs（目前最好的開源 CDE）自陳設計哲學是 *「always err on the side of caution」*
   —— 兩物體極近時**寧可誤報碰撞**。這意味著它會**漏掉完美嵌合的解**。
   一個「精確判定 + 保證找到 exact fit」的引擎有明確差異化。

**產品階梯**：開源 CDE 引擎（打知名度、當 jagua-rs 的精確版對手）→
商用 SDK 授權給 CAM 廠商 → 垂直 SaaS（上傳 DXF → 回傳排版 + G-code）。

### 誠實的風險

- nesting 的難點**主要是組合最佳化（NP-hard），不是幾何**。幾何魯棒性是差異化，不是護城河。
- 學術 SotA 剛開源（sparrow, 2025-09），且「outperforms SotA by an unexpectedly wide margin」——
  這代表這個領域正在被重新洗牌，機會與競爭同時放大。
- CAM 客戶要的是完整工作流（DXF/STEP 匯入、post-processor、報價），幾何只佔 20%。

---

## Q3：有沒有更值得研發、商業價值更高的方向？

**有。而且我認為比現在轉商業更值得。**

### 排序（我的判斷）

**第 1 名：髒資料容忍的精確 3D 布爾 / mesh 修復**

`manifold-rust` 的存在證明這個方向有人做且已商品化：它在標準 mesh 引擎之外**另寫一個
「robust engine」**，用 exact rational + mesh arrangement（Zhou/Grinspun/Zorin/Jacobson 2016）
處理 non-manifold、自交、掃描髒資料，Auto 模式自動選路。已出 NuGet/C# 綁定與 WASM。

**它的公開弱點正是機會**：自陳「重度自交大網格在 robust engine 會慢，~10k 語料中少數超過 120s 預算」。

為什麼這個方向好：
- 需求真實且痛（3D 掃描、醫療、逆向工程、3D 列印前處理的資料全是髒的）
- **精確算術在這裡是必需品而非優化** —— 髒資料就是會踩到退化情形，浮點方案根本做不出正確答案
- 效能是已知瓶頸 → 有明確的改進標的
- 本專案的 Tier 0 事前規劃在這裡**特別有價值**：可以在跑之前預測「這個網格會不會超時」，
  這正是 manifold 的 120s 預算問題

**第 2 名：GPU / SIMD 上的精確謂詞**

`INTERVAL.md` 已經定位出瓶頸不是算術而是**型別轉換**（`i128 as f64` 6.70 ns >
整個 i128 orient2d 5.83 ns）。這個洞察在批次處理下會放大：批次做 f64→擴展精度轉換可以攤銷。
現有的 Shewchuk 移植全是純量的。這是有技術壁壘、且無人佔據的位置。

**第 3 名：可驗證幾何（formal verification）**

把謂詞的正確性用 Coq/Lean 或 Kani 證明出來。航太／自駕／醫材認證會付錢買這個。
本專案已有的「濾波永不說謊」（100 萬組零錯誤）是可證明性質 —— 離形式化只差一步。
市場小但單價極高、競爭極少。

### 我的實際建議

**不要現在轉商業。** 理由：

現在轉商業，你要賣的東西是「一個 2D 精確謂詞庫」，而市場上有 1,200 萬下載的免費 MIT 替代品。
沒有故事可講。

但你手上有兩樣真正稀缺的東西，只是還沒被放到對的問題上：

1. **Tier 0 事前規劃**（`plan()` / `preflight()`）—— 我沒在任何競品裡看到這個。
   `robust` 是「跑了再說，反正保證正確」；本專案是「跑之前就告訴你要多少位元、會不會溢位、
   該走哪條路」。**這在有時間預算的系統（即時 CAD、雲端 API、認證環境）裡是獨門功能。**

2. **對自身極限的誠實量測** —— `LIMITS.md`、`DOWNSTREAM.md`、`INTERVAL.md` 記錄的
   靜默溢位、負收益、閉環斷裂點，是一般開源庫不會做的功課。

**建議路線**：把 Tier 0 規劃這個獨門能力，帶到「髒資料 3D 布爾」這個真正有痛點的問題上
（第 1 名方向），做出「能事前預測會不會超時／會不會需要 bignum」的 mesh 引擎。
先開源打出技術聲譽，商業化留到有人主動來問的時候。

如果你就是要現在變現，那就是 **nesting**（Q2），因為那是唯一「單人開發者已被驗證能賣到
$6.5K–9.5K」且「幾何魯棒性剛好是公開痛點」的交集。

---

## 附：本次新增的檔案

- `src/kernel.rs` —— 統一核心（`Kernel` / `Precision` / `preflight` / 跨算法組合）
- `tests/kernel.rs` —— 8 項測試
- 全 suite：**77 項通過，0 warning**
