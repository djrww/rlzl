//! 應用三：多邊形布爾運算（交 / 聯 / 差）。
//!
//! ## 這是三個應用中最困難的一個 —— 而困難點正好暴露了整數幾何的核心矛盾
//!
//! 布爾運算需要**計算交點**，而兩條整數線段的交點是**有理數**。
//! 若把有理數交點當成新頂點餵給下一次運算（CSG 鏈式操作），
//! 分母會逐次累積 —— 這正是 `GEOMETRY.md` 裡半平面交遇到的「次數累積」，
//! 但更嚴重，因為布爾運算的輸出通常要成為下一步的輸入。
//!
//! ## 本模組的處理方式：分離「判定」與「構造」
//!
//! * **判定層**（`orient2d`、`point_in_polygon`）—— 只依賴原始整數座標，
//!   次數 2，永不累積，**完全精確**。
//! * **構造層**（交點座標）—— 以齊次有理數 [`RatPt`] 精確表示，
//!   但**不保證能無限鏈式運算**。
//!
//! 這個分離讓「A 與 B 是否重疊」「點是否在結果內」這類**謂詞式查詢**
//! 可以無條件精確回答，即使完整的輸出多邊形難以維持整數表示。
//!
//! ## 實務結論（見 `GEOMETRY.md` §4 與本檔測試）
//!
//! 單次布爾運算：✅ 精確可行。
//! 鏈式 CSG（結果再當輸入）：⚠️ 需約分，或改用縮放到整數格點的 snap-rounding。

use crate::exact::Pt;
use crate::predicates::{orient2d, orient2d_det, RatPt};
use crate::segdist::segments_intersect;

/// 簡單多邊形（頂點按序，不自交）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Polygon {
    pub verts: Vec<Pt>,
}

impl Polygon {
    pub fn new(verts: Vec<Pt>) -> Self {
        Polygon { verts }
    }

    /// **有向面積 × 2**（Shoelace 公式）—— 恆為整數，**完全精確**。
    ///
    /// 這是整數幾何最漂亮的性質之一：多邊形面積的兩倍必為整數
    /// （Pick 定理的基礎），因此浮點誤差在此**根本不存在**。
    /// 次數 2，座標可到 ~10¹⁸。
    pub fn area2(&self) -> i128 {
        let n = self.verts.len();
        let mut s: i128 = 0;
        for i in 0..n {
            let a = self.verts[i];
            let b = self.verts[(i + 1) % n];
            s += a.0 * b.1 - b.0 * a.1;
        }
        s
    }

    /// 面積（浮點，僅供輸出）。精確值為 `area2() / 2`。
    pub fn area(&self) -> f64 {
        self.area2() as f64 / 2.0
    }

    /// 是否逆時針。
    pub fn is_ccw(&self) -> bool {
        self.area2() > 0
    }

    pub fn reversed(&self) -> Polygon {
        let mut v = self.verts.clone();
        v.reverse();
        Polygon { verts: v }
    }

    /// **點是否在多邊形內**（含邊界）—— 射線法的精確版。
    ///
    /// 回傳 1 = 內部，0 = 邊界上，-1 = 外部。
    ///
    /// 精確整數判定的關鍵：用 `orient2d` 決定交叉方向，
    /// 而非比較浮點交點座標 —— 因此**沒有「射線恰好穿過頂點」的經典 bug**。
    pub fn contains(&self, p: Pt) -> i32 {
        let n = self.verts.len();
        // 先檢查是否在邊界上
        for i in 0..n {
            let a = self.verts[i];
            let b = self.verts[(i + 1) % n];
            if orient2d(a, b, p) == 0 && on_seg(a, b, p) {
                return 0;
            }
        }
        // 繞數法（winding number），全整數
        let mut wn = 0i32;
        for i in 0..n {
            let a = self.verts[i];
            let b = self.verts[(i + 1) % n];
            if a.1 <= p.1 {
                if b.1 > p.1 && orient2d(a, b, p) > 0 {
                    wn += 1;
                }
            } else if b.1 <= p.1 && orient2d(a, b, p) < 0 {
                wn -= 1;
            }
        }
        if wn != 0 { 1 } else { -1 }
    }

    /// 兩多邊形是否重疊（有共同內點）—— **純謂詞，完全精確，不需構造交點**。
    pub fn overlaps(&self, other: &Polygon) -> bool {
        // 邊相交？
        let n = self.verts.len();
        let m = other.verts.len();
        for i in 0..n {
            let a = self.verts[i];
            let b = self.verts[(i + 1) % n];
            for j in 0..m {
                let c = other.verts[j];
                let d = other.verts[(j + 1) % m];
                if segments_intersect(a, b, c, d) {
                    return true;
                }
            }
        }
        // 包含關係？
        if self.verts.iter().any(|&p| other.contains(p) >= 0) {
            return true;
        }
        if other.verts.iter().any(|&p| self.contains(p) >= 0) {
            return true;
        }
        false
    }
}

fn on_seg(a: Pt, b: Pt, p: Pt) -> bool {
    let minx = a.0.min(b.0);
    let maxx = a.0.max(b.0);
    let miny = a.1.min(b.1);
    let maxy = a.1.max(b.1);
    p.0 >= minx && p.0 <= maxx && p.1 >= miny && p.1 <= maxy
}

// ---------------------------------------------------------------------------
// 凸多邊形裁剪（Sutherland–Hodgman）—— 交集的特例，精確有理數輸出
// ---------------------------------------------------------------------------

/// 用**凸**多邊形 `clip` 裁剪任意多邊形 `subj`，得到交集。
///
/// 輸出頂點為精確有理數。這是布爾運算中最容易做對的情形。
pub fn clip_convex(subj: &Polygon, clip: &Polygon) -> Vec<RatPt> {
    let mut out: Vec<RatPt> = subj
        .verts
        .iter()
        .map(|&p| RatPt { x: p.0, y: p.1, w: 1 })
        .collect();
    let m = clip.verts.len();
    for j in 0..m {
        if out.is_empty() {
            break;
        }
        let c = clip.verts[j];
        let d = clip.verts[(j + 1) % m];
        let mut next: Vec<RatPt> = Vec::new();
        let k = out.len();
        for i in 0..k {
            let cur = out[i];
            let nxt = out[(i + 1) % k];
            let si = side_rat(c, d, cur);
            let sn = side_rat(c, d, nxt);
            if si >= 0 {
                next.push(cur.reduce());
            }
            if (si > 0 && sn < 0) || (si < 0 && sn > 0) {
                if let Some(p) = line_seg_isect(c, d, cur, nxt) {
                    next.push(p.reduce());
                }
            }
        }
        out = next;
    }
    out
}

/// 有理點相對於有向直線 `cd` 的一側（精確）。
fn side_rat(c: Pt, d: Pt, p: RatPt) -> i32 {
    // cross(d−c, p/w − c) 的符號，乘上 w 的符號
    let v = (d.0 - c.0) * (p.y - c.1 * p.w) - (d.1 - c.1) * (p.x - c.0 * p.w);
    let s = if v > 0 { 1 } else if v < 0 { -1 } else { 0 };
    if p.w > 0 { s } else { -s }
}

/// 直線 `cd` 與有理線段 `pq` 的交點（齊次座標，精確）。
fn line_seg_isect(c: Pt, d: Pt, p: RatPt, q: RatPt) -> Option<RatPt> {
    let a = -(d.1 - c.1);
    let b = d.0 - c.0;
    let cc = (d.1 - c.1) * c.0 - (d.0 - c.0) * c.1;
    let fp = a * p.x + b * p.y + cc * p.w;
    let fq = a * q.x + b * q.y + cc * q.w;
    let den = fp * q.w - fq * p.w;
    if den == 0 {
        return None;
    }
    Some(RatPt {
        x: fp * q.x - fq * p.x,
        y: fp * q.y - fq * p.y,
        w: den,
    })
}

/// 有理多邊形的**有向面積 ×2**（Shoelace，齊次座標）。
///
/// 因分母不同需通分，量級增長快 —— 這是有理數輸出的代價。
/// 回傳 `(分子, 分母)`，可能溢位，故用 `checked` 版本。
pub fn rat_area2(verts: &[RatPt]) -> Option<(i128, i128)> {
    let n = verts.len();
    if n < 3 {
        return Some((0, 1));
    }
    let mut num: i128 = 0;
    let mut den: i128 = 1;
    for i in 0..n {
        let a = verts[i];
        let b = verts[(i + 1) % n];
        // a.x/a.w · b.y/b.w − b.x/b.w · a.y/a.w
        let t_num = a.x.checked_mul(b.y)?.checked_sub(b.x.checked_mul(a.y)?)?;
        let t_den = a.w.checked_mul(b.w)?;
        // num/den += t_num/t_den
        let g = gcd_i(den, t_den);
        let nd = den.checked_div(g)?.checked_mul(t_den)?;
        let l = num.checked_mul(nd.checked_div(den)?)?;
        let r = t_num.checked_mul(nd.checked_div(t_den)?)?;
        num = l.checked_add(r)?;
        den = nd;
        let g2 = gcd_i(num, den);
        if g2 > 1 {
            num /= g2;
            den /= g2;
        }
    }
    Some((num, den))
}

fn gcd_i(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    if a == 0 { 1 } else { a }
}

/// 凸多邊形交集的面積（精確有理數 `(num, den)`，面積 = num/den/2）。
pub fn convex_intersection_area2(a: &Polygon, b: &Polygon) -> Option<(i128, i128)> {
    let poly = clip_convex(a, b);
    rat_area2(&poly)
}

/// 三角形有向面積 ×2（精確整數）。
pub const fn tri_area2(a: Pt, b: Pt, c: Pt) -> i128 {
    orient2d_det(a, b, c)
}
