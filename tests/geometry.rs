//! 精確謂詞 + Delaunay / Voronoi / 半平面交 的正確性測試。

use welzl_macro::delaunay::*;
use welzl_macro::exact::Pt;
use welzl_macro::predicates::*;

// ---- 謂詞：編譯期即可求值 --------------------------------------------------

const O1: i32 = orient2d((0, 0), (1, 0), (0, 1));
const O2: i32 = orient2d((0, 0), (1, 1), (2, 2));
const IC: i32 = incircle((1, 0), (0, 1), (-1, 0), (0, -1));

#[test]
fn predicates_const() {
    assert_eq!(O1, 1); // 逆時針
    assert_eq!(O2, 0); // 共線，精確偵測
    assert_eq!(IC, 0); // 四點共圓，精確偵測
}

/// 共圓必須回傳精確 0 —— 浮點在大座標下會給出非 0。
#[test]
fn exact_cocircular_detection() {
    for k in [1i128, 1_000, 1_000_000, 100_000_000] {
        // 畢氏三元組：全部落在半徑 5k 的圓上
        let a = (3 * k, 4 * k);
        let b = (5 * k, 0);
        let c = (-5 * k, 0);
        let d = (-3 * k, -4 * k);
        assert_eq!(incircle_det(a, b, c, d), 0, "k={k} 共圓應精確為 0");
    }
}

/// Cassini 恆等式：真值恆為 ±1，是災難性抵消的教科書案例。
/// f64 在 F > 2×10⁸ 時會誤判為 0（共線），精確版永不出錯。
#[test]
fn cassini_catastrophic_cancellation() {
    let (mut x, mut y) = (1i128, 1i128);
    let mut fibs = vec![1i128, 1];
    for _ in 0..80 {
        let z = x + y;
        fibs.push(z);
        x = y;
        y = z;
    }
    let mut checked = 0;
    for n in 30..fibs.len() - 1 {
        let (fm, f0, fp) = (fibs[n - 1], fibs[n], fibs[n + 1]);
        if fp > 3_000_000_000 {
            break;
        }
        // det = fp·fm − f0² = ±1（Cassini）
        let det = orient2d_det((0, 0), (fp, f0), (f0, fm));
        assert_eq!(det.abs(), 1, "Cassini 恆等式應為 ±1，得 {det}");
        checked += 1;
    }
    assert!(checked >= 10, "應檢查足夠多組");
}

// ---- Delaunay --------------------------------------------------------------

#[test]
fn delaunay_random() {
    let mut s: u64 = 2024;
    let mut next = || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        s >> 33
    };
    for _ in 0..60 {
        let n = (next() % 12 + 4) as usize;
        let mut pts: Vec<Pt> = (0..n)
            .map(|_| ((next() % 40) as i128 - 20, (next() % 40) as i128 - 20))
            .collect();
        pts.sort();
        pts.dedup();
        if pts.len() < 3 {
            continue;
        }
        let t = delaunay(&pts);
        assert!(t.is_delaunay(), "非 Delaunay: {pts:?}");
    }
}

/// 完美網格 = 大量四點共圓，是浮點實作最容易崩潰的輸入。
#[test]
fn delaunay_degenerate_grid() {
    let grid: Vec<Pt> = (0..5)
        .flat_map(|x| (0..5).map(move |y| (x as i128 * 10, y as i128 * 10)))
        .collect();
    let t = delaunay(&grid);
    assert!(t.is_delaunay());
    assert!(t.tris.len() > 0);
    // 嚴格模式排除所有共圓三角形 → 網格上為空
    assert_eq!(delaunay_brute(&grid, true).tris.len(), 0);
}

/// 全部共圓：Delaunay 剖分不唯一，非嚴格模式回傳所有合法解。
#[test]
fn delaunay_all_cocircular() {
    let k = 1000i128;
    let pts = vec![
        (3 * k, 4 * k), (4 * k, 3 * k), (5 * k, 0), (4 * k, -3 * k),
        (3 * k, -4 * k), (0, -5 * k), (-3 * k, -4 * k), (-5 * k, 0), (0, 5 * k),
    ];
    let t = delaunay(&pts);
    assert!(t.is_delaunay());
    assert_eq!(delaunay_brute(&pts, true).tris.len(), 0, "嚴格模式應排除全部");
}

/// 大座標（1e8）—— incircle 次數 4，此範圍內仍安全。
#[test]
fn delaunay_large_coords() {
    let k = 100_000_000i128;
    let pts = vec![(0, 0), (k, 0), (0, k), (k, k), (k / 3, k / 2)];
    let t = delaunay(&pts);
    assert!(t.is_delaunay(), "1e8 座標下應仍正確");
}

// ---- Voronoi ---------------------------------------------------------------

#[test]
fn voronoi_vertices_exact() {
    let pts = vec![(0i128, 0i128), (4, 0), (0, 4), (5, 5)];
    let t = delaunay_brute(&pts, true);
    for c in t.voronoi_vertices() {
        assert!(c.cd != 0, "外接圓應存在");
    }
    // 直角等腰三角形 (0,0),(4,0),(0,4) 的外心為 (2,2)
    let c = circumcircle((0, 0), (4, 0), (0, 4));
    assert_eq!((c.cx, c.cy, c.cd), (2, 2, 1));
    assert_eq!(c.r2, 8);
}

/// Voronoi 頂點的分母只依賴原始座標，**不隨點數累積**。
#[test]
fn voronoi_denominator_bounded() {
    let k = 100_000_000i128;
    let pts = vec![(0, 0), (k, 0), (0, k), (k, k), (k / 3, k / 2)];
    let t = delaunay(&pts);
    for c in t.voronoi_vertices() {
        // 分母 = 2·orient2d，量級 O(C²)，遠低於 i128
        assert!(c.cd.abs() < i128::MAX / 1_000_000);
    }
}

// ---- 半平面交 --------------------------------------------------------------

#[test]
fn halfplane_triangle() {
    let hs = vec![
        HalfPlane::from_edge((0, 0), (10, 0)),
        HalfPlane::from_edge((10, 0), (0, 10)),
        HalfPlane::from_edge((0, 10), (0, 0)),
    ];
    let poly = halfplane_intersection(&hs, 100);
    assert_eq!(poly.verts.len(), 3, "應得三角形");
}

#[test]
fn halfplane_side_predicate() {
    let l1 = HalfPlane::from_edge((0, 0), (10, 0));
    let l2 = HalfPlane::from_edge((10, 0), (10, 10));
    let l3 = HalfPlane::from_edge((10, 10), (0, 10));
    // L1∩L2 = (10,0)，在 L3（y ≤ 10 的內側）之內
    assert!(hp_side_of_intersection(l1, l2, l3) > 0);
}

/// **關鍵測試**：約分能把分母膨脹從 127 bit 壓到個位數 bit。
///
/// 這是半平面交（唯一會累積次數的算法）能實用的前提。
#[test]
fn halfplane_denominator_growth_controlled() {
    let mut poly = ConvexPoly::bounding_box(1000);
    for i in 0..8 {
        let a = (i as i128 * 37) % 100 - 50;
        let b = (i as i128 * 53) % 100 - 50;
        poly = poly.clip(HalfPlane { a, b, c: 500 });
        if poly.is_empty() {
            break;
        }
        let maxw = poly.verts.iter().map(|v| v.w.abs()).max().unwrap_or(1);
        let bits = 128 - maxw.leading_zeros();
        assert!(bits < 40, "第 {} 次裁剪分母已達 {} bit，約分失效", i + 1, bits);
    }
}

#[test]
fn voronoi_cell_via_halfplanes() {
    let sites = vec![(0i128, 0i128), (10, 0), (5, 9), (-6, 4)];
    for i in 0..sites.len() {
        let cell = voronoi_cell(&sites, i, 1000);
        assert!(!cell.is_empty(), "site {i} 的 Voronoi 單元不應為空");
        // 單元內的點應離 site i 最近
        for v in &cell.verts {
            let (vx, vy, w) = (v.x, v.y, v.w);
            let di = (vx - sites[i].0 * w).pow(2) + (vy - sites[i].1 * w).pow(2);
            for (j, s) in sites.iter().enumerate() {
                if i == j {
                    continue;
                }
                let dj = (vx - s.0 * w).pow(2) + (vy - s.1 * w).pow(2);
                assert!(di <= dj, "site {i} 單元頂點離 site {j} 更近");
            }
        }
    }
}
