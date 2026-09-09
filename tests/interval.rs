//! 區間濾波與事前規劃的測試。
//!
//! 最重要的性質：**濾波永不說謊** —— 回傳 `Some(s)` 時，`s` 必等於精確答案。

use welzl_macro::interval::*;
use welzl_macro::predicates as P;
use welzl_macro::segdist::{dist2_ps, Rat};

// ---- 區間運算基本性質 ------------------------------------------------------

#[test]
fn interval_contains_truth() {
    let a = Interval::from_i128(3);
    let b = Interval::from_i128(7);
    let s = a.add(b);
    assert!(s.lo <= 10.0 && 10.0 <= s.hi, "區間必須包含真值 10");
    let m = a.mul(b);
    assert!(m.lo <= 21.0 && 21.0 <= m.hi, "區間必須包含真值 21");
}

#[test]
fn interval_sign_conservative() {
    // 明確正
    assert_eq!(Interval { lo: 1.0, hi: 2.0 }.sign(), Some(1));
    // 明確負
    assert_eq!(Interval { lo: -2.0, hi: -1.0 }.sign(), Some(-1));
    // 跨越 0 → 必須回報不確定
    assert_eq!(Interval { lo: -1.0, hi: 1.0 }.sign(), None);
    assert_eq!(Interval { lo: 0.0, hi: 1.0 }.sign(), None);
    assert_eq!(Interval { lo: -1.0, hi: 0.0 }.sign(), None);
}

#[test]
fn next_up_down_correct() {
    assert!(next_up(1.0) > 1.0);
    assert!(next_down(1.0) < 1.0);
    assert!(next_up(-1.0) > -1.0);
    assert!(next_down(0.0) < 0.0);
    assert_eq!(next_down(next_up(1.0)), 1.0);
}

/// 大整數轉 f64 會失真，區間必須向外擴張。
#[test]
fn from_i128_handles_precision_loss() {
    let big = (1i128 << 80) + 1; // 無法用 f64 精確表示
    let iv = Interval::from_i128(big);
    assert!(iv.lo < iv.hi, "失真時區間應有寬度");
    assert!(iv.lo <= big as f64 && big as f64 <= iv.hi);
    // 小整數應精確
    let small = Interval::from_i128(42);
    assert_eq!(small.lo, small.hi);
}

// ---- 核心性質：濾波永不說謊 ------------------------------------------------

/// 20 萬組隨機：濾波成功時必須與精確答案一致。
#[test]
fn filter_never_lies_orient2d() {
    let mut s: u64 = 42;
    let mut next = || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        s >> 33
    };
    let mut filtered = 0;
    for _ in 0..200_000 {
        let g = |k: &mut dyn FnMut() -> u64| {
            ((k() % 2001) as i128 - 1000, (k() % 2001) as i128 - 1000)
        };
        let (a, b, c) = (g(&mut next), g(&mut next), g(&mut next));
        let truth = P::orient2d(a, b, c);
        if let Some(f) = orient2d_interval(a, b, c) {
            assert_eq!(f, truth, "區間濾波說謊: {a:?} {b:?} {c:?}");
            filtered += 1;
        }
        if let Some(f) = orient2d_filter(a, b, c) {
            assert_eq!(f, truth, "靜態界濾波說謊: {a:?} {b:?} {c:?}");
        }
        // 自適應版必須永遠正確
        assert_eq!(orient2d_adaptive(a, b, c), truth);
        assert_eq!(orient2d_fast(a, b, c), truth);
    }
    assert!(filtered > 190_000, "隨機資料濾波成功率應極高");
}

#[test]
fn filter_never_lies_incircle() {
    let mut s: u64 = 99;
    let mut next = || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        s >> 33
    };
    for _ in 0..100_000 {
        let g = |k: &mut dyn FnMut() -> u64| {
            ((k() % 2001) as i128 - 1000, (k() % 2001) as i128 - 1000)
        };
        let (a, b, c, d) = (g(&mut next), g(&mut next), g(&mut next), g(&mut next));
        let truth = P::incircle(a, b, c, d);
        if let Some(f) = incircle_interval(a, b, c, d) {
            assert_eq!(f, truth, "區間濾波說謊");
        }
        if let Some(f) = incircle_filter(a, b, c, d) {
            assert_eq!(f, truth, "靜態界濾波說謊");
        }
        assert_eq!(incircle_adaptive(a, b, c, d), truth);
        assert_eq!(incircle_fast(a, b, c, d), truth);
    }
}

/// **退化輸入**：濾波必須回報不確定，而不是猜。
#[test]
fn filter_declines_on_degenerate() {
    // 完全共線 → 真值 0，濾波無法確定（區間必含 0）
    for k in [1i128, 100, 10_000] {
        let r = orient2d_interval((0, 0), (k, k), (2 * k, 2 * k));
        assert!(
            r.is_none() || r == Some(0),
            "共線時應回報不確定或精確 0，得 {r:?}"
        );
        // 但自適應版必須給出正確答案
        assert_eq!(orient2d_adaptive((0, 0), (k, k), (2 * k, 2 * k)), 0);
        assert_eq!(orient2d_fast((0, 0), (k, k), (2 * k, 2 * k)), 0);
    }
    // 完全共圓 → 真值 0
    let k = 1000i128;
    assert_eq!(
        incircle_fast((3 * k, 4 * k), (5 * k, 0), (-5 * k, 0), (-3 * k, -4 * k)),
        0
    );
}

/// Cassini 案例：浮點會誤判，濾波必須拒絕而非附和。
#[test]
fn filter_declines_on_cassini() {
    let (mut x, mut y) = (1i128, 1i128);
    let mut fibs = vec![1i128, 1];
    for _ in 0..80 {
        let z = x + y;
        fibs.push(z);
        x = y;
        y = z;
    }
    let mut declined = 0;
    let mut checked = 0;
    for n in 40..fibs.len() - 1 {
        let (fm, f0, fp) = (fibs[n - 1], fibs[n], fibs[n + 1]);
        if fp > 3_000_000_000 {
            break;
        }
        let (a, b, c) = ((0i128, 0i128), (fp, f0), (f0, fm));
        let truth = P::orient2d(a, b, c);
        assert_eq!(truth.abs(), 1, "Cassini 真值應為 ±1");
        // 濾波要嘛拒絕，要嘛給出正確答案 —— 絕不能給錯
        if let Some(f) = orient2d_filter(a, b, c) {
            assert_eq!(f, truth, "濾波在 Cassini 案例說謊！");
        } else {
            declined += 1;
        }
        assert_eq!(orient2d_fast(a, b, c), truth, "自適應版必須正確");
        checked += 1;
    }
    assert!(checked > 5);
    assert!(declined > 0, "Cassini 應觸發至少一次回退");
}

// ---- Tier 0：事前規劃 ------------------------------------------------------

#[test]
fn plan_matches_measured_bounds() {
    // orient2d 次數 2：小座標 f64 足夠
    assert_eq!(plan(1_000, SPEC_ORIENT2D), Plan::FloatSufficient);
    // incircle 次數 4：1e6 需要精確
    assert_eq!(plan(1_000_000, SPEC_INCIRCLE), Plan::FilterThenExact);
    // incircle 在 1e9 已超出 i128（實測 GEOMETRY.md 上界 ~9e8）
    assert_eq!(plan(1_000_000_000, SPEC_INCIRCLE), Plan::RequiresBignum);
    // MEC 次數 6：1e6 就已吃緊
    assert!(matches!(
        plan(1_000_000, SPEC_CIRC3),
        Plan::RequiresBignum | Plan::ExactMayOverflow
    ));
}

#[test]
fn required_bits_monotone() {
    // 位元需求隨座標與次數單調增加
    let b1 = required_bits(1_000, SPEC_ORIENT2D);
    let b2 = required_bits(1_000_000, SPEC_ORIENT2D);
    assert!(b2 > b1);
    let c1 = required_bits(1_000, SPEC_ORIENT2D);
    let c2 = required_bits(1_000, SPEC_INCIRCLE);
    assert!(c2 > c1, "次數高者需要更多位元");
}

/// 反推的安全座標必須與實測邊界一致。
#[test]
fn max_safe_coord_consistent() {
    // incircle 在 i128 下約 2.7e8（實測 GEOMETRY.md 說 ~9e8，同數量級）
    let m = max_safe_coord(SPEC_INCIRCLE, 123);
    assert!(m > 1_000_000 && m < 10_000_000_000, "得 {m}");
    // MEC 更小
    let mec = max_safe_coord(SPEC_CIRC3, 123);
    assert!(mec < m, "MEC 次數更高，安全範圍應更小");
    // f64 預算下範圍大幅縮小
    let f = max_safe_coord(SPEC_INCIRCLE, 50);
    assert!(f < m);
}

// ---- 有理數比較的事前溢位規劃 ----------------------------------------------

#[test]
fn ratcmp_overflow_prediction() {
    // 小值：不會溢位
    let a = Rat::new(3, 4);
    let b = Rat::new(5, 7);
    assert!(!ratcmp_will_overflow(a, b));
    // 3/4 = 0.75 > 5/7 ≈ 0.714
    assert_eq!(Rat::cmp(a, b), 1);
    assert_eq!(ratcmp_adaptive(a, b), Some(1));

    // 大值：`will_overflow` 必須正確預測 i128 交叉相乘會溢位
    let big = Rat { num: i128::MAX / 2, den: 3 };
    let big2 = Rat { num: i128::MAX / 3, den: 5 };
    assert!(ratcmp_will_overflow(big, big2), "應預測到 i128 溢位");
    // 但 ratcmp_adaptive 仍可能靠 f64 區間解決（見下方測試）——
    // 兩者不矛盾：will_overflow 描述的是 i128 路徑，不是整體可解性。
    let r = ratcmp_adaptive(big, big2);
    if let Some(s) = r {
        // 若區間濾波給出答案，必須正確（真值：MAX/2/3 vs MAX/3/5）
        assert_eq!(s, 1, "big > big2");
    }
}

/// **DOWNSTREAM.md 的靜默溢位案例，被區間濾波救回。**
///
/// 這是本次最有價值的發現：`Rat::cmp` 在此靜默給出錯誤答案（實測回傳 -1，
/// Python 任意精度驗證真值為 +1），`will_overflow` 也正確預測 i128 會溢位
/// （需 176 bit）—— 但**區間濾波仍能給出正確答案**。
///
/// 原因：f64 的指數範圍高達 ±308 位十進位，遠超 i128 的 38 位。
/// 這兩者是**互補**而非替代 —— i128 提供精度，f64 提供動態範圍。
#[test]
fn interval_rescues_i128_overflow_case() {
    let k = 1_000_000_000i128;
    let d1 = dist2_ps((k / 3, k / 7), (0, 0), (k, k + 1));
    let d2 = dist2_ps((k / 5, k / 11), (0, 0), (k + 1, k));

    // i128 路徑確實會溢位
    assert!(ratcmp_will_overflow(d1, d2), "i128 交叉相乘需 176 bit");
    // 未檢查版靜默給出錯誤答案
    assert_ne!(Rat::cmp(d1, d2), 1, "已知此處 i128 會出錯");
    // 但區間濾波正確解決（真值 = 1，Python 任意精度驗證）
    assert_eq!(
        ratcmp_interval(d1, d2),
        Some(1),
        "區間濾波應靠 f64 的大指數範圍救回"
    );
    assert_eq!(ratcmp_adaptive(d1, d2), Some(1), "自適應版應給出正確答案");

    // 小座標同樣正常
    let k = 1_000i128;
    let e1 = dist2_ps((k / 3, k / 7), (0, 0), (k, k + 1));
    let e2 = dist2_ps((k / 5, k / 11), (0, 0), (k + 1, k));
    assert!(ratcmp_adaptive(e1, e2).is_some());
}

/// 構造區間濾波**也無法**解決的案例：兩個極接近的大有理數。
#[test]
fn ratcmp_adaptive_declines_when_truly_ambiguous() {
    // 分子僅差 1，但量級遠超 f64 精度 → 區間必然重疊
    let a = Rat { num: (1i128 << 100) + 1, den: 3 };
    let b = Rat { num: 1i128 << 100, den: 3 };
    assert_eq!(ratcmp_interval(a, b), None, "區間應無法分辨");
    // 且 i128 交叉相乘也會溢位 → 必須拒絕
    if ratcmp_will_overflow(a, b) {
        assert_eq!(ratcmp_adaptive(a, b), None, "兩條路都不通時應拒絕");
    }
}

// ---- 統計 ------------------------------------------------------------------

#[test]
fn filter_stats_accounting() {
    let mut st = FilterStats::default();
    assert_eq!(st.total(), 0);
    assert_eq!(st.success_rate(), 0.0);
    orient2d_fast_stat((0, 0), (1, 0), (0, 1), &mut st); // 非退化 → 濾波成功
    orient2d_fast_stat((0, 0), (1, 1), (2, 2), &mut st); // 共線 → 回退
    assert_eq!(st.total(), 2);
    assert_eq!(st.filtered, 1);
    assert_eq!(st.fallback, 1);
    assert!((st.success_rate() - 0.5).abs() < 1e-12);
}

// ---- f64 輸入路徑 ----------------------------------------------------------

#[test]
fn f64_path_correct() {
    let mut s: u64 = 555;
    let mut next = || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        s >> 33
    };
    for _ in 0..100_000 {
        let g = |k: &mut dyn FnMut() -> u64| {
            ((k() % 2001) as i128 - 1000, (k() % 2001) as i128 - 1000)
        };
        let (a, b, c) = (g(&mut next), g(&mut next), g(&mut next));
        let af = (a.0 as f64, a.1 as f64);
        let bf = (b.0 as f64, b.1 as f64);
        let cf = (c.0 as f64, c.1 as f64);
        assert_eq!(
            orient2d_f64(af, bf, cf, 1.0),
            P::orient2d(a, b, c),
            "f64 路徑結果不符"
        );
    }
}
