# Changelog

本檔案格式依據 [Keep a Changelog](https://keepachangelog.com/zh-TW/1.1.0/)，
版本編號依據 [Semantic Versioning](https://semver.org/lang/zh-TW/)。

---

## ⚠️ 日期可信度聲明

本專案在納入 git 版本控制**之前**已完成六個開發階段。
工作區快照機制會將所有檔案的 mtime 重設為還原時間
（實測全部檔案皆為 `2026-09-09 07:33`），**因此各階段的真實日期無法從檔案系統復原**。

- `2026-09-09` 是**首次 git 提交日**，也是本 changelog 的撰寫日，此日期可信。
- 該日期之前的版本號（v0.1.0–v0.5.0）為**回溯重建**，用於記錄開發順序，
  其日期標記為 `未記錄`。順序本身可信（有 ADR 與文件的依賴關係佐證），
  絕對日期不可信。
- **自 v0.6.0 起，所有日期由 git commit timestamp 提供，可信。**

---

## [Unreleased]

### Added
- `docs/ECOSYSTEM.md` —— 與 jagua-rs / sparrow 的生態定位分析。

### Changed
- `ROADMAP.md` Phase 8B **撤回**「自建 nesting 引擎」，改為貢獻 jagua-rs 生態。

### Corrected
- 上一版 `KERNEL.md` / `ROADMAP.md` 對 jagua-rs 的描述精度不足。
  查證原始碼後更正：其生產謂詞為 **f32**（非泛稱的「浮點」），
  且專案內已自行以 f64 重算斷言以避免「繼承 f32 的保守捨入」
  （`collision_detection/quadtree/assertions.rs:19` 原始註解）。

### 實測發現（f32 / f64 / 精確有理數三方比對）
- **近共線 f32 謂詞誤判 9.96%**（49,820 / 500,000）；Cassini 構造下 **50%**（18/36）。
- **f64 計算 f32 輸入：全部攻擊組誤判 0**（近共線 500k、跨尺度 1.14M、
  大值微擾 500k、近上溢 100k、指數跨度 2^0–2^28 各 200k）。
- **f64 的真實失效邊界為指數跨度 2^53**：2^20→0%、2^40→0.025%、
  **2^53→100%**。機制為 `b − a == −a`（小項被完全吸收），行列式退化為 0。
- 成本：f32 1.97 ns → f64 2.38 ns = **1.21×**（含 `black_box`）。
- **推論**：nesting 工況座標量級同質，跨度遠小於 2^53，
  故 **f64 已足夠，i128 精確算術在此非必需**。

### Note
- 使用者以外部工具產出 Lean 4 形式化（10 定理、無 `sorry`）與
  `src/formal.rs` / `tests/formal_lemmas.rs`（19 項，總計 96 項）。
  **該批檔案目前不在本工作區**，本倉庫 HEAD 仍為 77 項。合併待辦。

## [0.6.0] — 2026-09-09

首次納入 git 版本控制。新增統一核心與商業化評估。

### Added
- `src/kernel.rs`（262 行）—— 統一幾何核心。
  - `Kernel` / `Precision`（`Exact` / `Filtered` / `Auto`）/ `KernelReport`
  - `preflight()` —— 事前攔截超出安全範圍的座標組合
  - `plan_for()` —— 單一謂詞的 Tier 0 規劃
  - 跨算法組合：`closest_pair()`（Delaunay → 最近點對）、
    `polygon_bounds()`（包覆圓 + 面積）、`polygon_distance2()`（布爾判定 + 線段距離）
- `tests/kernel.rs` —— 8 項測試（總數 69 → **77**）
- `KERNEL.md` —— 統一核心分析、商業化路線、研發方向評估
- `docs/adr/0001` … `0008` —— 八份架構決策記錄（Michael Nygard 格式）
- `docs/DATAFLOW.md` —— Level 0 / 1 / 2 資料流圖
- `CHANGELOG.md`、`ROADMAP.md`（本次新增）

### Changed
- `README.md` —— 新增「統一幾何核心」章節；測試數 69 → 77

### 驗證
- `cargo test --release`：**77 passed**（0+7+21+12+13+16+8），0 failed
- `cargo build --release`：**0 warning**
- 專案體積（不含 `target/`）：293 KB

### 實測發現
- **跨算法閉環第 1 輪即斷裂**。12 點 / 座標上界 100 → 15 個 Voronoi 頂點，
  各自分母僅 6–15 bit，但共同分母 LCM 達 **121 bit**，通分後溢位 i128。
  屬有理數幾何的本質障礙，非實作缺陷。
- snap rounding 對照組可無限迭代（位元線性成長 12→27 bit），
  但每輪引入 ≤0.5 誤差且拓撲改變（581→563 相異點）。**未採用。**
- 三種精度策略對相同輸入結果**完全一致**（20,000 組隨機驗證零分歧）。

---

## [0.5.0] — 未記錄

區間濾波與事前規劃。

### Added
- `src/interval.rs`（562 行）—— 四層結構：
  - **Tier 0** 事前規劃：`Plan` / `PredicateSpec` / `SPEC_*` / `plan()` /
    `required_bits()` / `max_safe_coord()`
  - **Tier 1** 區間濾波：`next_up` / `next_down` / `Interval` / `*_interval`
  - **Tier 1b** 靜態誤差界：`CCW_ERR_A` / `ICC_ERR_A` / `orient2d_filter` / `incircle_filter`
  - **Tier 1c** f64 路徑：`PtF` / `orient2d_f64`
  - **Tier 2** 回退：`*_adaptive` / `*_stat` / `FilterStats` / `ratcmp_will_overflow`
- `tests/interval.rs` —— 16 項（總數 53 → 69）
- `INTERVAL.md`

### 實測發現（與文獻相反）
- **濾波永不說謊**：100 萬組 orient2d + 10 萬組 incircle，錯誤 **0 次**。
- 成功率只取決於退化程度，幾乎與座標量級無關：
  均勻隨機 100.00% / 近共線 99.21% / 網格 99.22% / 完全共線 9.39%。
- ⚠️ **i128 輸入下濾波是負收益**：orient2d 12.02 ns → 53.10 ns（**0.23×**）。
- 根因（假設檢驗）：`i128 as f64` **6.70 ns** > 整個 i128 orient2d **5.83 ns**。
  **貴的是型別轉換，不是整數算術。** 文獻宣稱的 10× 是相對 GMP/MPFR，非 i128。
- 濾波唯一獲利場景為 **f64 輸入**：1.95×–2.14×（最壞 0.61×）。
- 意外收穫：區間算術靠 f64 的 ±308 位指數範圍，
  **救回 i128 靜默溢位案例**（C=1e9 需 176 bit）。
  **i128 給精度，f64 給動態範圍。**

### Fixed
- 首次 benchmark 未用 `black_box`，全部回報 0.00 ns，數據作廢後重測（見 ADR-0008）。
- `tests/interval.rs` 兩項初版測試失敗，診斷後確認是**測試假設寫錯** ——
  區間濾波正確解出了 `DOWNSTREAM.md` 記載的溢位案例。已改寫測試。

---

## [0.4.0] — 未記錄

下游應用與適用性邊界。

### Added
- `src/segdist.rs` —— `Rat{num,den}`（`new` 自動約分 / `cmp` / **`cmp_checked`** /
  `min` / `to_f64` / `int` / `is_zero`）、`dist2_pp` / `dist2_ps` /
  `segments_intersect` / `dist2_ss` / `within_distance2`
- `src/sphere.rs` —— `Pt3` / `det3` / `orient3d` / `on_sphere` / `convex_hull_3d` /
  `solid_angle` / `spherical_hull_area` / `integer_sphere_points`
- `src/boolops.rs` —— `Polygon` 含 `area2()` / `is_ccw` / `contains`（三態）/
  `overlaps`；`clip_convex`；`convex_intersection_area2`
- `tests/applications.rs` —— 21 項
- `DOWNSTREAM.md`、`APPLICABILITY.md`

### Fixed
- `solid_angle` 改用 `atan2(|num|, den)`，修正象限錯誤。
- 新增 `Rat::cmp_checked()`（見 ADR-0004）。

### 實測發現
- ⚠️ **`Rat::cmp` 在 C ≥ 1e8 靜默給出錯誤答案且不 panic**。
  根因：release 溢位為 wrapping，且 **`-C debug-assertions` 不跨 crate 邊界**。
  可信驗證法是用 Python 任意精度反查位元需求
  （C=1e6→116 bit、1e8→156、1e9→176、1e12→231）。
- **分母膨脹**：`clip` 未約分時 17→49→81→127 bit 飽和（僅夠 4 次）；
  加 `reduce()` 後 8 次僅 5 bit。CSG 8 次：未約分 21 bit / 約分 9 bit。
  約分是啟發式，非保證。
- **球面面積是超越數，bignum 亦無解** —— 只有組合層（1e12）與
  `tan(Ω/2)` 可精確。

---

## [0.3.0] — 未記錄

幾何謂詞與 Delaunay / Voronoi。

### Added
- `src/predicates.rs` —— `orient2d`（次數 2）/ `incircle`（次數 4）/
  `HalfPlane` / `RatPt` 含 `reduce()` / `hp_intersect` / `hp_contains`
- `src/delaunay.rs` —— `Tri` / `Triangulation` 含 `is_delaunay()` /
  `voronoi_vertices()`；`circumcircle`；`delaunay_brute(pts, strict)`（O(n⁴) 預言）；
  `ConvexPoly::clip`（每步 `reduce`）；`halfplane_intersection`；`voronoi_cell`
- `tests/geometry.rs` —— 13 項
- `GEOMETRY.md`

### Removed
- **Bowyer–Watson 增量插入實作已刪除**（見 ADR-0007）。
  空腔邊界判定錯誤：5×5 網格產出 1047 個三角形（應為 32），隨機僅 3/60 通過。
  **不得重新引入**；若需生產級演算法應從頭實作並以 `delaunay_brute` 當預言。

### 確立原則
- **次數不累積**：拓撲判定化約成原始座標的行列式，有理數頂點只在最終輸出。
  這是後續 v0.6.0 統一核心可行的理論基礎。

---

## [0.2.0] — 未記錄

自適應執行與極限探測。

### Added
- `src/adaptive.rs` —— `MAX_SAFE_FUEL=64` / `DEFAULT_FUEL=16` /
  `welzl_adaptive` / `welzl_escalate` / `welzl_auto` / `welzl_fueled`
- `tests/adaptive.rs` —— 7 項
- `tools/probe.py` + `probe_macro.json` / `probe_const.json` / `probe_tower.json`
- `LIMITS.md`

### 實測極限（rustc 1.98.1 / x86-64 / 2 vCPU / 1.9 GB RAM）

| 維度 | 極限 | 攔下它的 |
|---|---|---|
| 純宏 tt-munching | 126 / 14,336 點 | recursion limit / SIGSEGV |
| const-eval 遞迴 | 120 點 | `E0080 maximum stack frames` |
| **自適應 `@auto`** | **500,000 點**（38 s） | 記憶體 |
| const-eval 迭代 | 59,392 / 4,000,000 點 | lint / OOM |
| 自我生成宏塔 | 16 層（65,536 點） | — |

自適應相對 const-eval 遞迴為 **4,166 倍**。
500k 點執行期 `welzl_slice` 9.357 ms vs const-eval 38,000 ms ≈ **4,000×**。

### 實測發現
- **天花板是記憶體，不是堆疊。**
- const-eval **無法**在觸頂後捕捉錯誤（無 `catch_unwind`，`E0080` 直接編譯失敗），
  只能事前預算（見 ADR-0003）。
- `recursion_limit` 與 const-eval Miri stack frame 上限是**兩套獨立機制**。
- 直覺「燃料越少 → 續傳越多」**已被推翻**（fuel=1→gens=1，fuel=48→gens=2）。
- **無增量快取**；編譯期甜蜜點 256 點 / 26 ms。

### Fixed
- `probe.py` 初版未加 `--crate-type lib`，導致所有維度回報 `max=1`。
- `/usr/bin/time` 不存在（rc=127），改以 rc=137 判定 OOM。

---

## [0.1.0] — 未記錄

核心實作。

### Added
- `src/exact.rs` —— i128 精確核心（全 `const fn`）：`Circle{cx,cy,cd,r2}` /
  `in_circle` / `circ1`/`circ2`/`circ3` / `normalize` / `cmp_radius` / `COORD_BOUND=1e6`
- `src/welzl.rs` —— `welzl_rec` / `welzl_slice` / `welzl_arr` / `shuffle` /
  `gen_pts` / `brute` / `covers` / `trivial`
- `src/macros.rs` —— `welzl!` 分派；`welzl_count!` / `welzl_tower!` /
  `welzl_depth!` / `welzl_grid!`
- `tests/correctness.rs` —— 12 項（含 300 組隨機點集與 O(n³) 暴力法逐一比對）
- `examples/demo.rs`、`README.md`

### 確立決策
- ADR-0001：i128 有理數為唯一數域，半徑保留平方形式。
- ADR-0002：`macro_rules!` 負責展開與分派，`const fn` 負責數值；**拒絕 proc-macro**。

### 實測發現
- **對稱測試案例與 600,000 組隨機取樣都無法暴露浮點誤差。**
  必須用 Cassini 恆等式（`F(n+1)F(n-1) − F(n)² = ±1`）這類刻意構造的災難性抵消 ——
  該構造下浮點 16 組誤判 6 組。

---

[Unreleased]: https://github.com/djrww/rlzl/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/djrww/rlzl/releases/tag/v0.6.0
