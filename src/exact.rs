//! 精確有理數算術核心（全部 `const fn`，在編譯期求值）。
//!
//! 圓以 **完全精確** 的有理數形式表示，不使用浮點：
//!
//! ```text
//!   圓心 = (cx / cd , cy / cd)
//!   半徑² = r2 / cd²
//! ```
//!
//! 用同一個分母 `cd` 承載圓心與半徑²，好處是「點在圓內」判定退化成
//! 一次乘法比較，沒有交叉相乘的額外膨脹：
//!
//! ```text
//!   p ∈ D  ⟺  (p.x·cd − cx)² + (p.y·cd − cy)² ≤ r2
//! ```
//!
//! 空圓以 `r2 = -1` 表示：左式恆 ≥ 0，所以判定自然回傳 false，不必特判。

/// 平面上的整數點。用 tuple 而非 struct，方便宏直接把 `(x, y)` 字面量餵進來。
pub type Pt = (i128, i128);

/// 精確圓：圓心 `(cx/cd, cy/cd)`，半徑² `r2/cd²`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Circle {
    pub cx: i128,
    pub cy: i128,
    pub cd: i128,
    pub r2: i128,
}

/// 空集合的最小包覆圓（不含任何點）。
pub const EMPTY: Circle = Circle { cx: 0, cy: 0, cd: 1, r2: -1 };

impl Circle {
    pub const fn is_empty(&self) -> bool {
        self.r2 < 0
    }
    /// 浮點圓心（僅供輸出／人眼檢視，判定一律走精確路徑）。
    pub fn center_f64(&self) -> (f64, f64) {
        (self.cx as f64 / self.cd as f64, self.cy as f64 / self.cd as f64)
    }
    /// 浮點半徑（僅供輸出）。
    pub fn radius_f64(&self) -> f64 {
        if self.r2 < 0 {
            return f64::NAN;
        }
        (self.r2 as f64).sqrt() / (self.cd as f64).abs()
    }
}

const fn iabs(a: i128) -> i128 {
    if a < 0 { -a } else { a }
}

const fn igcd(mut a: i128, mut b: i128) -> i128 {
    a = iabs(a);
    b = iabs(b);
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

/// 約分並把分母正規化成正數。
///
/// 可除性論證：`r2` 恆為 `(px·cd − cx)² + (py·cd − cy)²` 的形式，
/// 若 `g | cd`、`g | cx`、`g | cy`，則每一項都可被 `g²` 整除，故 `r2 / g²` 為整數。
pub const fn normalize(c: Circle) -> Circle {
    if c.r2 < 0 {
        return EMPTY;
    }
    let (mut x, mut y, mut d, mut r2) = (c.cx, c.cy, c.cd, c.r2);
    if d < 0 {
        d = -d;
        x = -x;
        y = -y;
    }
    let g = igcd(igcd(x, y), d);
    if g > 1 {
        x /= g;
        y /= g;
        d /= g;
        r2 /= g * g;
    }
    Circle { cx: x, cy: y, cd: d, r2 }
}

/// 精確判定：點 `p` 是否落在圓 `c` 內（含邊界）。
#[inline]
pub const fn in_circle(c: Circle, p: Pt) -> bool {
    let dx = p.0 * c.cd - c.cx;
    let dy = p.1 * c.cd - c.cy;
    dx * dx + dy * dy <= c.r2
}

/// 一點決定的退化圓（半徑 0）。
pub const fn circ1(a: Pt) -> Circle {
    Circle { cx: a.0, cy: a.1, cd: 1, r2: 0 }
}

/// 兩點為直徑的圓。
pub const fn circ2(a: Pt, b: Pt) -> Circle {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    normalize(Circle {
        cx: a.0 + b.0,
        cy: a.1 + b.1,
        cd: 2,
        r2: dx * dx + dy * dy,
    })
}

/// 三點外接圓。
///
/// 共線（行列式為 0）時退化：取三組兩點圓中「能覆蓋第三點且最小」的那個，
/// 對共線點集必定存在（即兩個極端點構成的圓）。
pub const fn circ3(a: Pt, b: Pt, c: Pt) -> Circle {
    let d = 2 * (a.0 * (b.1 - c.1) + b.0 * (c.1 - a.1) + c.0 * (a.1 - b.1));
    if d == 0 {
        return degenerate3(a, b, c);
    }
    let na = a.0 * a.0 + a.1 * a.1;
    let nb = b.0 * b.0 + b.1 * b.1;
    let nc = c.0 * c.0 + c.1 * c.1;

    let ux = na * (b.1 - c.1) + nb * (c.1 - a.1) + nc * (a.1 - b.1);
    let uy = na * (c.0 - b.0) + nb * (a.0 - c.0) + nc * (b.0 - a.0);

    let ex = ux - a.0 * d;
    let ey = uy - a.1 * d;

    normalize(Circle { cx: ux, cy: uy, cd: d, r2: ex * ex + ey * ey })
}

const fn degenerate3(a: Pt, b: Pt, c: Pt) -> Circle {
    let ab = circ2(a, b);
    let bc = circ2(b, c);
    let ac = circ2(a, c);
    let mut best = EMPTY;
    if in_circle(ab, c) {
        best = ab;
    }
    if in_circle(bc, a) && (best.r2 < 0 || cmp_radius(bc, best) < 0) {
        best = bc;
    }
    if in_circle(ac, b) && (best.r2 < 0 || cmp_radius(ac, best) < 0) {
        best = ac;
    }
    best
}

/// 精確比較兩圓半徑：`r2a/cda² ⋛ r2b/cdb²`，回傳 -1 / 0 / 1。
pub const fn cmp_radius(x: Circle, y: Circle) -> i32 {
    if x.r2 < 0 {
        return if y.r2 < 0 { 0 } else { 1 };
    }
    if y.r2 < 0 {
        return -1;
    }
    let l = x.r2 * (y.cd * y.cd);
    let r = y.r2 * (x.cd * x.cd);
    if l < r {
        -1
    } else if l > r {
        1
    } else {
        0
    }
}

/// 座標安全上界。
///
/// 最深的中間量是 `circ3` 的 `ex² `，量級約 `C⁶`；i128 上限 ≈ 1.7·10³⁸，
/// 故 `C ≲ 3.4·10⁶`。取整到 10⁶ 作為宏層 `const` 斷言的門檻。
pub const COORD_BOUND: i128 = 1_000_000;

/// 供宏在編譯期呼叫的座標範圍檢查（超界即編譯失敗，而不是靜默溢位）。
pub const fn assert_bound(p: Pt) -> Pt {
    assert!(
        iabs(p.0) <= COORD_BOUND && iabs(p.1) <= COORD_BOUND,
        "welzl!: 座標超出 i128 精確算術安全上界 (±1e6)"
    );
    p
}
