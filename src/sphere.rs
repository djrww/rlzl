//! 應用二：球面凸包與面積 —— **本次實測中唯一撞到本質障礙的應用**。
//!
//! ## 障礙：球面面積是超越數，不是有理數
//!
//! 球面三角形面積 = 球面盈餘 `E = A + B + C − π`，其中 π 與各角皆為超越數。
//! **不存在任何整數/有理數表示能承載它。** 這不是 i128 位寬不夠，
//! 而是數學上的不可能 —— 換 bignum 也一樣。
//!
//! 這與前面所有應用形成鮮明對比：MEC 的半徑²、多邊形面積、距離² 都是有理數，
//! 而球面面積**根本不在有理數域內**。
//!
//! ## 但可以精確的部分遠比想像多
//!
//! 拆成三層，只有最後一層必須放棄精確：
//!
//! | 層次 | 內容 | 精確？ |
//! |------|------|--------|
//! | **組合層** | 哪些點在凸包上、面的鄰接關係 | ✅ 完全精確（`orient3d`，次數 3） |
//! | **代數層** | `tan(Ω/2)`（立體角的半角正切） | ✅ **完全精確的有理數** |
//! | **超越層** | `Ω = 2·arctan(...)`、總面積 | ❌ 必須浮點 |
//!
//! ### 代數層為何能精確：Van Oosterom–Strackee 公式
//!
//! ```text
//!   tan(Ω/2) = |a·(b×c)| / ( |a||b||c| + (a·b)|c| + (a·c)|b| + (b·c)|a| )
//! ```
//!
//! 分子是行列式（整數）。分母含 `|a|,|b|,|c|`（一般為無理數）——
//! **但若所有點都落在半徑 R 為整數的球面上**，則 `|a|=|b|=|c|=R`：
//!
//! ```text
//!   tan(Ω/2) = |det| / ( R³ + R(a·b + a·c + b·c) )
//!            = |det| / ( R · (R² + a·b + a·c + b·c) )
//! ```
//!
//! **分子分母皆為整數 → `tan(Ω/2)` 是精確有理數。**
//!
//! 實務意義：立體角的**比較、加總的代數結構、是否為零的判定**都可精確進行；
//! 只有最終要一個「以球面度為單位的數值」時才需要 `arctan`。
//! 而整數點球面（`x²+y²+z²=R²`）在晶格模型、測地網格、方向取樣中很常見。

use crate::segdist::Rat;

/// 3D 整數點。
pub type Pt3 = (i128, i128, i128);

// ---------------------------------------------------------------------------
// 3D 精確謂詞
// ---------------------------------------------------------------------------

/// 3×3 行列式 = `a · (b × c)`（有向體積 ×6）。次數 3。
#[inline]
pub const fn det3(a: Pt3, b: Pt3, c: Pt3) -> i128 {
    a.0 * (b.1 * c.2 - b.2 * c.1) - a.1 * (b.0 * c.2 - b.2 * c.0)
        + a.2 * (b.0 * c.1 - b.1 * c.0)
}

/// `orient3d`：`d` 相對於平面 `abc` 的側。次數 3，i128 座標上界 ~10¹².
#[inline]
pub const fn orient3d(a: Pt3, b: Pt3, c: Pt3, d: Pt3) -> i32 {
    let v = det3(
        (b.0 - a.0, b.1 - a.1, b.2 - a.2),
        (c.0 - a.0, c.1 - a.1, c.2 - a.2),
        (d.0 - a.0, d.1 - a.1, d.2 - a.2),
    );
    if v > 0 {
        1
    } else if v < 0 {
        -1
    } else {
        0
    }
}

#[inline]
pub const fn dot3(a: Pt3, b: Pt3) -> i128 {
    a.0 * b.0 + a.1 * b.1 + a.2 * b.2
}

#[inline]
pub const fn norm2(a: Pt3) -> i128 {
    dot3(a, a)
}

/// 整數半徑檢查：`|a|² == R²` 是否成立（即點是否精確落在球面上）。
#[inline]
pub const fn on_sphere(a: Pt3, r: i128) -> bool {
    norm2(a) == r * r
}

// ---------------------------------------------------------------------------
// 球面凸包 = 3D 凸包（組合層，完全精確）
// ---------------------------------------------------------------------------

/// 凸包的一個三角面（頂點索引，外向定向）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Facet(pub usize, pub usize, pub usize);

/// **3D 凸包**（暴力枚舉，O(n⁴)）—— 對球面點集即球面凸包。
///
/// 定義式實作：三點構成面 ⟺ 其餘所有點都在同一側。
/// 全部用 `orient3d` 判定，**無浮點、無 ε、共面退化被精確偵測**。
///
/// 共面情形（4 點以上共面）會產生多個共面三角形，這是三角化的自然結果。
pub fn convex_hull_3d(pts: &[Pt3]) -> Vec<Facet> {
    let n = pts.len();
    let mut facets = Vec::new();
    if n < 4 {
        return facets;
    }
    for i in 0..n {
        for j in 0..n {
            if j == i {
                continue;
            }
            for k in (j + 1)..n {
                if k == i {
                    continue;
                }
                let mut pos = false;
                let mut neg = false;
                let mut degenerate = false;
                for (m, &p) in pts.iter().enumerate() {
                    if m == i || m == j || m == k {
                        continue;
                    }
                    match orient3d(pts[i], pts[j], pts[k], p) {
                        v if v > 0 => pos = true,
                        v if v < 0 => neg = true,
                        _ => {}
                    }
                    if pos && neg {
                        degenerate = true;
                        break;
                    }
                }
                if degenerate {
                    continue;
                }
                // 所有點在負側 → (i,j,k) 為外向面
                if !pos {
                    // 去重：同一組頂點只保留一次
                    let mut key = [i, j, k];
                    key.sort_unstable();
                    if !facets.iter().any(|f: &Facet| {
                        let mut g = [f.0, f.1, f.2];
                        g.sort_unstable();
                        g == key
                    }) {
                        facets.push(Facet(i, j, k));
                    }
                }
            }
        }
    }
    facets
}

// ---------------------------------------------------------------------------
// 代數層：tan(Ω/2) 精確有理數
// ---------------------------------------------------------------------------

/// **立體角的半角正切**，對整數球面點為**精確有理數**。
///
/// 前提：`a, b, c` 皆滿足 `|·|² = r²`（用 [`on_sphere`] 檢查）。
///
/// 回傳 `tan(Ω/2) = det / (r·(r² + a·b + a·c + b·c))`。
/// 分母為 0 時表示 `Ω = π`（半球），以 `den = 0` 表示無窮大。
pub const fn solid_angle_tan_half(a: Pt3, b: Pt3, c: Pt3, r: i128) -> Rat {
    let num = det3(a, b, c);
    let den = r * (r * r + dot3(a, b) + dot3(a, c) + dot3(b, c));
    if den == 0 {
        return Rat { num: if num >= 0 { 1 } else { -1 }, den: 0 };
    }
    Rat::new(num, den)
}

/// 立體角 Ω（球面度，**浮點** —— 超越層，無法精確）。
///
/// `Ω = 2·atan2(|det|, den)`。
///
/// ⚠️ **必須用 `atan2` 而非 `atan`**：單純 `atan(num/den)` 會丟失分母的符號，
/// 導致 `Ω > π`（鈍角球面三角形，`den < 0`）的分支被錯誤地折回銳角。
/// 這個 bug 在實測中表現為「八個八分體加總得 8π 而非 4π」。
///
/// 取 `|det|` 是因為此處要的是**無向**立體角；定向資訊已由組合層
/// （`convex_hull_3d` 的面定向）承載。
pub fn solid_angle(a: Pt3, b: Pt3, c: Pt3, r: i128) -> f64 {
    let num = det3(a, b, c);
    let den = r * (r * r + dot3(a, b) + dot3(a, c) + dot3(b, c));
    if num == 0 && den == 0 {
        return 0.0;
    }
    // atan2 正確處理 den < 0（Ω > π）與 den = 0（Ω = π）
    2.0 * (num.abs() as f64).atan2(den as f64)
}

/// 球面三角形面積（球面度 × r²）—— 超越層。
pub fn spherical_triangle_area(a: Pt3, b: Pt3, c: Pt3, r: i128) -> f64 {
    solid_angle(a, b, c, r) * (r * r) as f64
}

/// **球面凸包面積**：凸包三角面的立體角總和 × r²。
///
/// 組合層（哪些面）精確，代數層（每個 `tan(Ω/2)`）精確，
/// 只有最後的 `arctan` 與加總是浮點。
pub fn spherical_hull_area(pts: &[Pt3], r: i128) -> f64 {
    let facets = convex_hull_3d(pts);
    let mut total = 0.0;
    for f in &facets {
        total += solid_angle(pts[f.0], pts[f.1], pts[f.2], r).abs();
    }
    total * (r * r) as f64
}

/// 凸包是否包住球心（等價於：立體角總和 = 4π）。
///
/// **這個判定是精確的** —— 用 `orient3d` 檢查原點是否在所有面的內側，
/// 不需要計算任何角度。
pub fn hull_contains_origin(pts: &[Pt3]) -> bool {
    let facets = convex_hull_3d(pts);
    if facets.is_empty() {
        return false;
    }
    facets
        .iter()
        .all(|f| orient3d(pts[f.0], pts[f.1], pts[f.2], (0, 0, 0)) <= 0)
}

/// 生成半徑 `r` 球面上的整數點（暴力搜尋 `x²+y²+z²=r²`）。
///
/// 這類點的存在性由三平方和定理保證；`r` 取 3、5、9、15、25 等有豐富解。
pub fn integer_sphere_points(r: i128, limit: usize) -> Vec<Pt3> {
    let mut out = Vec::new();
    let r2 = r * r;
    for x in -r..=r {
        for y in -r..=r {
            let rem = r2 - x * x - y * y;
            if rem < 0 {
                continue;
            }
            let z = (rem as f64).sqrt().round() as i128;
            if z * z == rem {
                out.push((x, y, z));
                if z != 0 {
                    out.push((x, y, -z));
                }
                if out.len() >= limit {
                    return out;
                }
            }
        }
    }
    out
}
