//! 自適應遞迴測試：觸頂 → 自帶遞增 → 返回演算法。
//!
//! 核心不變量：**續傳點的位置不影響結果**。
//! 不論在第幾層把棒子從遞迴交給迭代，得到的都是同一個精確解。

use welzl_macro::adaptive::*;
use welzl_macro::exact::*;
use welzl_macro::welzl;
use welzl_macro::welzl::*;

// ---- 1. 突破 120 點硬牆 ----------------------------------------------------
//
// 對照組：`welzl_rec_arr` 在 121 點就 E0080 編譯失敗。
// 這裡 500 點順利通過，因為遞迴深度被鉗在 fuel。

const P500: [Pt; 500] = gen_pts::<500>(0xC0FFEE);
const S500: Solution = welzl_escalate(P500);
const I500: Circle = welzl_arr(P500);

/// 編譯期斷言：自適應解 == 迭代解（若不等，編譯直接失敗）。
const _: () = assert!(cmp_radius(S500.circle, I500) == 0);
const _: () = assert!(covers(S500.circle, &P500));
/// 深度確實被鉗住，遠低於 500。
const _: () = assert!(S500.depth <= MAX_SAFE_FUEL);

#[test]
fn breaks_recursion_wall() {
    // 500 點：純遞迴版必定 E0080，自適應版通過
    assert!(S500.resumed, "500 點必然觸頂並續傳");
    assert!(S500.gens > 0, "應有續傳世代");
    assert!(S500.depth <= MAX_SAFE_FUEL, "深度須被鉗在安全值內");
    assert_eq!(cmp_radius(S500.circle, I500), 0, "自適應解必須等於迭代解");
}

// ---- 2. 切換點不變性：任意 fuel 都給出同一個解 -----------------------------

const P80: [Pt; 80] = gen_pts::<80>(0xBEEF);
const F1: Solution = welzl_fueled(P80, 1);
const F2: Solution = welzl_fueled(P80, 4);
const F3: Solution = welzl_fueled(P80, 16);
const F4: Solution = welzl_fueled(P80, 48);
const REF80: Circle = welzl_arr(P80);

// 編譯期就鎖死這個不變量
const _: () = assert!(cmp_radius(F1.circle, REF80) == 0);
const _: () = assert!(cmp_radius(F2.circle, REF80) == 0);
const _: () = assert!(cmp_radius(F3.circle, REF80) == 0);
const _: () = assert!(cmp_radius(F4.circle, REF80) == 0);

#[test]
fn switchover_invariance() {
    // fuel=1 幾乎立刻交棒，fuel=48 深入遞迴，結果必須完全相同
    for (label, s) in [("fuel=1", F1), ("fuel=4", F2), ("fuel=16", F3), ("fuel=48", F4)] {
        assert_eq!(cmp_radius(s.circle, REF80), 0, "{label} 解不一致");
        assert!(covers(s.circle, &P80), "{label} 未覆蓋全部點");
        assert!(s.depth <= s.fuel, "{label} 深度 {} 超過燃料 {}", s.depth, s.fuel);
    }
    // 燃料不足以吃下 80 點時，每一輪都必然發生續傳
    for (label, s) in [("fuel=1", F1), ("fuel=4", F2), ("fuel=16", F3), ("fuel=48", F4)] {
        assert!(s.resumed && s.gens >= 1, "{label} 應觸頂續傳");
    }
    // 深度恰好吃滿燃料 —— 證明遞迴確實走到底才交棒，而非提早放棄
    assert_eq!((F1.depth, F2.depth, F3.depth, F4.depth), (1, 4, 16, 48));
}

// ---- 3. 遞增行為：小問題不觸頂，大問題自動升級 -----------------------------

const SMALL: Solution = welzl_escalate(gen_pts::<10>(7));
const LARGE: Solution = welzl_escalate(gen_pts::<300>(7));

#[test]
fn escalation_behavior() {
    // 10 點 < DEFAULT_FUEL=16，純遞迴即可跑完，不需續傳
    assert!(!SMALL.resumed, "小問題不應觸頂");
    assert_eq!(SMALL.gens, 0, "小問題應純遞迴完成");
    assert_eq!(SMALL.rounds, 1, "小問題不需遞增");

    // 300 點遠超燃料，必然觸頂 → 自動遞增
    assert!(LARGE.resumed, "大問題必然觸頂");
    assert!(LARGE.fuel >= DEFAULT_FUEL, "燃料應已遞增");
    assert!(LARGE.rounds >= 1);
    assert!(LARGE.depth <= MAX_SAFE_FUEL, "遞增不得越過安全上限");
}

/// 燃料遞增必須停在安全上限內，永不觸發 E0080。
#[test]
fn escalation_respects_ceiling() {
    for n_seed in [1u64, 42, 999, 0xABCDEF] {
        let pts = gen_pts::<200>(n_seed);
        let s = welzl_adaptive(&pts, 1, MAX_SAFE_FUEL);
        assert!(s.fuel <= MAX_SAFE_FUEL, "燃料 {} 越界", s.fuel);
        assert!(s.depth <= MAX_SAFE_FUEL, "深度 {} 越界", s.depth);
    }
}

// ---- 4. 續傳核心：mec_with_k 在各種 |R| 狀態下都正確 -----------------------

#[test]
fn resume_correctness_all_states() {
    let mut s: u64 = 20240909;
    let mut next = || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        s >> 33
    };
    // 對每個 fuel 值，與暴力法比對 —— 涵蓋所有可能的 |R| 交棒狀態
    for _ in 0..200 {
        let n = (next() % 10 + 1) as usize;
        let pts: Vec<Pt> = (0..n)
            .map(|_| ((next() % 31) as i128 - 15, (next() % 31) as i128 - 15))
            .collect();
        let b = brute(&pts);
        for fuel in 0..=8u32 {
            let p = rec_fuel(&pts, pts.len(), [(0, 0), (0, 0), (0, 0)], 0, fuel, 0);
            assert!(covers(p.c, &pts), "fuel={fuel} 未覆蓋: {pts:?}");
            assert_eq!(
                cmp_radius(p.c, b),
                0,
                "fuel={fuel} 非最小圓: {pts:?} got={:?} want={b:?}",
                p.c
            );
            assert!(p.depth <= fuel, "fuel={fuel} 深度 {} 越界", p.depth);
        }
    }
}

// ---- 5. 宏介面 -------------------------------------------------------------

const M_AUTO: Circle = welzl!(@auto (0,0),(10,0),(10,10),(0,10),(5,5),(3,7),(8,2));
const M_TRACE: Solution = welzl!(@trace (0,0),(10,0),(10,10),(0,10),(5,5),(3,7),(8,2));
const M_FUEL: Solution = welzl!(@fuel 2; (0,0),(10,0),(10,10),(0,10),(5,5),(3,7),(8,2));
const M_ITER: Circle = welzl!((0,0),(10,0),(10,10),(0,10),(5,5),(3,7),(8,2));

#[test]
fn macro_arms() {
    // 正方形 → 圓心 (5,5), r² = 50
    assert_eq!(M_AUTO.center_f64(), (5.0, 5.0));
    assert_eq!(M_AUTO.r2, 50);
    assert_eq!(cmp_radius(M_AUTO, M_ITER), 0);
    assert_eq!(cmp_radius(M_TRACE.circle, M_ITER), 0);
    // fuel=2 強制早期交棒，結果仍相同
    assert_eq!(cmp_radius(M_FUEL.circle, M_ITER), 0);
    assert!(M_FUEL.resumed, "fuel=2 對 7 點必然觸頂");
}

// ---- 6. 退化情形在續傳路徑下仍正確 ----------------------------------------

const COL_AUTO: Solution = welzl_fueled([(0,0),(1,1),(2,2),(3,3),(4,4),(5,5)], 1);
const DUP_AUTO: Solution = welzl_fueled([(7,7),(7,7),(7,7),(7,7),(7,7)], 1);

#[test]
fn degenerate_via_resume() {
    // 共線，fuel=1 走續傳路徑
    assert_eq!(COL_AUTO.circle.center_f64(), (2.5, 2.5));
    assert!(covers(COL_AUTO.circle, &[(0,0),(1,1),(2,2),(3,3),(4,4),(5,5)]));
    // 重複點
    assert_eq!(DUP_AUTO.circle.r2, 0);
    assert_eq!(DUP_AUTO.circle.center_f64(), (7.0, 7.0));
}
