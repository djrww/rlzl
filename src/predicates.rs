//! 計算幾何的三個基本謂詞（exact predicates），全部 `const fn`、全部 i128 精確。
//!
//! ## 為什麼「次數（degree）」決定一切
//!
//! 精確謂詞的可用座標範圍由它在座標中的**多項式次數**決定。
//! 若座標量級為 C、謂詞次數為 d，中間量約 C^d，必須 < 2¹²⁷。
//!
//! | 謂詞 | 用途 | 次數 | i128 座標上界 |
//! |------|------|------|---------------|
//! | [`orient2d`] | 凸包、三角剖分方向判定 | 2 | ~10¹⁸ |
//! | [`incircle`] | **Delaunay / Voronoi 核心** | 4 | ~10⁸ |
//! | `circ3`（MEC） | 最小包覆圓 | 6 | ~10⁶ |
//!
//! **關鍵發現：`incircle` 的次數比 MEC 的 `circ3` 低**，
//! 所以 Delaunay/Voronoi 能用的座標範圍反而**比 MEC 大 100 倍**。
//!
//! ## 為什麼這些謂詞不會「次數爆炸」
//!
//! Delaunay、Voronoi、凸包的所有判定都能化約成**原始輸入座標**的行列式 ——
//! 即使 Voronoi 頂點本身是有理數，判定它的謂詞仍只依賴輸入點。
//! 這是這類算法能用定長整數精確計算的根本原因。
//!
//! 真正會爆炸的是**衍生點回饋成輸入**的情境（見 `APPLICABILITY.md`）。

use crate::exact::Pt;

// ---------------------------------------------------------------------------
// 次數 2：方向謂詞
// ---------------------------------------------------------------------------

/// 三點方向：> 0 逆時針，< 0 順時針，= 0 共線。
///
/// 次數 2，中間量 ≈ 8C²，i128 可容座標約 ±4.6×10¹⁸。
#[inline]
pub const fn orient2d(a: Pt, b: Pt, c: Pt) -> i32 {
    let d = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    if d > 0 {
        1
    } else if d < 0 {
        -1
    } else {
        0
    }
}

/// 方向謂詞的原始行列式值（不只符號）。
#[inline]
pub const fn orient2d_det(a: Pt, b: Pt, c: Pt) -> i128 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

// ---------------------------------------------------------------------------
// 次數 4：內接圓謂詞 —— Delaunay / Voronoi 的核心
// ---------------------------------------------------------------------------

/// `d` 是否在 `a,b,c` 的外接圓內。
///
/// 回傳 > 0 表示在圓內，< 0 在圓外，= 0 共圓（**四點共圓被精確偵測**，
/// 這正是浮點實作最容易出錯、導致三角剖分拓撲崩壞的地方）。
///
/// 前提：`a,b,c` 須為**逆時針**。順時針時符號相反。
///
/// 行列式形式（提升到拋物面）：
/// ```text
///   | ax-dx  ay-dy  (ax-dx)²+(ay-dy)² |
///   | bx-dx  by-dy  (bx-dx)²+(by-dy)² |
///   | cx-dx  cy-dy  (cx-dx)²+(cy-dy)² |
/// ```
/// 次數 4，中間量 ≈ 192C⁴，i128 可容座標約 ±9.7×10⁸。
#[inline]
pub const fn incircle_det(a: Pt, b: Pt, c: Pt, d: Pt) -> i128 {
    let adx = a.0 - d.0;
    let ady = a.1 - d.1;
    let bdx = b.0 - d.0;
    let bdy = b.1 - d.1;
    let cdx = c.0 - d.0;
    let cdy = c.1 - d.1;

    let alift = adx * adx + ady * ady;
    let blift = bdx * bdx + bdy * bdy;
    let clift = cdx * cdx + cdy * cdy;

    alift * (bdx * cdy - bdy * cdx) - blift * (adx * cdy - ady * cdx)
        + clift * (adx * bdy - ady * bdx)
}

/// [`incircle_det`] 的符號版本。
#[inline]
pub const fn incircle(a: Pt, b: Pt, c: Pt, d: Pt) -> i32 {
    let v = incircle_det(a, b, c, d);
    if v > 0 {
        1
    } else if v < 0 {
        -1
    } else {
        0
    }
}

/// 方向無關版：自動修正 `a,b,c` 的定向後判定。
#[inline]
pub const fn in_circumcircle(a: Pt, b: Pt, c: Pt, d: Pt) -> i32 {
    let o = orient2d(a, b, c);
    if o == 0 {
        return 0; // 退化三角形，外接圓未定義
    }
    let v = incircle(a, b, c, d);
    if o > 0 {
        v
    } else {
        -v
    }
}

// ---------------------------------------------------------------------------
// 座標上界（實測值見 tools/bounds.py）
// ---------------------------------------------------------------------------

/// `orient2d` 的安全座標上界（次數 2）。
pub const ORIENT_BOUND: i128 = 4_000_000_000_000_000_000; // ~4×10¹⁸
/// `incircle` 的安全座標上界（次數 4）—— Delaunay/Voronoi 可用範圍。
pub const INCIRCLE_BOUND: i128 = 900_000_000; // ~9×10⁸
/// MEC `circ3` 的安全座標上界（次數 6）。
pub const MEC_BOUND: i128 = 1_000_000; // 10⁶

// ---------------------------------------------------------------------------
// 半平面：以整數係數表示，判定同樣化約成行列式
// ---------------------------------------------------------------------------

/// 有向直線 `a·x + b·y + c ≥ 0` 定義的閉半平面。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HalfPlane {
    pub a: i128,
    pub b: i128,
    pub c: i128,
}

impl HalfPlane {
    /// 由有向邊 `p → q` 構造「左側」半平面。
    pub const fn from_edge(p: Pt, q: Pt) -> Self {
        // 左側：cross(q-p, x-p) ≥ 0
        let a = -(q.1 - p.1);
        let b = q.0 - p.0;
        let c = (q.1 - p.1) * p.0 - (q.0 - p.0) * p.1;
        HalfPlane { a, b, c }
    }
}

/// 兩直線的交點（有理數 `(x/w, y/w)`）。`w = 0` 表示平行。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RatPt {
    pub x: i128,
    pub y: i128,
    pub w: i128,
}

/// 兩半平面邊界的交點，齊次座標表示（**精確有理數，不做除法**）。
pub const fn hp_intersect(l: HalfPlane, m: HalfPlane) -> RatPt {
    let w = l.a * m.b - l.b * m.a;
    let x = l.b * m.c - l.c * m.b;
    let y = l.c * m.a - l.a * m.c;
    RatPt { x, y, w }
}

/// **半平面交的核心謂詞**：`L1 ∩ L2` 的交點是否落在 `L3` 內側。
///
/// 關鍵技巧：**不需要真的算出交點再代入**（那會導致次數累積）。
/// 直接對三條線的係數取 3×3 行列式即可：
///
/// ```text
///   sign(L3(p12)) = sign( det[L1;L2;L3] ) · sign( cross(L1,L2) )
/// ```
///
/// 次數在**線係數**中為 3；若係數本身由座標一次導出（`from_edge`），
/// 則在座標中為次數 3 —— 比 incircle 還低，範圍更寬。
/// 這就是半平面交能穩定用定長整數計算的原因。
pub const fn hp_side_of_intersection(l1: HalfPlane, l2: HalfPlane, l3: HalfPlane) -> i32 {
    let det = l1.a * (l2.b * l3.c - l2.c * l3.b) - l1.b * (l2.a * l3.c - l2.c * l3.a)
        + l1.c * (l2.a * l3.b - l2.b * l3.a);
    let w = l1.a * l2.b - l1.b * l2.a;
    if w == 0 {
        return 0; // L1 ∥ L2，交點不存在
    }
    let s = if det > 0 {
        1
    } else if det < 0 {
        -1
    } else {
        0
    };
    if w > 0 {
        s
    } else {
        -s
    }
}

const fn iabs(a: i128) -> i128 { if a < 0 { -a } else { a } }
const fn igcd(mut a: i128, mut b: i128) -> i128 {
    a = iabs(a); b = iabs(b);
    while b != 0 { let t = a % b; a = b; b = t; }
    a
}

impl RatPt {
    /// 約分齊次座標並正規化分母為正。
    ///
    /// **半平面交的分母膨脹緩解手段**：每次裁剪後約分，可把增長從
    /// 「每次 +32 bit」壓到只保留真正必要的位數。
    /// 但這只能延緩、不能根除 —— 見 `APPLICABILITY.md`。
    pub const fn reduce(self) -> RatPt {
        let g = igcd(igcd(self.x, self.y), self.w);
        if g <= 1 {
            return self;
        }
        let (mut x, mut y, mut w) = (self.x / g, self.y / g, self.w / g);
        if w < 0 { x = -x; y = -y; w = -w; }
        RatPt { x, y, w }
    }
}

/// 點（整數）是否在半平面內側。次數 1。
#[inline]
pub const fn hp_contains(h: HalfPlane, p: Pt) -> bool {
    h.a * p.0 + h.b * p.1 + h.c >= 0
}

/// 有理點是否在半平面內側（齊次，避免除法）。
#[inline]
pub const fn hp_contains_rat(h: HalfPlane, p: RatPt) -> bool {
    let v = h.a * p.x + h.b * p.y + h.c * p.w;
    if p.w > 0 {
        v >= 0
    } else {
        v <= 0
    }
}
