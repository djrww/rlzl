//! 正確性測試：所有結果都是 `const`，即在**編譯期**由宏 + const-eval 算出。
//! 測試本體只做斷言比對。

use welzl_macro::exact::*;
use welzl_macro::welzl;
use welzl_macro::welzl::*;

// ---- 基底情形（宏層直接分派） --------------------------------------------

const C0: Circle = welzl!();
const C1: Circle = welzl!((3, 4));
const C2: Circle = welzl!((0, 0), (4, 0));
const C3: Circle = welzl!((0, 0), (4, 0), (0, 3));

#[test]
fn base_cases() {
    assert!(C0.is_empty());
    assert_eq!(C1, Circle { cx: 3, cy: 4, cd: 1, r2: 0 });
    // 直徑 4 → 圓心 (2,0)，r² = 4
    assert_eq!(C2.center_f64(), (2.0, 0.0));
    assert!((C2.radius_f64() - 2.0).abs() < 1e-12);
    // 直角三角形 → 外接圓以斜邊為直徑，圓心 (2, 1.5)，r = 2.5
    assert_eq!(C3.center_f64(), (2.0, 1.5));
    assert!((C3.radius_f64() - 2.5).abs() < 1e-12);
}

// ---- 精確性：結果是有理數，沒有任何浮點誤差 -------------------------------

#[test]
fn exact_rational() {
    // 圓心 (2, 3/2) 應以 cd = 2 精確表示，而非浮點近似
    assert_eq!((C3.cx, C3.cy, C3.cd), (4, 3, 2));
    // r² = 25/4
    assert_eq!((C3.r2, C3.cd * C3.cd), (25, 4));
}

const R2: (i128, i128) = welzl!(@radius2 (0, 0), (4, 0), (0, 3));

#[test]
fn radius2_form() {
    assert_eq!(R2, (25, 4));
}

// ---- 一般情形 n ≥ 4，三種實作互相比對 -------------------------------------

const PTS: [Pt; 8] = [
    (0, 0), (10, 0), (10, 10), (0, 10),
    (5, 5), (3, 7), (8, 2), (1, 9),
];

const IT: Circle = welzl_arr(PTS);
const RE: Circle = welzl_rec_arr(PTS);
const BR: Circle = brute(&PTS);

#[test]
fn three_impls_agree() {
    // 正方形 → 圓心 (5,5)，r² = 50
    assert_eq!(IT.center_f64(), (5.0, 5.0));
    assert_eq!(cmp_radius(IT, BR), 0, "迭代版 vs 暴力法");
    assert_eq!(cmp_radius(RE, BR), 0, "遞迴版 vs 暴力法");
    assert!(covers(IT, &PTS));
    assert!(covers(RE, &PTS));
}

// ---- 洗牌不變性：Welzl 的隨機化不影響最優解 -------------------------------

const S1: Circle = welzl!(@seed 1; (0,0),(10,0),(10,10),(0,10),(5,5),(2,8));
const S2: Circle = welzl!(@seed 99999; (0,0),(10,0),(10,10),(0,10),(5,5),(2,8));
const S3: Circle = welzl!(@seed 0xABCDEF; (0,0),(10,0),(10,10),(0,10),(5,5),(2,8));

#[test]
fn shuffle_invariance() {
    assert_eq!(S1, S2);
    assert_eq!(S2, S3);
}

// ---- 退化情形：共線 --------------------------------------------------------

const COL: Circle = welzl!((0, 0), (1, 1), (2, 2), (3, 3), (4, 4));

#[test]
fn collinear() {
    // 兩端點 (0,0),(4,4) 為直徑 → 圓心 (2,2)
    assert_eq!(COL.center_f64(), (2.0, 2.0));
    assert!(covers(COL, &[(0, 0), (1, 1), (2, 2), (3, 3), (4, 4)]));
}

const DUP: Circle = welzl!((7, 7), (7, 7), (7, 7), (7, 7));

#[test]
fn duplicates() {
    assert_eq!(DUP.r2, 0);
    assert_eq!(DUP.center_f64(), (7.0, 7.0));
}

// ---- 大座標：i128 精確算術不溢位 ------------------------------------------

const BIG: Circle = welzl!(@checked (-1000000, -1000000), (1000000, 1000000), (0, 999999));

#[test]
fn big_coords() {
    assert_eq!(BIG.center_f64(), (0.0, 0.0));
    assert!(covers(BIG, &[(-1000000, -1000000), (1000000, 1000000), (0, 999999)]));
}

// ---- 隨機點集：宏生成 + 與暴力法比對 --------------------------------------

const RND: [Pt; 40] = gen_pts::<40>(0xC0FFEE);
const RND_W: Circle = welzl_arr(RND);
const RND_B: Circle = brute(&RND);

#[test]
fn random_vs_brute() {
    assert_eq!(cmp_radius(RND_W, RND_B), 0);
    assert!(covers(RND_W, &RND));
}

/// 執行期大量隨機比對，確認 Welzl 對任意輸入都給出最優解。
#[test]
fn randomized_stress() {
    let mut s: u64 = 12345;
    let mut next = || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        s >> 33
    };
    for _ in 0..300 {
        let n = (next() % 12 + 1) as usize;
        let pts: Vec<Pt> = (0..n)
            .map(|_| ((next() % 41) as i128 - 20, (next() % 41) as i128 - 20))
            .collect();
        let w = welzl_slice(&shuffle_vec(&pts, next()));
        let b = brute(&pts);
        assert!(covers(w, &pts), "未覆蓋全部點: {pts:?}");
        assert_eq!(cmp_radius(w, b), 0, "非最小圓: {pts:?} w={w:?} b={b:?}");
    }
}

fn shuffle_vec(pts: &[Pt], seed: u64) -> Vec<Pt> {
    let mut a = pts.to_vec();
    let mut s = seed | 1;
    for i in (1..a.len()).rev() {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        a.swap(i, ((s >> 33) as usize) % (i + 1));
    }
    a
}

// ---- 宏自身的 token 層遞迴 -------------------------------------------------

const CNT: usize = welzl_macro::welzl_count!((0,0) (1,1) (2,2) (3,3) (4,4));

#[test]
fn macro_counting() {
    assert_eq!(CNT, 5);
}

// ---- 自我生成的宏塔 --------------------------------------------------------

welzl_macro::welzl_tower!(TOWER_A, 4, + + +);

#[test]
fn tower_expands() {
    // base=4, 3 層 → 4 << 3 = 32 個點
    assert!(!TOWER_A.is_empty());
    assert!(TOWER_A.radius_f64() > 0.0);
}
