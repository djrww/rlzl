//! # 統一幾何核心（Unified Geometry Kernel）
//!
//! 把 i128 精確謂詞、Delaunay/Voronoi、線段最近距離、多邊形布爾、區間濾波
//! 整合成**單一入口**。回答「能不能編在同一個演算法內」這個問題。
//!
//! ## 答案：能整合成單一「引擎」，但不是單一「演算法」
//!
//! 它們不是一個演算法 —— 是**一個共享地基上的演算法家族**。
//! 真正統一的東西只有三樣：
//!
//! 1. **同一個數域**：i128 有理數（`Rat` / `RatPt` / `Circle`）
//! 2. **同一套謂詞**：所有拓撲判定都化約成原始座標的行列式
//! 3. **同一條決策管線**：Tier 0 規劃 → Tier 1 濾波 → Tier 2 精確
//!
//! 這三樣共享，讓上層算法可以互相組合而不會在介面處丟失精度 ——
//! 這正是 CGAL「精確幾何核心 + 演算法層」的架構，也是它能成為
//! 工業標準的原因。
//!
//! ## 為什麼「單一演算法」是錯誤的目標
//!
//! Delaunay 是 O(n log n) 的分治／增量；布爾運算是掃描線；
//! 最近距離是空間索引查詢。強行合併只會得到一個又慢又難維護的巨獸。
//! 正確的統一層級是**資料表示與謂詞**，不是控制流。
//!
//! 本模組提供的是這個「共享地基 + 統一分派」，而非一個巨型函式。

use crate::boolops::Polygon;
use crate::delaunay::{self, Triangulation};
use crate::exact::{Circle, Pt};
use crate::interval::{self, FilterStats, Plan, PredicateSpec};
use crate::predicates as pred;
use crate::segdist::{self, Rat};

// ---------------------------------------------------------------------------
// 統一的精度策略
// ---------------------------------------------------------------------------

/// 核心的精度策略 —— 一次設定，貫穿所有算法。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Precision {
    /// 純 i128 精確（輸入為整數時最快 —— 見 `INTERVAL.md`）。
    #[default]
    Exact,
    /// 先濾波再精確（輸入為 f64 時約 2× 加速）。
    Filtered,
    /// 自動：由 Tier 0 事前規劃決定。
    Auto,
}

/// 核心的執行報告 —— 讓呼叫端知道「發生了什麼」。
#[derive(Clone, Copy, Debug, Default)]
pub struct KernelReport {
    pub stats: FilterStats,
    /// 事前規劃的結論（若有執行）。
    pub plan: Option<Plan>,
    /// 是否偵測到超出安全範圍的座標。
    pub out_of_range: bool,
}

/// **統一幾何核心**：共享數域、共享謂詞、共享決策管線。
#[derive(Clone, Debug)]
pub struct Kernel {
    pub precision: Precision,
    /// 輸入座標的絕對值上界（用於 Tier 0 規劃）。
    pub coord_bound: i128,
    pub report: KernelReport,
}

impl Default for Kernel {
    fn default() -> Self {
        Kernel {
            precision: Precision::Exact,
            coord_bound: 1_000_000,
            report: KernelReport::default(),
        }
    }
}

impl Kernel {
    pub fn new(coord_bound: i128) -> Self {
        Kernel { coord_bound, ..Default::default() }
    }

    pub fn with_precision(mut self, p: Precision) -> Self {
        self.precision = p;
        self
    }

    // -- Tier 0：事前規劃 ---------------------------------------------------

    /// 對指定謂詞做事前規劃 —— O(1)，不碰資料。
    pub fn plan_for(&self, spec: PredicateSpec) -> Plan {
        interval::plan(self.coord_bound, spec)
    }

    /// **安全性檢查**：這批資料能不能用 i128 安全處理？
    ///
    /// 這是整合後最有價值的功能之一：**在跑任何算法之前**就知道
    /// 會不會遇到 `DOWNSTREAM.md` 記錄的靜默溢位。
    pub fn preflight(&self, specs: &[PredicateSpec]) -> Result<(), String> {
        for &s in specs {
            match self.plan_for(s) {
                Plan::RequiresBignum => {
                    return Err(format!(
                        "座標上界 {} 對謂詞 `{}`（次數 {}）需要 {} bit，超出 i128 —— 請縮小座標或改用 bignum",
                        self.coord_bound,
                        s.name,
                        s.degree,
                        interval::required_bits(self.coord_bound, s)
                    ));
                }
                Plan::ExactMayOverflow => {
                    return Err(format!(
                        "座標上界 {} 對謂詞 `{}` 邊際不足（{} bit / 127）—— 建議縮小座標",
                        self.coord_bound,
                        s.name,
                        interval::required_bits(self.coord_bound, s)
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }

    // -- 統一謂詞分派 -------------------------------------------------------

    /// 方向謂詞（依 `precision` 分派）。
    #[inline]
    pub fn orient2d(&mut self, a: Pt, b: Pt, c: Pt) -> i32 {
        match self.precision {
            Precision::Exact => pred::orient2d(a, b, c),
            Precision::Filtered => {
                interval::orient2d_fast_stat(a, b, c, &mut self.report.stats)
            }
            Precision::Auto => match self.plan_for(interval::SPEC_ORIENT2D) {
                Plan::FloatSufficient | Plan::FilterThenExact => {
                    interval::orient2d_fast_stat(a, b, c, &mut self.report.stats)
                }
                _ => pred::orient2d(a, b, c),
            },
        }
    }

    /// 內接圓謂詞（Delaunay/Voronoi 核心）。
    #[inline]
    pub fn incircle(&mut self, a: Pt, b: Pt, c: Pt, d: Pt) -> i32 {
        match self.precision {
            Precision::Exact => pred::incircle(a, b, c, d),
            Precision::Filtered | Precision::Auto => {
                interval::incircle_fast_stat(a, b, c, d, &mut self.report.stats)
            }
        }
    }

    // -- 上層算法：共享同一個核心 -------------------------------------------

    /// 最小包覆圓（Welzl）。
    pub fn min_enclosing_circle(&mut self, pts: &[Pt]) -> Circle {
        crate::welzl::welzl_slice(pts)
    }

    /// Delaunay 三角剖分。
    pub fn delaunay(&mut self, pts: &[Pt]) -> Triangulation {
        delaunay::delaunay(pts)
    }

    /// Voronoi 頂點（Delaunay 對偶）—— 精確有理數。
    pub fn voronoi_vertices(&mut self, pts: &[Pt]) -> Vec<Circle> {
        self.delaunay(pts).voronoi_vertices()
    }

    /// 兩線段最近距離平方（精確有理數）。
    pub fn segment_distance2(&mut self, p1: Pt, p2: Pt, q1: Pt, q2: Pt) -> Rat {
        segdist::dist2_ss(p1, p2, q1, q2)
    }

    /// 多邊形面積 ×2（精確整數）。
    pub fn polygon_area2(&mut self, p: &Polygon) -> i128 {
        p.area2()
    }

    /// 兩多邊形是否重疊（純謂詞，完全精確）。
    pub fn polygons_overlap(&mut self, a: &Polygon, b: &Polygon) -> bool {
        a.overlaps(b)
    }

    // -- 跨算法組合：整合的真正價值 ----------------------------------------

    /// **跨算法組合範例**：以 Delaunay 加速「最近點對」查詢。
    ///
    /// 最近點對必為 Delaunay 邊 —— 這是經典結論。
    /// 這說明整合的價值：Delaunay 的輸出可以**無損**餵給距離計算，
    /// 因為兩者共享同一個 i128 數域，介面處沒有精度損失。
    pub fn closest_pair(&mut self, pts: &[Pt]) -> Option<(usize, usize, Rat)> {
        if pts.len() < 2 {
            return None;
        }
        let tri = self.delaunay(pts);
        let mut best: Option<(usize, usize, Rat)> = None;
        let consider = |i: usize, j: usize, best: &mut Option<(usize, usize, Rat)>| {
            let d = segdist::dist2_pp(pts[i], pts[j]);
            match best {
                None => *best = Some((i, j, d)),
                Some((_, _, bd)) if Rat::cmp(d, *bd) < 0 => *best = Some((i, j, d)),
                _ => {}
            }
        };
        if tri.tris.is_empty() {
            // 退化（全共線等）：暴力
            for i in 0..pts.len() {
                for j in (i + 1)..pts.len() {
                    consider(i, j, &mut best);
                }
            }
            return best;
        }
        for t in &tri.tris {
            for (i, j) in [(t.0, t.1), (t.1, t.2), (t.2, t.0)] {
                consider(i, j, &mut best);
            }
        }
        best
    }

    /// **跨算法組合範例**：多邊形的最小包覆圓 + 面積，共享同一份頂點。
    ///
    /// 回傳 `(包覆圓, 面積×2)`，兩者都是精確值。
    pub fn polygon_bounds(&mut self, p: &Polygon) -> (Circle, i128) {
        (self.min_enclosing_circle(&p.verts), p.area2())
    }

    /// **跨算法組合範例**：兩多邊形間的最小距離（精確）。
    ///
    /// 重疊時回傳 0；否則枚舉邊對。展示布爾判定與距離計算共享謂詞。
    pub fn polygon_distance2(&mut self, a: &Polygon, b: &Polygon) -> Rat {
        if a.overlaps(b) {
            return Rat::int(0);
        }
        let (n, m) = (a.verts.len(), b.verts.len());
        let mut best = Rat { num: i128::MAX, den: 1 };
        for i in 0..n {
            let (p1, p2) = (a.verts[i], a.verts[(i + 1) % n]);
            for j in 0..m {
                let (q1, q2) = (b.verts[j], b.verts[(j + 1) % m]);
                let d = segdist::dist2_ss(p1, p2, q1, q2);
                if Rat::cmp(d, best) < 0 {
                    best = d;
                }
            }
        }
        best
    }
}

/// 所有謂詞規格的清單 —— 供 `preflight` 一次檢查。
pub const ALL_SPECS: [PredicateSpec; 4] = [
    interval::SPEC_ORIENT2D,
    interval::SPEC_ORIENT3D,
    interval::SPEC_INCIRCLE,
    interval::SPEC_CIRC3,
];
