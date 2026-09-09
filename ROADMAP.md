# Roadmap

**記錄日期**：2026-09-09
**目前版本**：v0.6.0（77 tests / 2,858 LOC src / 1,304 LOC tests / 0 warning）

本 roadmap 依 **Session → Phase → Task** 三層組織。
Session 1–6 為**已完成**的實際開發歷程（回溯記錄，日期不可信 —— 見 CHANGELOG.md）。
Session 7 起為**規劃**。

---

## 戰略前提（依實測與市場查證）

以下四點決定了 Phase 7 之後的方向，全部有證據支撐：

1. **純 2D 精確謂詞庫沒有商業空間。**
   `robust` crate（georust，Shewchuk 移植）全時 **1,201 萬下載**、MIT/Apache、
   已含 orient3d/insphere，feature-complete。本專案的 `predicates.rs` 技術正確但無法變現。

2. **本專案唯一未見於競品的能力是 Tier 0 事前規劃。**
   `robust` 的模型是「跑了再說，反正保證正確」；`plan()` / `preflight()` 能在
   **讀取任何資料之前**回答「需要幾位元、會不會溢位、該走哪條路」。
   在有時間預算的系統中有實際價值。

3. **精確幾何唯一被驗證的商業模式是 CGAL 式雙授權** ——
   核心 LGPL、高階演算法 GPL，閉源產品必須付費（實測報價：單一功能 >$25K、
   Industrial Research License €6,000/年）。**錢在演算法庫，不在謂詞。**

4. **兩個可變現方向已有單人開發者驗證：**
   - nesting：PolygonLogix（單人）$6,500–9,500 永久授權；SigmaNEST $8K–35K/seat
   - 租核心做產品：Plasticity（單人，租 Parasolid）$149–299

---

# Session 1–6 — 已完成

## Phase 1 — 核心實作 `[v0.1.0] ✅`

| Task | 狀態 | 產出 |
|---|---|---|
| 1.1 i128 精確數值核心 | ✅ | `src/exact.rs`（186 行，全 `const fn`） |
| 1.2 Welzl 遞迴演算法 | ✅ | `src/welzl.rs`（185 行） |
| 1.3 宏分派層 | ✅ | `src/macros.rs`（187 行） |
| 1.4 正確性測試 vs O(n³) 暴力法 | ✅ | `tests/correctness.rs`（12 項 / 300 組隨機） |
| 1.5 浮點反例構造 | ✅ | Cassini 恆等式，16 組誤判 6 組 |

## Phase 2 — 自適應與極限探測 `[v0.2.0] ✅`

| Task | 狀態 | 產出 |
|---|---|---|
| 2.1 燃料預算 + 續傳 | ✅ | `src/adaptive.rs`（236 行） |
| 2.2 極限探測工具 | ✅ | `tools/probe.py` + 3 個 JSON |
| 2.3 五維度極限量測 | ✅ | `LIMITS.md`；自適應達 500,000 點（4,166×） |

## Phase 3 — 幾何謂詞與 Delaunay/Voronoi `[v0.3.0] ✅`

| Task | 狀態 | 產出 |
|---|---|---|
| 3.1 orient2d / incircle 謂詞 | ✅ | `src/predicates.rs`（237 行） |
| 3.2 Delaunay 暴力預言 O(n⁴) | ✅ | `delaunay_brute` |
| 3.3 Voronoi 對偶 | ✅ | `voronoi_vertices()` |
| 3.4 半平面交與凸多邊形裁剪 | ✅ | `ConvexPoly::clip`（每步 `reduce`） |
| 3.5 ~~Bowyer–Watson 增量插入~~ | ❌ **已刪除** | ADR-0007；5×5 網格產出 1047 三角形 |

## Phase 4 — 下游應用與適用性邊界 `[v0.4.0] ✅`

| Task | 狀態 | 產出 |
|---|---|---|
| 4.1 線段最近距離 | ✅ | `src/segdist.rs`（238 行） |
| 4.2 球面凸包與立體角 | ✅ | `src/sphere.rs`（255 行） |
| 4.3 多邊形布爾 | ✅ | `src/boolops.rs`（259 行） |
| 4.4 靜默溢位調查 | ✅ | `DOWNSTREAM.md`；`Rat::cmp_checked()` |
| 4.5 適用性邊界文件 | ✅ | `APPLICABILITY.md`；球面面積為超越數 |

## Phase 5 — 區間濾波與事前規劃 `[v0.5.0] ✅`

| Task | 狀態 | 產出 |
|---|---|---|
| 5.1 Tier 0 事前規劃 | ✅ | `plan()` / `required_bits()` / `max_safe_coord()` |
| 5.2 Tier 1 區間算術 | ✅ | `Interval` / `next_up` / `next_down` |
| 5.3 Tier 1b 靜態誤差界 | ✅ | `CCW_ERR_A` / `ICC_ERR_A` |
| 5.4 Tier 1c f64 快速路徑 | ✅ | `orient2d_f64`（1.95–2.14×） |
| 5.5 成功率量測（110 萬組） | ✅ | 錯誤 0 次；共線 9.39% |
| 5.6 效能假設檢驗 | ✅ | 瓶頸為 `i128 as f64`（6.70 ns）；ADR-0008 |

## Phase 6 — 統一核心與商業化評估 `[v0.6.0] ✅ 2026-09-09`

| Task | 狀態 | 產出 |
|---|---|---|
| 6.1 統一核心實作 | ✅ | `src/kernel.rs`（262 行） |
| 6.2 三精度策略一致性驗證 | ✅ | 20,000 組零分歧 |
| 6.3 跨算法組合 | ✅ | `closest_pair` / `polygon_bounds` / `polygon_distance2` |
| 6.4 閉環斷裂點量測 | ✅ | LCM 121 bit，第 1 輪即斷 |
| 6.5 商業化路線評估 | ✅ | `KERNEL.md` |
| 6.6 工程文件補齊 | ✅ | 8 份 ADR / DATAFLOW / CHANGELOG / ROADMAP |
| 6.7 納入 git 版本控制 | ✅ | github.com/djrww/rlzl |

---

# Session 7 — 技術債清償（商業化前置）

> **目標**：把「研究原型」變成「可被第三方依賴的函式庫」。
> 這一段不產生新賣點，但不做完就無法談商業化。

## Phase 7 — 生產就緒

| Task | 優先 | 說明 | 驗收標準 |
|---|---|---|---|
| 7.1 Delaunay O(n log n) 重寫 | **P0** | 從頭實作（ADR-0007 禁止復用舊碼），以 `delaunay_brute` 為預言 | 10,000 組隨機 + 全退化組態（網格/共圓/共線）100% 符合預言 |
| 7.2 API 安全化 | **P0** | unchecked 版本改名 `_unchecked`；`Rat::cmp` 預設改為 checked | 無法在不明示的情況下觸發靜默溢位 |
| 7.3 錯誤型別統一 | P1 | 以 `GeomError` 取代 `Option`/`bool` 回傳 | 所有失效模式可區分 |
| 7.4 `no_std` 驗證 | P1 | 核心路徑已無堆積配置，需實際驗證 | `cargo build --no-default-features` 通過 |
| 7.5 模糊測試 | P1 | `cargo-fuzz` 對全部謂詞 | 24 小時無 panic、無與預言分歧 |
| 7.6 文件與範例 | P2 | rustdoc 覆蓋率、`docs.rs` 可讀 | 所有 pub 項目有文件與範例 |
| 7.7 CI | P2 | GitHub Actions：test / clippy / fmt / MSRV | main 分支保護 |

**退出條件**：可以在不加註「這是研究原型」的前提下發佈到 crates.io。

---

# Session 8 — 方向抉擇（互斥，擇一）

> 三條路線互斥，資源不足以並行。決策點在 Session 7 結束時。

## Phase 8A — 髒資料 3D 布爾 / mesh 修復 `【建議・仍為首選】`

**依據**：`manifold-rust` 已證明此方向可商品化（exact rational + mesh arrangement，
Zhou/Grinspun/Zorin/Jacobson 2016；已出 NuGet/C# 綁定與 WASM）。
其**公開弱點**是「重度自交大網格在 robust engine 會慢，~10k 語料中少數超過 120 s 預算」。

**為何契合本專案**：那個 120 s 預算問題，正是 Tier 0 事前規劃要解決的東西 ——
在跑之前預測「這個網格會不會超時 / 需不需要 bignum」。

| Task | 說明 |
|---|---|
| 8A.1 | 3D 精確謂詞擴充（`orient3d` 已有，補 `insphere`） |
| 8A.2 | Mesh arrangement 的精確實作 |
| 8A.3 | **Tier 0 延伸至 mesh 層** —— 依網格統計預測執行時間與位元需求 |
| 8A.4 | 髒資料語料庫（自交 / non-manifold / 掃描雜訊） |
| 8A.5 | 對標 `manifold-rust` 的效能與正確性 benchmark |

**風險**：需求真實但技術難度最高；精確算術在此是**必需品而非優化**
（髒資料必然踩到退化組態，浮點方案根本算不出正確答案）—— 這既是機會也是門檻。

## Phase 8B — ~~True-shape nesting 引擎~~ → **貢獻 jagua-rs 生態** `【已修訂 2026-09-09】`

> **修訂原因**：原計畫「從零自建 nesting 引擎對打 sparrow」已**撤回**。
> 查證原始碼後發現兩件事實（詳見 [`docs/ECOSYSTEM.md`](docs/ECOSYSTEM.md)）：
>
> 1. jagua-rs 生產謂詞用 **f32**，近共線誤判率實測 **9.96%**（49,820/500,000）。
> 2. 但這個缺口**不需要 i128** —— 改用 f64 計算 f32 輸入，誤判降為 **0**，
>    代價僅 **1.21×**。本專案的精確算術在此工況並非必需品。
>
> 從零對打一個 581 commits / 190 stars / 學術 SotA / 活躍維護的專案沒有勝算，
> 且 nesting 難點主要在 NP-hard 組合最佳化而非幾何。

| Task | 說明 | 狀態 |
|---|---|---|
| 8B.1 | 提交近共線 f32 誤判再現測試集（含 Cassini 構造） | 待辦 |
| 8B.2 | 提交關鍵謂詞 f64 化 PR（誤判 0，成本 1.21×） | 待辦 |
| 8B.3 | 文件化 f64 失效邊界：指數跨度 **2^53**（實測 100% 誤判） | 待辦 |
| 8B.4 | exact-fit 精確後端（sparrow 論文自陳做不到） | 評估中 |
| ~~8B.5~~ | ~~自建組合最佳化層 / DXF 匯入~~ | **撤回** |

**授權相容性**：jagua-rs 為 MPL-2.0（檔案級 copyleft，商業可用），接受外部 PR。

## Phase 8C — GPU / SIMD 批次精確謂詞

**依據**：ADR-0005 已定位瓶頸為型別轉換而非算術。批次處理可攤銷轉換成本。
現有 Shewchuk 移植全是純量實作。

| Task | 說明 |
|---|---|
| 8C.1 | SIMD 批次 orient2d / incircle |
| 8C.2 | 批次 f64 → 擴展精度轉換（攤銷 6.70 ns 成本） |
| 8C.3 | GPU kernel（wgpu / CUDA） |

**風險**：技術壁壘高、無人佔據，但**市場需求未經驗證** —— 沒有證據顯示有人願意為此付費。

---

# Session 9 — 商業化（僅在 Session 8 產出可驗證優勢後）

## Phase 9 — 產品化

| Task | 說明 |
|---|---|
| 9.1 | 授權模式決定（CGAL 式雙授權 / 純商業 SDK / 開源 + 服務） |
| 9.2 | 開源核心打技術聲譽 |
| 9.3 | 商用 SDK 授權 |
| 9.4 | 垂直 SaaS（依 8A/8B 而定） |

**建議節奏**：先開源打出技術聲譽，商業化留到有人主動詢問時再談。
現在就轉商業，要賣的是「一個 2D 精確謂詞庫」，而市場上有 1,200 萬下載的免費 MIT 替代品 ——
沒有故事可講。

---

## 明確不做的事

| 項目 | 原因 |
|---|---|
| 重新引入 Bowyer–Watson 舊實作 | ADR-0007；錯誤的快速演算法比正確的慢演算法更糟 |
| 預設啟用區間濾波 | ADR-0005；i128 輸入下為 0.23× 負收益 |
| 支援跨算法閉環回饋 | ADR-0006；LCM 位元爆炸是本質障礙，第 1 輪即斷 |
| 採用 snap rounding 作為預設 | 有損、拓撲會變，違背「精確」承諾 |
| 改用 proc-macro | ADR-0002；會引入依賴且觸碰的是行程限制而非編譯器限制 |
| 追求球面面積的精確值 | 超越數，bignum 亦無解 |
