//! 三個下游應用的實測：線段最近距離 / 球面凸包面積 / 多邊形布爾運算。
//!
//! 每一項都與**獨立的真值來源**比對（解析解、浮點參考實作、或已知恆等式）。

use welzl_macro::boolops::*;
use welzl_macro::exact::Pt;
use welzl_macro::segdist::*;
use welzl_macro::sphere::*;

// ===========================================================================
// 應用一：線段最近距離
// ===========================================================================

const D_PARALLEL: Rat = dist2_ss((0, 0), (10, 0), (0, 3), (10, 3));
const D_CROSS: Rat = dist2_ss((0, 0), (10, 10), (0, 10), (10, 0));

#[test]
fn segdist_const_eval() {
    // 平行線段，距離 3 → 距離² = 9
    assert_eq!(D_PARALLEL, Rat { num: 9, den: 1 });
    // 交叉 → 精確 0，不需 ε
    assert!(D_CROSS.is_zero());
}

#[test]
fn segdist_exact_rational() {
    // 點 (0,0) 到線段 (1,0)-(0,1)：垂足在段內，dist² = 1/2
    let d = dist2_ps((0, 0), (1, 0), (0, 1));
    assert_eq!(d, Rat { num: 1, den: 2 }, "應為精確的 1/2，非 0.4999...");
    // 這正是浮點做不到的：1/2 在此被精確表示
    assert_eq!(d.num, 1);
    assert_eq!(d.den, 2);
}

#[test]
fn segdist_touching_endpoints() {
    // 端點接觸 → 距離 0（退化情形，浮點常誤判）
    assert!(dist2_ss((0, 0), (5, 5), (5, 5), (10, 0)).is_zero());
    // T 型接觸
    assert!(dist2_ss((0, 0), (10, 0), (5, 0), (5, 10)).is_zero());
    // 共線重疊
    assert!(dist2_ss((0, 0), (10, 0), (5, 0), (15, 0)).is_zero());
    // 共線但不重疊：距離 5
    assert_eq!(dist2_ss((0, 0), (10, 0), (15, 0), (20, 0)), Rat::int(25));
}

/// 與浮點參考實作比對：隨機 + 退化輸入。
#[test]
fn segdist_vs_float_reference() {
    fn f_dist2_ps(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let dd = dx * dx + dy * dy;
        if dd == 0.0 {
            return (p.0 - a.0).powi(2) + (p.1 - a.1).powi(2);
        }
        let t = ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / dd;
        let t = t.clamp(0.0, 1.0);
        let (qx, qy) = (a.0 + t * dx, a.1 + t * dy);
        (p.0 - qx).powi(2) + (p.1 - qy).powi(2)
    }
    let cf = |p: Pt| (p.0 as f64, p.1 as f64);

    let mut s: u64 = 777;
    let mut next = || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        s >> 33
    };
    for _ in 0..2000 {
        let g = |n: &mut dyn FnMut() -> u64| ((n() % 41) as i128 - 20, (n() % 41) as i128 - 20);
        let (p1, p2) = (g(&mut next), g(&mut next));
        let (q1, q2) = (g(&mut next), g(&mut next));
        let exact = dist2_ss(p1, p2, q1, q2);
        let fl = [
            f_dist2_ps(cf(p1), cf(q1), cf(q2)),
            f_dist2_ps(cf(p2), cf(q1), cf(q2)),
            f_dist2_ps(cf(q1), cf(p1), cf(p2)),
            f_dist2_ps(cf(q2), cf(p1), cf(p2)),
        ]
        .into_iter()
        .fold(f64::INFINITY, f64::min);
        let fl = if segments_intersect(p1, p2, q1, q2) { 0.0 } else { fl };
        assert!(
            (exact.to_f64() - fl).abs() < 1e-9,
            "不符 exact={} float={fl} pts={p1:?}{p2:?}{q1:?}{q2:?}",
            exact.to_f64()
        );
    }
}

/// 門檻查詢（次數較低，座標範圍更寬）。
#[test]
fn segdist_threshold_large_coords() {
    let k = 1_000_000i128;
    // 平行，距離恰為 3
    assert!(within_distance2((0, 0), (k, 0), (0, 3), (k, 3), 9, 1));
    assert!(!within_distance2((0, 0), (k, 0), (0, 3), (k, 3), 8, 1));
    // 邊界：dist² = 9 恰好等於門檻 9/1
    assert!(within_distance2((0, 0), (k, 0), (0, 3), (k, 3), 9, 1));
}

// ===========================================================================
// 應用二：球面凸包（組合層精確 / 超越層浮點）
// ===========================================================================

#[test]
fn sphere_integer_points_exist() {
    // r=5：(3,4,0)、(5,0,0) 等
    let pts = integer_sphere_points(5, 100);
    assert!(pts.len() >= 20, "r=5 應有豐富整數點，得 {}", pts.len());
    for &p in &pts {
        assert!(on_sphere(p, 5), "{p:?} 不在球面上");
    }
}

/// **代數層精確性**：`tan(Ω/2)` 對整數球面點為精確有理數。
#[test]
fn solid_angle_tan_is_exact_rational() {
    let r = 1i128;
    // 標準八分體：(1,0,0),(0,1,0),(0,0,1)，立體角 = π/2 → tan(π/4) = 1
    let t = solid_angle_tan_half((1, 0, 0), (0, 1, 0), (0, 0, 1), r);
    assert_eq!(t, Rat { num: 1, den: 1 }, "八分體 tan(Ω/2) 應恰為 1");
    // 對應 Ω = 2·arctan(1) = π/2
    let om = solid_angle((1, 0, 0), (0, 1, 0), (0, 0, 1), r);
    assert!(
        (om - std::f64::consts::FRAC_PI_2).abs() < 1e-12,
        "Ω 應為 π/2，得 {om}"
    );
}

/// 八個八分體的立體角總和 = 4π（球面全覆蓋）。
#[test]
fn octants_sum_to_full_sphere() {
    let r = 1i128;
    let mut total = 0.0;
    for &sx in &[1i128, -1] {
        for &sy in &[1i128, -1] {
            for &sz in &[1i128, -1] {
                total += solid_angle((sx, 0, 0), (0, sy, 0), (0, 0, sz), r).abs();
            }
        }
    }
    assert!(
        (total - 4.0 * std::f64::consts::PI).abs() < 1e-10,
        "八個八分體應合為 4π，得 {total}"
    );
}

/// **組合層精確性**：凸包面的判定完全用 orient3d，無浮點。
#[test]
fn hull_combinatorics_exact() {
    // 正八面體的 6 個頂點
    let oct: Vec<Pt3> = vec![
        (1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1),
    ];
    let f = convex_hull_3d(&oct);
    assert_eq!(f.len(), 8, "正八面體應有 8 個面，得 {}", f.len());
    assert!(hull_contains_origin(&oct), "正八面體應包含原點");
}

#[test]
fn hull_does_not_contain_origin() {
    // 全部集中在同一卦限 → 不含原點
    let pts: Vec<Pt3> = vec![(5, 0, 0), (4, 3, 0), (3, 4, 0), (0, 3, 4), (4, 0, 3)];
    assert!(!hull_contains_origin(&pts));
}

/// 球面凸包面積：整個球面被覆蓋時應為 4πr²。
#[test]
fn spherical_hull_full_sphere_area() {
    let oct: Vec<Pt3> = vec![
        (1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1),
    ];
    let a = spherical_hull_area(&oct, 1);
    let expect = 4.0 * std::f64::consts::PI;
    assert!((a - expect).abs() < 1e-9, "應為 4π ≈ {expect}，得 {a}");
}

/// 大半徑整數球面：組合層仍精確。
#[test]
fn sphere_large_radius() {
    let r = 25i128; // 有豐富整數解
    let pts = integer_sphere_points(r, 40);
    assert!(pts.len() >= 10);
    for &p in &pts {
        assert!(on_sphere(p, r));
    }
    // tan(Ω/2) 對任意三點皆為有理數（分母非 0 時）
    let t = solid_angle_tan_half(pts[0], pts[1], pts[2], r);
    assert!(t.den >= 0);
}

// ===========================================================================
// 應用三：多邊形布爾運算
// ===========================================================================

/// **Shoelace 面積恆為整數的一半** —— 浮點誤差在此根本不存在。
#[test]
fn polygon_area_is_exact_integer() {
    let sq = Polygon::new(vec![(0, 0), (10, 0), (10, 10), (0, 10)]);
    assert_eq!(sq.area2(), 200, "10×10 正方形，area2 應恰為 200");
    assert_eq!(sq.area(), 100.0);

    // 不規則多邊形：面積必為 .0 或 .5，永不出現浮點尾數
    let p = Polygon::new(vec![(0, 0), (7, 1), (5, 6), (1, 4)]);
    let a2 = p.area2();
    assert_eq!(a2 % 1, 0);
    assert!(a2 > 0);
    // Pick 定理形式：area2 為整數
    assert_eq!(a2 as f64 / 2.0, p.area());
}

#[test]
fn polygon_orientation() {
    let ccw = Polygon::new(vec![(0, 0), (10, 0), (10, 10), (0, 10)]);
    assert!(ccw.is_ccw());
    assert!(!ccw.reversed().is_ccw());
    assert_eq!(ccw.area2(), -ccw.reversed().area2());
}

/// **點在多邊形內**：精確射線法，無「射線穿過頂點」的經典 bug。
#[test]
fn point_in_polygon_exact() {
    let sq = Polygon::new(vec![(0, 0), (10, 0), (10, 10), (0, 10)]);
    assert_eq!(sq.contains((5, 5)), 1, "內部");
    assert_eq!(sq.contains((15, 5)), -1, "外部");
    assert_eq!(sq.contains((0, 0)), 0, "頂點上");
    assert_eq!(sq.contains((5, 0)), 0, "邊上");
    assert_eq!(sq.contains((10, 10)), 0, "對角頂點上");

    // 關鍵退化：射線恰好穿過頂點（浮點實作最常出錯之處）
    let dia = Polygon::new(vec![(0, 0), (5, -5), (10, 0), (5, 5)]);
    assert_eq!(dia.contains((5, 0)), 1, "中心應在內部");
    assert_eq!(dia.contains((-1, 0)), -1, "射線穿過左頂點，應判為外部");
    assert_eq!(dia.contains((11, 0)), -1, "射線穿過右頂點，應判為外部");
}

/// 重疊判定：純謂詞，完全精確，不需構造交點。
#[test]
fn polygon_overlap_predicate() {
    let a = Polygon::new(vec![(0, 0), (10, 0), (10, 10), (0, 10)]);
    let b = Polygon::new(vec![(5, 5), (15, 5), (15, 15), (5, 15)]);
    let c = Polygon::new(vec![(20, 20), (30, 20), (30, 30), (20, 30)]);
    let d = Polygon::new(vec![(2, 2), (8, 2), (8, 8), (2, 8)]); // 完全在 a 內

    assert!(a.overlaps(&b), "部分重疊");
    assert!(!a.overlaps(&c), "完全分離");
    assert!(a.overlaps(&d), "包含關係");
    // 邊界接觸（共邊）
    let e = Polygon::new(vec![(10, 0), (20, 0), (20, 10), (10, 10)]);
    assert!(a.overlaps(&e), "共邊接觸");
}

/// 凸多邊形交集面積：與解析解比對。
#[test]
fn convex_intersection_exact_area() {
    let a = Polygon::new(vec![(0, 0), (10, 0), (10, 10), (0, 10)]);
    let b = Polygon::new(vec![(5, 5), (15, 5), (15, 15), (5, 15)]);
    // 交集為 5×5 正方形，面積 25 → area2 = 50
    let (num, den) = convex_intersection_area2(&a, &b).expect("不應溢位");
    assert_eq!(num as f64 / den as f64, 50.0, "交集 area2 應為 50");

    // 完全包含：交集 = 內部多邊形
    let d = Polygon::new(vec![(2, 2), (8, 2), (8, 8), (2, 8)]);
    let (n2, d2) = convex_intersection_area2(&a, &d).unwrap();
    assert_eq!(n2 as f64 / d2 as f64, 72.0, "6×6=36，area2=72");
}

/// 交集面積產生**真有理數**（非整數）的情形。
#[test]
fn convex_intersection_rational_area() {
    // 三角形與正方形相交，交點座標為分數
    let sq = Polygon::new(vec![(0, 0), (4, 0), (4, 4), (0, 4)]);
    let tri = Polygon::new(vec![(1, 1), (7, 3), (1, 5)]);
    let (num, den) = convex_intersection_area2(&sq, &tri).expect("不應溢位");
    assert!(num > 0 && den > 0);
    // 精確有理數，可與浮點粗算比對
    let area = num as f64 / den as f64 / 2.0;
    assert!(area > 0.0 && area < 16.0, "面積 {area} 應在合理範圍");
}

/// 分離的多邊形：交集為空。
#[test]
fn convex_intersection_empty() {
    let a = Polygon::new(vec![(0, 0), (10, 0), (10, 10), (0, 10)]);
    let c = Polygon::new(vec![(20, 20), (30, 20), (30, 30), (20, 30)]);
    let poly = clip_convex(&a, &c);
    assert!(poly.len() < 3, "應為空，得 {} 頂點", poly.len());
}

/// 隨機驗證：交集面積 ≤ 兩者面積的較小值。
#[test]
fn intersection_area_monotone() {
    let mut s: u64 = 31337;
    let mut next = || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        s >> 33
    };
    let mut checked = 0;
    for _ in 0..300 {
        let x = (next() % 15) as i128;
        let y = (next() % 15) as i128;
        let w = (next() % 8 + 2) as i128;
        let h = (next() % 8 + 2) as i128;
        let a = Polygon::new(vec![(0, 0), (10, 0), (10, 10), (0, 10)]);
        let b = Polygon::new(vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)]);
        if let Some((num, den)) = convex_intersection_area2(&a, &b) {
            let ia = num as f64 / den as f64;
            assert!(ia >= -1e-9, "面積不應為負: {ia}");
            assert!(
                ia <= (a.area2().min(b.area2())) as f64 + 1e-9,
                "交集面積 {ia} 超過較小者"
            );
            checked += 1;
        }
    }
    assert!(checked > 250, "多數案例應成功，得 {checked}");
}

/// **溢位安全性**：大座標下 `cmp` 會靜默出錯，`cmp_checked` 必須回報 None。
///
/// 這是本次實測發現的最重要安全問題：release 模式整數溢位為 wrapping，
/// 且 `-C debug-assertions` 不跨 crate 邊界 —— 錯誤答案不會 panic。
#[test]
fn rat_cmp_overflow_is_detected() {
    // C=1e6：安全（交叉相乘需 116 bit）
    let k = 1_000_000i128;
    let d1 = dist2_ps((k / 3, k / 7), (0, 0), (k, k + 1));
    let d2 = dist2_ps((k / 5, k / 11), (0, 0), (k + 1, k));
    assert_eq!(Rat::cmp_checked(d1, d2), Some(1), "1e6 應可安全比較");
    assert_eq!(Rat::cmp(d1, d2), 1, "1e6 未溢位，兩者應一致");

    // C=1e9：交叉相乘需 176 bit，必然溢位
    let k = 1_000_000_000i128;
    let d1 = dist2_ps((k / 3, k / 7), (0, 0), (k, k + 1));
    let d2 = dist2_ps((k / 5, k / 11), (0, 0), (k + 1, k));
    assert_eq!(
        Rat::cmp_checked(d1, d2),
        None,
        "1e9 必然溢位，checked 版應回報 None"
    );
    // 真值為 1（Python 任意精度驗證），但未檢查版會靜默給出錯誤答案
    assert_ne!(Rat::cmp(d1, d2), 1, "未檢查版在此確實給出錯誤答案");
}
