//! Welzl 最小包覆圓（Minimum Enclosing Circle）—— 全部在 `const fn` 中完成。
//!
//! 這裡給出兩種形式，兩者都是 Welzl (1991) 的算法：
//!
//! * [`welzl_rec`]  —— 教科書遞迴形式 `welzl(P, R)`，遞迴深度 = |P|。
//! * [`welzl_arr`]  —— 等價的三層增量迴圈形式，無遞迴，可在 const-eval
//!   中撐到遠比遞迴版更大的 N（遞迴版會先撞上 const-eval 的堆疊深度）。
//!
//! 兩者都先做一次 **編譯期 Fisher–Yates 洗牌**（LCG 偽隨機，種子為常數，
//! 因此結果 100% 可重現）。洗牌正是 Welzl 隨機化的來源：期望 O(n)，
//! 沒有洗牌時對抗性輸入會退化成 O(n³)。

use crate::exact::*;

/// |R| ≤ 3 的平凡子問題。
pub const fn trivial(r: [Pt; 3], rn: usize) -> Circle {
    match rn {
        0 => EMPTY,
        1 => circ1(r[0]),
        2 => circ2(r[0], r[1]),
        _ => circ3(r[0], r[1], r[2]),
    }
}

// ---------------------------------------------------------------------------
// 遞迴形式：welzl(P, R)
// ---------------------------------------------------------------------------

/// 教科書遞迴 Welzl：`P = pts[0..n]`，`R = r[0..rn]` 為強制落在邊界上的點。
///
/// ```text
/// welzl(P, R):
///     if P = ∅ or |R| = 3: return trivial(R)
///     p := P 中一點
///     D := welzl(P − p, R)
///     if p ∈ D: return D
///     return welzl(P − p, R ∪ {p})
/// ```
pub const fn welzl_rec(pts: &[Pt], n: usize, r: [Pt; 3], rn: usize) -> Circle {
    if n == 0 || rn == 3 {
        return trivial(r, rn);
    }
    let p = pts[n - 1];
    let d = welzl_rec(pts, n - 1, r, rn);
    if in_circle(d, p) {
        return d;
    }
    let mut r2 = r;
    r2[rn] = p;
    welzl_rec(pts, n - 1, r2, rn + 1)
}

/// 遞迴版入口（含編譯期洗牌）。
pub const fn welzl_rec_arr<const N: usize>(pts: [Pt; N]) -> Circle {
    let a = shuffle(pts, DEFAULT_SEED);
    welzl_rec(&a, N, [(0, 0), (0, 0), (0, 0)], 0)
}

// ---------------------------------------------------------------------------
// 迭代形式：三層增量迴圈（Welzl 的 move-to-front / 增量展開）
// ---------------------------------------------------------------------------

/// 迭代 Welzl。與遞迴版數學上等價，逐點增量維護當前最小圓。
pub const fn welzl_slice(pts: &[Pt]) -> Circle {
    let n = pts.len();
    let mut c = EMPTY;
    let mut i = 0;
    while i < n {
        if !in_circle(c, pts[i]) {
            // pts[i] 必在最終圓的邊界上
            c = circ1(pts[i]);
            let mut j = 0;
            while j < i {
                if !in_circle(c, pts[j]) {
                    // pts[i], pts[j] 皆在邊界上
                    c = circ2(pts[i], pts[j]);
                    let mut k = 0;
                    while k < j {
                        if !in_circle(c, pts[k]) {
                            c = circ3(pts[i], pts[j], pts[k]);
                        }
                        k += 1;
                    }
                }
                j += 1;
            }
        }
        i += 1;
    }
    c
}

/// 迭代版入口（含編譯期洗牌）—— `welzl!` 宏預設走這條路。
pub const fn welzl_arr<const N: usize>(pts: [Pt; N]) -> Circle {
    let a = shuffle(pts, DEFAULT_SEED);
    welzl_slice(&a)
}

// ---------------------------------------------------------------------------
// 編譯期偽隨機
// ---------------------------------------------------------------------------

pub const DEFAULT_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

const fn lcg(s: u64) -> u64 {
    s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)
}

/// 編譯期 Fisher–Yates 洗牌：Welzl 的隨機化步驟。
pub const fn shuffle<const N: usize>(pts: [Pt; N], seed: u64) -> [Pt; N] {
    let mut a = pts;
    let mut s = seed ^ (N as u64).wrapping_mul(0x517C_C1B7_2722_0A95);
    let mut i = N;
    while i > 1 {
        i -= 1;
        s = lcg(s);
        let j = ((s >> 33) as usize) % (i + 1);
        let t = a[i];
        a[i] = a[j];
        a[j] = t;
    }
    a
}

/// 編譯期生成 N 個偽隨機點（給自我生成的宏塔使用）。
pub const fn gen_pts<const N: usize>(seed: u64) -> [Pt; N] {
    let mut out = [(0i128, 0i128); N];
    let mut s = seed | 1;
    let mut i = 0;
    while i < N {
        s = lcg(s);
        let x = ((s >> 33) % 2001) as i128 - 1000;
        s = lcg(s);
        let y = ((s >> 33) % 2001) as i128 - 1000;
        out[i] = (x, y);
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// 驗證用：暴力法（O(n³)），供測試比對
// ---------------------------------------------------------------------------

/// 暴力枚舉所有 1/2/3 點圓，取覆蓋全體的最小者。用於測試正確性。
pub const fn brute(pts: &[Pt]) -> Circle {
    let n = pts.len();
    let mut best = EMPTY;
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j < n {
            let mut k = j;
            while k < n {
                let c = if i == j && j == k {
                    circ1(pts[i])
                } else if i == j || j == k || i == k {
                    let (a, b) = if i == j { (i, k) } else { (i, j) };
                    circ2(pts[a], pts[b])
                } else {
                    circ3(pts[i], pts[j], pts[k])
                };
                if !c.is_empty() && covers(c, pts) && (best.is_empty() || cmp_radius(c, best) < 0) {
                    best = c;
                }
                k += 1;
            }
            j += 1;
        }
        i += 1;
    }
    best
}

/// 精確檢查圓是否覆蓋全部點。
pub const fn covers(c: Circle, pts: &[Pt]) -> bool {
    let mut i = 0;
    while i < pts.len() {
        if !in_circle(c, pts[i]) {
            return false;
        }
        i += 1;
    }
    true
}
