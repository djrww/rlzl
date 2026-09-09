//! 區間運算濾波層：**事前規劃**（static plan）+ **執行期濾波**（interval filter）。
//!
//! ## 三層架構
//!
//! ```text
//!   Tier 0  靜態規劃   由座標量級與謂詞次數，O(1) 決定「這批資料該走哪條路」
//!                      —— 完全不做算術，可在編譯期或迴圈外執行一次
//!   Tier 1  區間濾波   f64 區間運算，若 0 ∉ [lo,hi] 則符號確定，直接採信
//!                      —— 快，但可能「不確定」
//!   Tier 2  精確回退   i128 精確算術（僅在 Tier 1 不確定時才執行）
//! ```
//!
//! 核心洞見：**謂詞只需要符號，不需要數值**。
//! 若區間 `[lo, hi]` 不含 0，符號就已確定，無需精確計算 —— 這是
//! Shewchuk 自適應精度謂詞的思想，本模組用區間運算實現。
//!
//! ## 為什麼區間運算是「可證明正確」的濾波
//!
//! 每次浮點運算後向外擴張一個 ulp，保證真值 **必定** 落在區間內
//! （round-to-nearest 誤差 ≤ 0.5 ulp）。因此：
//!
//! * `lo > 0` ⟹ 真值 > 0（**可證明**，非啟發式）
//! * `hi < 0` ⟹ 真值 < 0
//! * `lo ≤ 0 ≤ hi` ⟹ 不確定，必須回退
//!
//! **濾波永不給出錯誤答案**，最壞情況只是退化成「全部走精確路徑」。

use crate::exact::Pt;
use crate::sphere::Pt3;

// ---------------------------------------------------------------------------
// 有向捨入：next_up / next_down
// ---------------------------------------------------------------------------

/// 下一個可表示的浮點數（向 +∞）。
#[inline]
pub fn next_up(x: f64) -> f64 {
    if x.is_nan() || x == f64::INFINITY {
        return x;
    }
    if x == 0.0 {
        return f64::from_bits(1); // 最小正規化下數
    }
    let b = x.to_bits();
    if x > 0.0 {
        f64::from_bits(b + 1)
    } else {
        f64::from_bits(b - 1)
    }
}

/// 下一個可表示的浮點數（向 −∞）。
#[inline]
pub fn next_down(x: f64) -> f64 {
    -next_up(-x)
}

// ---------------------------------------------------------------------------
// 區間
// ---------------------------------------------------------------------------

/// 閉區間 `[lo, hi]`，保證包含真值。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub lo: f64,
    pub hi: f64,
}

impl Interval {
    #[inline]
    pub fn point(v: f64) -> Self {
        Interval { lo: v, hi: v }
    }

    /// 由 i128 建構。**i128 → f64 可能失真**，故向外擴張一個 ulp。
    #[inline]
    pub fn from_i128(v: i128) -> Self {
        let f = v as f64;
        if (f as i128) == v {
            Interval { lo: f, hi: f } // 精確可表示
        } else {
            Interval { lo: next_down(f), hi: next_up(f) }
        }
    }

    #[inline]
    pub fn add(self, o: Interval) -> Interval {
        Interval {
            lo: next_down(self.lo + o.lo),
            hi: next_up(self.hi + o.hi),
        }
    }

    #[inline]
    pub fn sub(self, o: Interval) -> Interval {
        Interval {
            lo: next_down(self.lo - o.hi),
            hi: next_up(self.hi - o.lo),
        }
    }

    #[inline]
    pub fn mul(self, o: Interval) -> Interval {
        let (a, b, c, d) = (
            self.lo * o.lo,
            self.lo * o.hi,
            self.hi * o.lo,
            self.hi * o.hi,
        );
        let lo = a.min(b).min(c).min(d);
        let hi = a.max(b).max(c).max(d);
        Interval { lo: next_down(lo), hi: next_up(hi) }
    }

    /// **符號判定**：`Some(±1/0)` 表示確定，`None` 表示不確定（需回退）。
    #[inline]
    pub fn sign(self) -> Option<i32> {
        if self.lo > 0.0 {
            Some(1)
        } else if self.hi < 0.0 {
            Some(-1)
        } else if self.lo == 0.0 && self.hi == 0.0 {
            Some(0) // 精確為 0（僅當所有運算都無捨入時才可能）
        } else {
            None // 區間跨越 0 → 不確定
        }
    }

    /// 區間寬度（診斷用）。
    #[inline]
    pub fn width(self) -> f64 {
        self.hi - self.lo
    }
}

// ---------------------------------------------------------------------------
// Tier 0：事前規劃（static plan）
// ---------------------------------------------------------------------------

/// 規劃結果：這批資料該走哪條路。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// f64 保證足夠（所需位元 < 53，含安全邊際）—— 連濾波都不必，直接用浮點。
    FloatSufficient,
    /// f64 可能不足，但 i128 保證足夠 —— **濾波層的最佳戰場**。
    FilterThenExact,
    /// i128 也可能溢位 —— 必須用 checked 版本或改用 bignum。
    ExactMayOverflow,
    /// 必定溢位 —— 應直接拒絕或改用 bignum。
    RequiresBignum,
}

/// 謂詞的多項式次數與常數因子（決定所需位元）。
#[derive(Clone, Copy, Debug)]
pub struct PredicateSpec {
    pub name: &'static str,
    /// 在座標中的多項式次數。
    pub degree: u32,
    /// 行列式展開的項數（位元的加法常數 ≈ log2(項數)）。
    pub terms: u32,
}

pub const SPEC_ORIENT2D: PredicateSpec = PredicateSpec { name: "orient2d", degree: 2, terms: 4 };
pub const SPEC_INCIRCLE: PredicateSpec = PredicateSpec { name: "incircle", degree: 4, terms: 192 };
pub const SPEC_ORIENT3D: PredicateSpec = PredicateSpec { name: "orient3d", degree: 3, terms: 12 };
pub const SPEC_CIRC3: PredicateSpec = PredicateSpec { name: "circ3 (MEC)", degree: 6, terms: 64 };
pub const SPEC_RATCMP: PredicateSpec = PredicateSpec { name: "Rat::cmp", degree: 6, terms: 2 };

/// **事前規劃**：由座標上界估算所需位元，決定執行路徑。
///
/// 這是 O(1) 的純算術，不碰任何實際資料 —— 可在迴圈外執行一次，
/// 甚至在編譯期（若座標範圍為常數）。
///
/// 位元估算：`bits ≈ degree · log2(C) + log2(terms) + 1`（符號位）
pub fn plan(coord_bound: i128, spec: PredicateSpec) -> Plan {
    let bits = required_bits(coord_bound, spec);
    if bits + 3 < 53 {
        Plan::FloatSufficient
    } else if bits + 4 < 127 {
        Plan::FilterThenExact
    } else if bits < 127 {
        Plan::ExactMayOverflow
    } else {
        Plan::RequiresBignum
    }
}

/// 估算謂詞所需的位元數。
pub fn required_bits(coord_bound: i128, spec: PredicateSpec) -> u32 {
    if coord_bound <= 1 {
        return spec.terms.ilog2() + 1;
    }
    let log2c = 128 - (coord_bound.unsigned_abs()).leading_zeros();
    spec.degree * log2c + spec.terms.ilog2() + 1
}

/// 給定謂詞與位元預算，反推**最大安全座標**。
pub fn max_safe_coord(spec: PredicateSpec, budget_bits: u32) -> i128 {
    let overhead = spec.terms.ilog2() + 1;
    if budget_bits <= overhead {
        return 0;
    }
    let per = (budget_bits - overhead) / spec.degree;
    if per >= 127 {
        i128::MAX
    } else {
        1i128 << per
    }
}

// ---------------------------------------------------------------------------
// Tier 1：區間濾波版謂詞
// ---------------------------------------------------------------------------

/// 濾波統計（診斷用，可測量成功率）。
#[derive(Clone, Copy, Debug, Default)]
pub struct FilterStats {
    /// 濾波成功（符號已確定，未回退）。
    pub filtered: u64,
    /// 濾波失敗（回退到精確路徑）。
    pub fallback: u64,
}

impl FilterStats {
    pub fn total(&self) -> u64 {
        self.filtered + self.fallback
    }
    /// 成功率（0.0 ~ 1.0）。
    pub fn success_rate(&self) -> f64 {
        if self.total() == 0 {
            return 0.0;
        }
        self.filtered as f64 / self.total() as f64
    }
}

/// `orient2d` 的區間濾波版。`None` 表示不確定，需回退精確路徑。
#[inline]
pub fn orient2d_interval(a: Pt, b: Pt, c: Pt) -> Option<i32> {
    let i = |v: i128| Interval::from_i128(v);
    let bax = i(b.0).sub(i(a.0));
    let bay = i(b.1).sub(i(a.1));
    let cax = i(c.0).sub(i(a.0));
    let cay = i(c.1).sub(i(a.1));
    bax.mul(cay).sub(bay.mul(cax)).sign()
}

/// **完整的自適應 `orient2d`**：先濾波，不確定才走精確。
///
/// 語意與 `predicates::orient2d` 完全相同，但通常快得多。
#[inline]
pub fn orient2d_adaptive(a: Pt, b: Pt, c: Pt) -> i32 {
    match orient2d_interval(a, b, c) {
        Some(s) => s,
        None => crate::predicates::orient2d(a, b, c),
    }
}

/// 帶統計的版本（測量成功率用）。
#[inline]
pub fn orient2d_adaptive_stat(a: Pt, b: Pt, c: Pt, st: &mut FilterStats) -> i32 {
    match orient2d_interval(a, b, c) {
        Some(s) => {
            st.filtered += 1;
            s
        }
        None => {
            st.fallback += 1;
            crate::predicates::orient2d(a, b, c)
        }
    }
}

/// `incircle` 的區間濾波版。
#[inline]
pub fn incircle_interval(a: Pt, b: Pt, c: Pt, d: Pt) -> Option<i32> {
    let i = |v: i128| Interval::from_i128(v);
    let adx = i(a.0).sub(i(d.0));
    let ady = i(a.1).sub(i(d.1));
    let bdx = i(b.0).sub(i(d.0));
    let bdy = i(b.1).sub(i(d.1));
    let cdx = i(c.0).sub(i(d.0));
    let cdy = i(c.1).sub(i(d.1));

    let alift = adx.mul(adx).add(ady.mul(ady));
    let blift = bdx.mul(bdx).add(bdy.mul(bdy));
    let clift = cdx.mul(cdx).add(cdy.mul(cdy));

    let t1 = alift.mul(bdx.mul(cdy).sub(bdy.mul(cdx)));
    let t2 = blift.mul(adx.mul(cdy).sub(ady.mul(cdx)));
    let t3 = clift.mul(adx.mul(bdy).sub(ady.mul(bdx)));
    t1.sub(t2).add(t3).sign()
}

/// 自適應 `incircle`。
#[inline]
pub fn incircle_adaptive(a: Pt, b: Pt, c: Pt, d: Pt) -> i32 {
    match incircle_interval(a, b, c, d) {
        Some(s) => s,
        None => crate::predicates::incircle(a, b, c, d),
    }
}

#[inline]
pub fn incircle_adaptive_stat(a: Pt, b: Pt, c: Pt, d: Pt, st: &mut FilterStats) -> i32 {
    match incircle_interval(a, b, c, d) {
        Some(s) => {
            st.filtered += 1;
            s
        }
        None => {
            st.fallback += 1;
            crate::predicates::incircle(a, b, c, d)
        }
    }
}

/// `orient3d` 的區間濾波版。
#[inline]
pub fn orient3d_interval(a: Pt3, b: Pt3, c: Pt3, d: Pt3) -> Option<i32> {
    let i = |v: i128| Interval::from_i128(v);
    let (ax, ay, az) = (i(b.0).sub(i(a.0)), i(b.1).sub(i(a.1)), i(b.2).sub(i(a.2)));
    let (bx, by, bz) = (i(c.0).sub(i(a.0)), i(c.1).sub(i(a.1)), i(c.2).sub(i(a.2)));
    let (cx, cy, cz) = (i(d.0).sub(i(a.0)), i(d.1).sub(i(a.1)), i(d.2).sub(i(a.2)));
    let m1 = by.mul(cz).sub(bz.mul(cy));
    let m2 = bx.mul(cz).sub(bz.mul(cx));
    let m3 = bx.mul(cy).sub(by.mul(cx));
    ax.mul(m1).sub(ay.mul(m2)).add(az.mul(m3)).sign()
}

#[inline]
pub fn orient3d_adaptive(a: Pt3, b: Pt3, c: Pt3, d: Pt3) -> i32 {
    match orient3d_interval(a, b, c, d) {
        Some(s) => s,
        None => crate::sphere::orient3d(a, b, c, d),
    }
}

// ---------------------------------------------------------------------------
// 有理數比較的濾波 + 事前溢位規劃
// ---------------------------------------------------------------------------

use crate::segdist::Rat;

/// **事前判定** `Rat::cmp` 是否會溢位 —— 純位元檢查，不做乘法。
///
/// 這是最便宜的「事前規劃」：`leading_zeros` 是單一 CPU 指令。
#[inline]
pub fn ratcmp_will_overflow(a: Rat, b: Rat) -> bool {
    let bits = |v: i128| 128 - v.unsigned_abs().leading_zeros();
    bits(a.num) + bits(b.den) > 126 || bits(b.num) + bits(a.den) > 126
}

/// 有理數比較的區間濾波版。
#[inline]
pub fn ratcmp_interval(a: Rat, b: Rat) -> Option<i32> {
    let l = Interval::from_i128(a.num).mul(Interval::from_i128(b.den));
    let r = Interval::from_i128(b.num).mul(Interval::from_i128(a.den));
    l.sub(r).sign()
}

/// **完整的自適應有理數比較**：三層決策。
///
/// 1. 區間濾波確定 → 直接回傳
/// 2. 事前規劃判定 i128 安全 → 走精確
/// 3. 事前規劃判定會溢位 → 回傳 `None`（拒絕給出可能錯誤的答案）
#[inline]
pub fn ratcmp_adaptive(a: Rat, b: Rat) -> Option<i32> {
    if let Some(s) = ratcmp_interval(a, b) {
        return Some(s);
    }
    if ratcmp_will_overflow(a, b) {
        return None; // 寧可說「算不了」，也不靜默出錯
    }
    Some(Rat::cmp(a, b))
}

// ---------------------------------------------------------------------------
// Tier 1b：靜態誤差界濾波（Shewchuk 式）—— 實測比區間運算快一個數量級
// ---------------------------------------------------------------------------
//
// 區間運算每步都要 next_up/next_down（位元操作 + 分支），實測比 i128 乘法還貴。
// 標準做法改為 **靜態誤差界**：用普通 f64 算出近似值 `det`，
// 並用預先推導的界 `bound` 判斷符號是否可信：
//
//     |det| > bound  ⟹  符號確定
//
// `bound` 由浮點誤差分析預先算出（編譯期常數），執行期只多一次比較。

/// `orient2d` 的靜態誤差界係數（Shewchuk 1997）。
///
/// `ccwerrboundA = (3 + 16ε)·ε`，其中 ε = 2⁻⁵³。
pub const CCW_ERR_A: f64 = 3.330669073875472e-16;

/// `incircle` 的靜態誤差界係數。`iccerrboundA = (10 + 96ε)·ε`。
pub const ICC_ERR_A: f64 = 1.1102230246251577e-15;

/// **靜態誤差界濾波版 `orient2d`** —— 每次只多一次乘法與比較。
#[inline(always)]
pub fn orient2d_filter(a: Pt, b: Pt, c: Pt) -> Option<i32> {
    let (ax, ay) = (a.0 as f64, a.1 as f64);
    let (bx, by) = (b.0 as f64, b.1 as f64);
    let (cx, cy) = (c.0 as f64, c.1 as f64);
    let detleft = (bx - ax) * (cy - ay);
    let detright = (by - ay) * (cx - ax);
    let det = detleft - detright;
    // 誤差界正比於運算元量級之和
    let detsum = if detleft > 0.0 {
        if detright <= 0.0 {
            return Some(if det > 0.0 { 1 } else if det < 0.0 { -1 } else { 0 });
        }
        detleft + detright
    } else if detleft < 0.0 {
        if detright >= 0.0 {
            return Some(if det > 0.0 { 1 } else if det < 0.0 { -1 } else { 0 });
        }
        -detleft - detright
    } else {
        return Some(if det > 0.0 { 1 } else if det < 0.0 { -1 } else { 0 });
    };
    let errbound = CCW_ERR_A * detsum;
    if det >= errbound || -det >= errbound {
        Some(if det > 0.0 { 1 } else { -1 })
    } else {
        None
    }
}

/// 自適應 `orient2d`（靜態誤差界版）—— **生產環境建議使用這個**。
#[inline(always)]
pub fn orient2d_fast(a: Pt, b: Pt, c: Pt) -> i32 {
    match orient2d_filter(a, b, c) {
        Some(s) => s,
        None => crate::predicates::orient2d(a, b, c),
    }
}

#[inline]
pub fn orient2d_fast_stat(a: Pt, b: Pt, c: Pt, st: &mut FilterStats) -> i32 {
    match orient2d_filter(a, b, c) {
        Some(s) => { st.filtered += 1; s }
        None => { st.fallback += 1; crate::predicates::orient2d(a, b, c) }
    }
}

/// **靜態誤差界濾波版 `incircle`**。
#[inline(always)]
pub fn incircle_filter(a: Pt, b: Pt, c: Pt, d: Pt) -> Option<i32> {
    let adx = a.0 as f64 - d.0 as f64;
    let ady = a.1 as f64 - d.1 as f64;
    let bdx = b.0 as f64 - d.0 as f64;
    let bdy = b.1 as f64 - d.1 as f64;
    let cdx = c.0 as f64 - d.0 as f64;
    let cdy = c.1 as f64 - d.1 as f64;

    let bdxcdy = bdx * cdy;
    let cdxbdy = cdx * bdy;
    let alift = adx * adx + ady * ady;
    let cdxady = cdx * ady;
    let adxcdy = adx * cdy;
    let blift = bdx * bdx + bdy * bdy;
    let adxbdy = adx * bdy;
    let bdxady = bdx * ady;
    let clift = cdx * cdx + cdy * cdy;

    let det = alift * (bdxcdy - cdxbdy) + blift * (cdxady - adxcdy) + clift * (adxbdy - bdxady);
    let permanent = (bdxcdy.abs() + cdxbdy.abs()) * alift
        + (cdxady.abs() + adxcdy.abs()) * blift
        + (adxbdy.abs() + bdxady.abs()) * clift;
    let errbound = ICC_ERR_A * permanent;
    if det > errbound || -det > errbound {
        Some(if det > 0.0 { 1 } else { -1 })
    } else {
        None
    }
}

/// 自適應 `incircle`（靜態誤差界版）—— **生產環境建議使用這個**。
#[inline(always)]
pub fn incircle_fast(a: Pt, b: Pt, c: Pt, d: Pt) -> i32 {
    match incircle_filter(a, b, c, d) {
        Some(s) => s,
        None => crate::predicates::incircle(a, b, c, d),
    }
}

#[inline]
pub fn incircle_fast_stat(a: Pt, b: Pt, c: Pt, d: Pt, st: &mut FilterStats) -> i32 {
    match incircle_filter(a, b, c, d) {
        Some(s) => { st.filtered += 1; s }
        None => { st.fallback += 1; crate::predicates::incircle(a, b, c, d) }
    }
}

// ---------------------------------------------------------------------------
// Tier 1c：f64 輸入路徑 —— 濾波真正能贏的唯一場景
// ---------------------------------------------------------------------------
//
// 實測結論（見 `INTERVAL.md`）：當輸入已是 i128 時，`i128 as f64` 轉換
// 就要 6.7 ns，比整個 i128 精確謂詞（5.8 ns）還貴，濾波必定虧本。
//
// 濾波唯一能獲利的場景是**輸入本來就是 f64**（CAD/GIS/圖形學的常態）：
// 此時快路徑無轉換成本，只有在濾波失敗時才付出「轉整數 + 精確計算」的代價。

/// f64 點。
pub type PtF = (f64, f64);

/// 由 f64 點做 `orient2d`，快路徑純 f64，慢路徑轉整數精確計算。
///
/// `scale` 為「f64 → 整數」的縮放因子（例如座標以 μm 為單位時傳 1000.0）。
/// 要求縮放後為整數且在安全範圍內。
#[inline(always)]
pub fn orient2d_f64(a: PtF, b: PtF, c: PtF, scale: f64) -> i32 {
    let detleft = (b.0 - a.0) * (c.1 - a.1);
    let detright = (b.1 - a.1) * (c.0 - a.0);
    let det = detleft - detright;
    let detsum = if detleft > 0.0 {
        if detright <= 0.0 {
            return if det > 0.0 { 1 } else if det < 0.0 { -1 } else { 0 };
        }
        detleft + detright
    } else if detleft < 0.0 {
        if detright >= 0.0 {
            return if det > 0.0 { 1 } else if det < 0.0 { -1 } else { 0 };
        }
        -detleft - detright
    } else {
        return if det > 0.0 { 1 } else if det < 0.0 { -1 } else { 0 };
    };
    let errbound = CCW_ERR_A * detsum;
    if det >= errbound || -det >= errbound {
        return if det > 0.0 { 1 } else { -1 };
    }
    // 慢路徑：轉整數精確計算
    let q = |p: PtF| ((p.0 * scale).round() as i128, (p.1 * scale).round() as i128);
    crate::predicates::orient2d(q(a), q(b), q(c))
}

#[inline]
pub fn orient2d_f64_stat(a: PtF, b: PtF, c: PtF, scale: f64, st: &mut FilterStats) -> i32 {
    let detleft = (b.0 - a.0) * (c.1 - a.1);
    let detright = (b.1 - a.1) * (c.0 - a.0);
    let det = detleft - detright;
    let detsum = if detleft > 0.0 {
        if detright <= 0.0 { st.filtered += 1; return if det > 0.0 { 1 } else if det < 0.0 { -1 } else { 0 }; }
        detleft + detright
    } else if detleft < 0.0 {
        if detright >= 0.0 { st.filtered += 1; return if det > 0.0 { 1 } else if det < 0.0 { -1 } else { 0 }; }
        -detleft - detright
    } else {
        st.filtered += 1;
        return if det > 0.0 { 1 } else if det < 0.0 { -1 } else { 0 };
    };
    let errbound = CCW_ERR_A * detsum;
    if det >= errbound || -det >= errbound {
        st.filtered += 1;
        return if det > 0.0 { 1 } else { -1 };
    }
    st.fallback += 1;
    let q = |p: PtF| ((p.0 * scale).round() as i128, (p.1 * scale).round() as i128);
    crate::predicates::orient2d(q(a), q(b), q(c))
}
