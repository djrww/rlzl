//! 統一核心測試：驗證「共享數域 + 共享謂詞 + 統一分派」確實成立。
use welzl_macro::boolops::Polygon;
use welzl_macro::interval::{Plan, SPEC_CIRC3, SPEC_INCIRCLE, SPEC_ORIENT2D};
use welzl_macro::kernel::*;
use welzl_macro::segdist::Rat;

#[test]
fn preflight_catches_unsafe_coords() {
    // 1e6 座標對 MEC(次數6) 不安全
    let k = Kernel::new(1_000_000);
    assert!(k.preflight(&[SPEC_CIRC3]).is_err(), "應攔截 MEC 溢位風險");
    // 但對 orient2d(次數2) 完全安全
    assert!(k.preflight(&[SPEC_ORIENT2D]).is_ok());
    // 1e3 對全部謂詞都安全
    let k2 = Kernel::new(1_000);
    assert!(k2.preflight(&ALL_SPECS).is_ok(), "1e3 應全部安全");
}

#[test]
fn plan_dispatch_consistent() {
    let k = Kernel::new(1_000);
    assert_eq!(k.plan_for(SPEC_ORIENT2D), Plan::FloatSufficient);
    let k2 = Kernel::new(1_000_000_000);
    assert_eq!(k2.plan_for(SPEC_INCIRCLE), Plan::RequiresBignum);
}

/// 三種精度策略必須給出**完全相同**的答案。
#[test]
fn precision_modes_agree() {
    let pts = vec![(0i128, 0i128), (10, 0), (10, 10), (0, 10), (5, 5), (3, 7), (8, 2)];
    let mut results = Vec::new();
    for p in [Precision::Exact, Precision::Filtered, Precision::Auto] {
        let mut k = Kernel::new(100).with_precision(p);
        let c = k.min_enclosing_circle(&pts);
        let t = k.delaunay(&pts);
        results.push((c, t.tris.len()));
    }
    assert_eq!(results[0], results[1], "Exact vs Filtered 不一致");
    assert_eq!(results[1], results[2], "Filtered vs Auto 不一致");
}

/// 謂詞分派在三種模式下結果一致。
#[test]
fn predicate_dispatch_agrees() {
    let mut s: u64 = 777;
    let mut next = || { s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); s >> 33 };
    for _ in 0..20_000 {
        let g = |k: &mut dyn FnMut() -> u64| ((k() % 201) as i128 - 100, (k() % 201) as i128 - 100);
        let (a, b, c, d) = (g(&mut next), g(&mut next), g(&mut next), g(&mut next));
        let mut ke = Kernel::new(100).with_precision(Precision::Exact);
        let mut kf = Kernel::new(100).with_precision(Precision::Filtered);
        assert_eq!(ke.orient2d(a, b, c), kf.orient2d(a, b, c));
        assert_eq!(ke.incircle(a, b, c, d), kf.incircle(a, b, c, d));
    }
}

/// **跨算法組合**：Delaunay → 最近點對，介面無精度損失。
#[test]
fn closest_pair_via_delaunay() {
    let mut k = Kernel::new(1000);
    let pts = vec![(0i128, 0i128), (100, 0), (50, 80), (52, 82), (10, 90)];
    let (i, j, d) = k.closest_pair(&pts).unwrap();
    // (50,80) 與 (52,82) 距離² = 8
    assert_eq!(d, Rat::int(8), "最近距離² 應為 8");
    assert!((i == 2 && j == 3) || (i == 3 && j == 2));

    // 與暴力法比對
    let mut s: u64 = 31337;
    let mut next = || { s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); s >> 33 };
    for _ in 0..60 {
        let n = (next() % 10 + 3) as usize;
        let mut p: Vec<(i128, i128)> = (0..n).map(|_| ((next() % 61) as i128 - 30, (next() % 61) as i128 - 30)).collect();
        p.sort(); p.dedup();
        if p.len() < 2 { continue; }
        let got = k.closest_pair(&p).unwrap().2;
        let mut want = Rat { num: i128::MAX, den: 1 };
        for a in 0..p.len() { for b in (a + 1)..p.len() {
            let d = welzl_macro::segdist::dist2_pp(p[a], p[b]);
            if Rat::cmp(d, want) < 0 { want = d; }
        }}
        assert_eq!(Rat::cmp(got, want), 0, "最近點對不符: {p:?}");
    }
}

/// **跨算法組合**：多邊形包覆圓 + 面積共享頂點。
#[test]
fn polygon_bounds_combined() {
    let mut k = Kernel::new(1000);
    let sq = Polygon::new(vec![(0, 0), (10, 0), (10, 10), (0, 10)]);
    let (c, a2) = k.polygon_bounds(&sq);
    assert_eq!(a2, 200, "面積×2");
    assert_eq!(c.center_f64(), (5.0, 5.0));
    assert_eq!(c.r2, 50, "外接圓 r²=50");
}

/// **跨算法組合**：布爾判定 + 距離，共享謂詞。
#[test]
fn polygon_distance_combined() {
    let mut k = Kernel::new(1000);
    let a = Polygon::new(vec![(0, 0), (10, 0), (10, 10), (0, 10)]);
    let b = Polygon::new(vec![(20, 0), (30, 0), (30, 10), (20, 10)]);
    // 水平間距 10 → 距離² = 100
    assert_eq!(k.polygon_distance2(&a, &b), Rat::int(100));
    // 重疊 → 0
    let c = Polygon::new(vec![(5, 5), (15, 5), (15, 15), (5, 15)]);
    assert!(k.polygon_distance2(&a, &c).is_zero());
}

/// 濾波統計確實被記錄。
#[test]
fn kernel_reports_stats() {
    let mut k = Kernel::new(100).with_precision(Precision::Filtered);
    for i in 0..100i128 { k.orient2d((0, 0), (i + 1, 0), (0, i + 1)); }
    assert_eq!(k.report.stats.total(), 100);
    assert!(k.report.stats.success_rate() > 0.9);
}
