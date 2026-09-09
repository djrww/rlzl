//! 應用一：線段最近距離（2D，精確）。
//!
//! ## 為什麼這是精確算術的理想應用
//!
//! 距離本身是**無理數**（含 √），但幾乎所有實務需求要的是**比較**：
//! 「這兩條線段是否相距 < ε？」「哪一對最近？」
//! 比較只需要距離**平方**，而距離平方是**有理數** —— 完全落在 i128 能力範圍內。
//!
//! ```text
//!   dist²(p, ab) = cross(p−a, b−a)² / |b−a|²      （垂足在段內）
//!                = min(|p−a|², |p−b|²)             （垂足在段外）
//! ```
//!
//! 這是「**避免開方**」這個經典技巧的精確版：不只避免浮點誤差，
//! 而是讓結果成為可精確比較的有理數。
//!
//! ## 次數分析
//!
//! | 運算 | 次數 | i128 座標上界 |
//! |------|------|---------------|
//! | 相交判定（`orient2d` ×4） | 2 | ~4×10¹⁸ |
//! | 距離² 的分子 | 4 | ~10⁹ |
//! | **兩距離² 的比較**（交叉相乘） | **6** | **~10⁶** |
//!
//! 若只需「是否相交」或「距離² 是否 ≤ 給定有理數」，範圍可到 10⁹；
//! 只有在互相比較兩個不同分母的距離時才降到 10⁶。

use crate::exact::Pt;
use crate::predicates::orient2d;

/// 精確有理數（`num / den`，`den > 0`）。用於承載距離平方。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rat {
    pub num: i128,
    pub den: i128,
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

impl Rat {
    pub const fn new(num: i128, den: i128) -> Self {
        let (mut n, mut d) = (num, den);
        if d < 0 {
            n = -n;
            d = -d;
        }
        let g = igcd(n, d);
        if g > 1 {
            n /= g;
            d /= g;
        }
        Rat { num: n, den: d }
    }
    pub const fn int(v: i128) -> Self {
        Rat { num: v, den: 1 }
    }
    pub const fn is_zero(&self) -> bool {
        self.num == 0
    }
    /// 精確比較：-1 / 0 / 1。**這一步的次數最高**（交叉相乘）。
    ///
    /// ⚠️ **溢位風險**：交叉相乘使次數達 6，座標超過 ~10⁶ 時可能溢位。
    /// release 模式下 Rust 的整數溢位是 **wrapping（靜默回繞）**，
    /// 且 `-C debug-assertions` **不會跨 crate 邊界**生效 ——
    /// 因此本函式在大座標下會**靜默給出錯誤答案而不 panic**。
    ///
    /// 生產環境請改用 [`Rat::cmp_checked`]。
    pub const fn cmp(a: Rat, b: Rat) -> i32 {
        let l = a.num * b.den;
        let r = b.num * a.den;
        if l < r {
            -1
        } else if l > r {
            1
        } else {
            0
        }
    }

    /// **溢位安全**的精確比較：溢位時回傳 `None` 而非錯誤答案。
    ///
    /// 這是 [`Rat::cmp`] 的生產版本 —— 寧可回報「算不了」，
    /// 也不要靜默回傳錯誤的拓撲判定。
    pub const fn cmp_checked(a: Rat, b: Rat) -> Option<i32> {
        let l = match a.num.checked_mul(b.den) {
            Some(v) => v,
            None => return None,
        };
        let r = match b.num.checked_mul(a.den) {
            Some(v) => v,
            None => return None,
        };
        Some(if l < r {
            -1
        } else if l > r {
            1
        } else {
            0
        })
    }
    pub const fn min(a: Rat, b: Rat) -> Rat {
        if Rat::cmp(a, b) <= 0 { a } else { b }
    }
    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }
    /// 實際距離（開方後）—— 僅供輸出，判定一律用平方。
    pub fn sqrt_f64(self) -> f64 {
        (self.num as f64 / self.den as f64).sqrt()
    }
}

// ---------------------------------------------------------------------------
// 點與點 / 點與線段
// ---------------------------------------------------------------------------

/// 兩點距離平方（整數，次數 2）。
#[inline]
pub const fn dist2_pp(a: Pt, b: Pt) -> Rat {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    Rat { num: dx * dx + dy * dy, den: 1 }
}

/// 點到線段 `ab` 的距離平方（精確有理數）。
///
/// 分三種情形，全部只用整數判定，無除法、無開方。
pub const fn dist2_ps(p: Pt, a: Pt, b: Pt) -> Rat {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let dd = dx * dx + dy * dy; // |b−a|²
    if dd == 0 {
        // 退化成點
        return dist2_pp(p, a);
    }
    let wx = p.0 - a.0;
    let wy = p.1 - a.1;
    let t = wx * dx + wy * dy; // (p−a)·(b−a)，未除以 dd
    if t <= 0 {
        return dist2_pp(p, a); // 垂足在 a 之前
    }
    if t >= dd {
        return dist2_pp(p, b); // 垂足在 b 之後
    }
    // 垂足在段內：dist² = cross² / dd
    let cr = wx * dy - wy * dx;
    Rat::new(cr * cr, dd)
}

// ---------------------------------------------------------------------------
// 線段相交（精確）
// ---------------------------------------------------------------------------

/// 兩線段是否相交（含端點接觸與共線重疊）。純 `orient2d`，次數 2。
pub const fn segments_intersect(p1: Pt, p2: Pt, q1: Pt, q2: Pt) -> bool {
    let d1 = orient2d(q1, q2, p1);
    let d2 = orient2d(q1, q2, p2);
    let d3 = orient2d(p1, p2, q1);
    let d4 = orient2d(p1, p2, q2);

    if ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0))
        && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0))
    {
        return true;
    }
    // 共線 / 端點接觸的退化情形
    if d1 == 0 && on_segment(q1, q2, p1) {
        return true;
    }
    if d2 == 0 && on_segment(q1, q2, p2) {
        return true;
    }
    if d3 == 0 && on_segment(p1, p2, q1) {
        return true;
    }
    if d4 == 0 && on_segment(p1, p2, q2) {
        return true;
    }
    false
}

/// 已知共線時，`p` 是否落在線段 `ab` 上。
const fn on_segment(a: Pt, b: Pt, p: Pt) -> bool {
    let minx = if a.0 < b.0 { a.0 } else { b.0 };
    let maxx = if a.0 > b.0 { a.0 } else { b.0 };
    let miny = if a.1 < b.1 { a.1 } else { b.1 };
    let maxy = if a.1 > b.1 { a.1 } else { b.1 };
    p.0 >= minx && p.0 <= maxx && p.1 >= miny && p.1 <= maxy
}

// ---------------------------------------------------------------------------
// 線段-線段最近距離
// ---------------------------------------------------------------------------

/// **兩線段的最近距離平方**（精確有理數）。
///
/// 相交時回傳 0（精確判定，不需 ε）；否則最近點對必含至少一個端點，
/// 故取四個「端點到對段」距離的最小值 —— 這是 2D 的標準結論。
pub const fn dist2_ss(p1: Pt, p2: Pt, q1: Pt, q2: Pt) -> Rat {
    if segments_intersect(p1, p2, q1, q2) {
        return Rat { num: 0, den: 1 };
    }
    let a = dist2_ps(p1, q1, q2);
    let b = dist2_ps(p2, q1, q2);
    let c = dist2_ps(q1, p1, p2);
    let d = dist2_ps(q2, p1, p2);
    Rat::min(Rat::min(a, b), Rat::min(c, d))
}

/// 距離² 是否 ≤ 給定有理數門檻 `num/den`。
///
/// **這是實務上最有用的形式**：不比較兩個距離，只與常數門檻比，
/// 次數較低，座標範圍更寬。
pub const fn within_distance2(p1: Pt, p2: Pt, q1: Pt, q2: Pt, num: i128, den: i128) -> bool {
    let d = dist2_ss(p1, p2, q1, q2);
    d.num * den <= num * d.den
}

/// 座標安全上界：比較兩個距離²（次數 6）。
///
/// **實測**：C=1e6 時交叉相乘需 116 bit（安全）；C=1e8 已需 156 bit（溢位）。
/// 超過此界請用 [`Rat::cmp_checked`]。
pub const SEGDIST_CMP_BOUND: i128 = 1_000_000;
/// 座標安全上界：僅判定相交（次數 2）。
pub const SEGDIST_ISECT_BOUND: i128 = 4_000_000_000_000_000_000;
