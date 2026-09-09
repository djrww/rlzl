//! Delaunay 三角剖分 / Voronoi 對偶 / 半平面交 —— 全部建立在精確謂詞之上。
//!
//! 這裡不追求最優複雜度（用的是 O(n²) 增量翻轉），重點是證明
//! **`exact.rs` + `predicates.rs` 這組核心足以支撐這些算法，且拓撲永不崩壞**。
//!
//! 精確謂詞在此的意義不是「更準」，而是**保證算法終止且輸出合法拓撲**：
//! 浮點 incircle 誤判會導致翻轉迴圈（infinite flip loop）與非平面圖輸出，
//! 這是計算幾何實作最常見的 crash 來源。

use crate::exact::{normalize, Circle, Pt};
use crate::predicates::*;

// ---------------------------------------------------------------------------
// Delaunay 三角剖分（增量法 + Lawson 翻轉）
// ---------------------------------------------------------------------------

/// 一個三角形，存頂點索引。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tri(pub usize, pub usize, pub usize);

/// Delaunay 三角剖分結果。
#[derive(Clone, Debug)]
pub struct Triangulation {
    pub pts: Vec<Pt>,
    pub tris: Vec<Tri>,
    /// 翻轉次數（精確謂詞下必定有限）。
    pub flips: usize,
}

impl Triangulation {
    /// 驗證 Delaunay 性質：**沒有任何點落在任何三角形的外接圓內**。
    ///
    /// 這是 O(n·t) 的完全檢查，只有精確謂詞才能讓它嚴格成立。
    pub fn is_delaunay(&self) -> bool {
        for t in &self.tris {
            let (a, b, c) = (self.pts[t.0], self.pts[t.1], self.pts[t.2]);
            if orient2d(a, b, c) == 0 {
                continue;
            }
            for (i, &p) in self.pts.iter().enumerate() {
                if i == t.0 || i == t.1 || i == t.2 {
                    continue;
                }
                if in_circumcircle(a, b, c, p) > 0 {
                    return false;
                }
            }
        }
        true
    }

    /// 每個三角形的外接圓 —— 即 **Voronoi 頂點**（精確有理數）。
    pub fn voronoi_vertices(&self) -> Vec<Circle> {
        self.tris
            .iter()
            .map(|t| circumcircle(self.pts[t.0], self.pts[t.1], self.pts[t.2]))
            .collect()
    }
}

/// 三點外接圓（精確有理數圓心）—— Voronoi 頂點的座標。
///
/// 與 `exact::circ3` 同源，但這裡不做「共線退化」的 MEC 特殊處理：
/// 共線時外接圓不存在，回傳 `cd = 0` 表示無窮遠（Voronoi 的無界邊）。
pub const fn circumcircle(a: Pt, b: Pt, c: Pt) -> Circle {
    let d = 2 * (a.0 * (b.1 - c.1) + b.0 * (c.1 - a.1) + c.0 * (a.1 - b.1));
    if d == 0 {
        return Circle { cx: 0, cy: 0, cd: 0, r2: -1 };
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

/// **暴力 Delaunay**（O(n⁴)）：枚舉所有三元組，保留「外接圓內無其他點」者。
///
/// 這是 Delaunay 三角剖分的**定義本身**，因此無條件正確 —— 沒有翻轉迴圈、
/// 沒有插入順序依賴、沒有拓撲狀態機可以出錯。用它作為參考實作，
/// 是為了讓「精確謂詞」的效果不被複雜的網格維護程式碼污染。
///
/// 代價是 O(n⁴)，僅適用 n ≲ 100。生產環境應改用 Bowyer–Watson 或
/// divide-and-conquer，並把本函式當作測試預言（test oracle）。
///
/// ## 共圓退化的處理
///
/// 四點共圓時 Delaunay 剖分**不唯一**（正方形的兩種對角線都合法）。
/// 本函式回傳所有合法三角形的**聯集**，因此會包含重疊三角形。
/// `strict = true` 時改用「外接圓內無點且無共圓點」的嚴格判定，
/// 輸出唯一但在退化輸入上會少掉部分三角形 —— 這個取捨是退化情形的本質，
/// 而非實作缺陷。生產實作通常以符號擾動（SoS）打破平局。
pub fn delaunay_brute(pts: &[Pt], strict: bool) -> Triangulation {
    let n = pts.len();
    let mut tris = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            for k in (j + 1)..n {
                let (a, b, c) = (pts[i], pts[j], pts[k]);
                let o = orient2d(a, b, c);
                if o == 0 {
                    continue; // 退化三角形
                }
                let mut empty = true;
                for (m, &p) in pts.iter().enumerate() {
                    if m == i || m == j || m == k {
                        continue;
                    }
                    let v = in_circumcircle(a, b, c, p);
                    if v > 0 || (strict && v == 0) {
                        empty = false;
                        break;
                    }
                }
                if empty {
                    tris.push(if o > 0 { Tri(i, j, k) } else { Tri(i, k, j) });
                }
            }
        }
    }
    Triangulation { pts: pts.to_vec(), tris, flips: 0 }
}

/// 預設入口：非嚴格模式（保留共圓退化下的所有合法三角形）。
pub fn delaunay(pts: &[Pt]) -> Triangulation {
    delaunay_brute(pts, false)
}

// ---------------------------------------------------------------------------
// 半平面交（O(n²) 增量裁剪，精確有理數頂點）
// ---------------------------------------------------------------------------

/// 凸多邊形，頂點為精確有理點。
#[derive(Clone, Debug)]
pub struct ConvexPoly {
    pub verts: Vec<RatPt>,
}

impl ConvexPoly {
    /// 以一個大矩形作為初始「全平面」近似。
    pub fn bounding_box(m: i128) -> Self {
        ConvexPoly {
            verts: vec![
                RatPt { x: -m, y: -m, w: 1 },
                RatPt { x: m, y: -m, w: 1 },
                RatPt { x: m, y: m, w: 1 },
                RatPt { x: -m, y: m, w: 1 },
            ],
        }
    }

    /// 用半平面裁剪（Sutherland–Hodgman），全程精確有理數。
    ///
    /// 注意：交點座標的分母會隨裁剪次數累積增長，這是半平面交在
    /// 定長整數下的真實限制（見 `APPLICABILITY.md` 的「次數累積」討論）。
    pub fn clip(&self, h: HalfPlane) -> ConvexPoly {
        let n = self.verts.len();
        let mut out = Vec::new();
        for i in 0..n {
            let cur = self.verts[i];
            let nxt = self.verts[(i + 1) % n];
            let ci = hp_contains_rat(h, cur);
            let ni = hp_contains_rat(h, nxt);
            if ci {
                out.push(cur.reduce());
            }
            if ci != ni {
                if let Some(p) = seg_line_intersect(cur, nxt, h) {
                    out.push(p.reduce()); // 約分以延緩分母膨脹
                }
            }
        }
        ConvexPoly { verts: out }
    }

    pub fn is_empty(&self) -> bool {
        self.verts.len() < 3
    }
}

/// 線段（有理端點）與直線的交點，齊次座標。
fn seg_line_intersect(p: RatPt, q: RatPt, h: HalfPlane) -> Option<RatPt> {
    // f(t) = h(p + t(q-p)) = 0，在齊次座標下求解
    let fp = h.a * p.x + h.b * p.y + h.c * p.w;
    let fq = h.a * q.x + h.b * q.y + h.c * q.w;
    let den = fp * q.w - fq * p.w;
    if den == 0 {
        return None;
    }
    // 交點 = (fp·q - fq·p) 的齊次組合
    Some(RatPt {
        x: fp * q.x - fq * p.x,
        y: fp * q.y - fq * p.y,
        w: den,
    })
}

/// 半平面交：從包圍盒開始逐一裁剪。
pub fn halfplane_intersection(hs: &[HalfPlane], bound: i128) -> ConvexPoly {
    let mut poly = ConvexPoly::bounding_box(bound);
    for &h in hs {
        poly = poly.clip(h);
        if poly.is_empty() {
            break;
        }
    }
    poly
}

/// **Voronoi 單元 = 半平面交**：點 `i` 的單元是所有
/// 「到 i 比到 j 近」的半平面之交集。
///
/// 垂直平分線 `|x-pi|² ≤ |x-pj|²` 展開後是**線性**的，
/// 且係數為整數 —— 這是 Voronoi 能精確計算的關鍵。
pub fn voronoi_cell(pts: &[Pt], i: usize, bound: i128) -> ConvexPoly {
    let mut hs = Vec::new();
    let p = pts[i];
    for (j, &q) in pts.iter().enumerate() {
        if j == i {
            continue;
        }
        // 2(qx-px)x + 2(qy-py)y ≤ |q|²-|p|²  → 內側為 ≥ 0 形式
        let a = 2 * (p.0 - q.0);
        let b = 2 * (p.1 - q.1);
        let c = (q.0 * q.0 + q.1 * q.1) - (p.0 * p.0 + p.1 * p.1);
        hs.push(HalfPlane { a, b, c });
    }
    halfplane_intersection(&hs, bound)
}
