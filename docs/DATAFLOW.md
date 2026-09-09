# Data Flow Diagram

**版本**：對應 v0.6.0（77 tests, 2,858 LOC src）
**記錄日期**：2026-09-09

---

## 圖例

```
[ 外部實體 ]      ( 處理程序 )      || 資料儲存 ||      --> 資料流
```

---

## Level 0 — Context Diagram

```
                          i128 整數點集
   [ 呼叫端 ]  ─────────────────────────────►  (  welzl_macro  )
   (使用者程式碼 /                                     │
    編譯期宏調用)  ◄─────────────────────────────────┘
                    Circle{cx,cy,cd,r2} / Triangulation /
                    Rat / Polygon / Plan / Err(超出範圍)
```

**信任邊界**：`welzl_macro` 不做 I/O、不配置堆積（核心路徑）、無外部依賴。
唯一的外部輸入是座標值本身，唯一的失效模式是**座標超出安全範圍**。

---

## Level 1 — 主資料流

```
 [ 呼叫端 ]
     │
     │ ① coord_bound: i128（僅上界，不含實際資料）
     ▼
 ( 1.0 Tier 0 事前規劃 )  ◄──── || SPEC_ORIENT2D / ORIENT3D / INCIRCLE / CIRC3 ||
     │                                    (謂詞次數表，編譯期常數)
     │ required_bits = d·log2(C) + log2(terms) + 1
     │
     ├──► Plan::RequiresBignum ─────► [ 呼叫端 ]  ✗ 提前失敗，未讀取任何資料
     ├──► Plan::ExactMayOverflow ──► [ 呼叫端 ]  ✗ 邊際不足
     │
     └──► Plan::FloatSufficient / FilterThenExact / Exact
                    │
                    │ ② 精度策略確立
                    ▼
              ( 2.0 謂詞分派 )  ◄──── ③ pts: &[(i128,i128)]
                    │
        ┌───────────┼───────────────┐
        ▼           ▼               ▼
   (2.1 f64     (2.2 靜態界      (2.3 i128
    快速路徑)     濾波)            精確)
        │           │               │
        │      成功 │  不確定        │
        │           └──────────────►│
        └───────────────────────────┤
                                    ▼
                          ④ 符號 ∈ {-1, 0, +1}
                                    │
     ┌──────────────┬───────────────┼──────────────┬──────────────┐
     ▼              ▼               ▼              ▼              ▼
 (3.1 Welzl     (3.2 Delaunay   (3.3 線段      (3.4 多邊形    (3.5 球面
  MEC)           /Voronoi)       距離)          布爾)          凸包)
     │              │               │              │              │
     ▼              ▼               ▼              ▼              ▼
  Circle      Triangulation       Rat          Polygon      f64 面積
 {cx,cy,cd,r2}  → voronoi_          {num,den}   {verts}      (超越數,
                  vertices()                                  近似值)
     │              │               │              │              │
     └──────────────┴───────────────┴──────────────┴──────────────┘
                                    │
                                    ▼
                              [ 呼叫端 ]
```

### 關鍵性質：④ 是**唯一**的跨模組介面

所有上層演算法（3.1–3.5）只消費「符號」，不消費彼此的建構結果。
這是**次數不累積原則**的具體體現，也是五個子系統能共存於單一核心的原因。

---

## Level 2 — 精度決策管線（處理程序 1.0 / 2.0 展開）

```
   coord_bound C ──► ( required_bits ) ──► d·log2(C)+log2(terms)+1
                            │
                            ▼
              ┌─────────────────────────────┐
              │  bits ≤ 53   → FloatSufficient
              │  bits ≤ 127  → FilterThenExact
              │  bits ≤ 127* → ExactMayOverflow (邊際不足)
              │  bits > 127  → RequiresBignum
              └─────────────────────────────┘
```

### 謂詞次數表（|| 資料儲存 ||，編譯期常數）

| 謂詞 | 次數 d | i128 安全上界 | f64 (53 bit) 上界 |
|---|---|---|---|
| `hp_contains` | 1 | ~10³⁸ | — |
| `orient2d` | 2 | 4×10¹⁸ | 8.4×10⁶ |
| `orient3d` | 3 | 5.5×10¹¹ | 3.3×10⁴ |
| `incircle` | 4 | 2.7×10⁸ | 1.0×10³ |
| `circ3` (MEC) / `Rat::cmp` | 6 | 5.2×10⁵ | 1.3×10² |

### `plan()` 輸出矩陣（實測）

| 謂詞 \ C | 1e3 | 1e6 | 1e9 | 1e12 | 1e18 |
|---|---|---|---|---|---|
| orient2d | Float | Float | Filter | Filter | Overflow |
| orient3d | Float | Filter | Filter | Overflow | Bignum |
| incircle | Float | Filter | Bignum | Bignum | Bignum |
| circ3 | Filter | Bignum | Bignum | Bignum | Bignum |

---

## Level 2 — 編譯期資料流（宏路徑）

```
 [ 原始碼 ]
     │ welzl!(...) / welzl_tower!(N)
     ▼
 ( M1 macro_rules! 展開 )
     │   ├─ tt-munching 解析點集字面量
     │   └─ selfgen：遞迴生成更深層宏調用
     │
     │ 極限：126 點（recursion_limit）/ 14,336 點（SIGSEGV）
     │ 宏塔：16 層 = 65,536 點
     ▼
 ( M2 const-eval 求值 )
     │   遞迴路徑 → 120 點（E0080 max stack frames）
     │   迭代路徑 → 59,392 點（lint）/ 4,000,000 點（OOM 1.9GB）
     │   @auto    → 500,000 點（38 s）
     ▼
 [ 編譯產物：const Circle ]
```

**兩套限制彼此獨立** —— `recursion_limit` 與 const-eval 的 Miri stack frame
上限是不同機制，調整前者不影響後者。

**無增量快取**：每次編譯重新求值。編譯期甜蜜點為 256 點 / 26 ms。

---

## 斷裂點：閉環回饋（**不支援的資料流**）

```
   pts (i128)
      │
      ▼
  ( Delaunay ) ──► ( voronoi_vertices ) ──► Circle{cx,cy,cd}
                                                 │
                                                 │ 有理數頂點
                                                 ▼
                                          ( 通分成共同分母 )
                                                 │
                                    LCM = 121 bit ✗ 溢位 i128
                                                 │
                                                 ▼
                                        ✗✗ 閉環斷裂於第 1 輪 ✗✗
```

實測：12 點 / 座標上界 100（7 bit）→ 15 個 Voronoi 頂點。
各頂點分母**個別**為 `[6,6,6,7,8,8,10,11,12,12,13,13,13,14,15]` bit（都很小），
但共同分母 LCM 達 **121 bit**，通分後座標溢位。

**有損繞道（未採用）**：snap rounding 每輪四捨五入回整數，
位元僅線性成長（12→15→18→21→24→25→27 bit），但每輪引入 ≤0.5 誤差、
拓撲可能改變（實測 43→42、581→563 個相異點，差額即被合併的點）。

詳見 ADR-0006。

---

## 儲存與副作用清單

| 項目 | 說明 |
|---|---|
| 堆積配置 | 僅 `Vec` 於 Delaunay / Polygon / 非 const 路徑；核心謂詞零配置 |
| 全域可變狀態 | 無 |
| I/O | 無（`tools/probe.py` 除外，屬開發工具） |
| 不安全程式碼 | 無 `unsafe` |
| 外部依賴 | 無（`Cargo.toml` 無 `[dependencies]`） |
| 可觀測性 | `FilterStats`（濾波成功率計數）、`KernelReport` |

---

## 已知失效模式

| 失效 | 偵測方式 | 緩解 |
|---|---|---|
| 座標超出謂詞安全範圍 | `Kernel::preflight()` 事前攔截 | 縮小座標 / 改 bignum |
| `Rat::cmp` 靜默溢位 | **無法在執行期偵測** | 使用 `cmp_checked()` |
| Delaunay O(n⁴) 效能 | — | 已知技術債（ADR-0007） |
| 球面面積為超越數 | — | 本質限制，bignum 亦無解 |
| 閉環回饋位元爆炸 | 通分時 `checked_mul` 回 `None` | 不支援；改用 snap rounding（有損） |
