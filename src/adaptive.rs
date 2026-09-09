//! 自適應 Welzl：觸及 const-eval 遞迴上限時，**自帶遞增後返回演算法**。
//!
//! ## 問題
//!
//! `welzl_rec` 的遞迴深度 = |P|，在 const-eval 中 121 點就撞 `E0080:
//! reached the configured maximum number of stack frames` —— 而且這是**編譯失敗**，
//! 不是可捕捉的錯誤。const-eval 沒有 catch_unwind，程式無法在觸頂後「接住自己」。
//!
//! ## 解法：燃料 + 續傳（fuel + resume）
//!
//! 既然無法事後捕捉，就**事前預測**。遞迴攜帶一份燃料 `fuel`：
//!
//! ```text
//!   welzl(P, R, fuel):
//!       if P = ∅ or |R| = 3:  return trivial(R)
//!       if fuel = 0:          ← 觸頂前一步
//!           generation += 1                      ← 自帶遞增
//!           return resume(P, R)                  ← 返回演算法（迭代續解）
//!       p := P 中一點
//!       D := welzl(P − p, R, fuel − 1)
//!       if p ∈ D: return D
//!       return welzl(P − p, R ∪ {p}, fuel − 1)
//! ```
//!
//! 關鍵在 `resume(P, R)` —— 它不是近似、不是放棄，而是**同一個問題的迭代解法**：
//! 「以 R 為強制邊界點，求 P 的最小包覆圓」。遞迴版與迭代版求的是同一個數學物件
//! `MEC(P | R ⊂ ∂D)`，所以在任何深度切換都得到**完全相同的答案**。
//!
//! 於是遞迴深度被鉗在 `fuel`，而 |P| 不再受限：
//! 遞迴負責頂部 `fuel` 層（保留 Welzl 的隨機化分治結構），
//! 迭代接手剩餘部分（不吃堆疊）。**120 點的天花板消失了。**
//!
//! ## 自動遞增（escalation）
//!
//! [`welzl_escalate`] 從小燃料起跳，只要發生過續傳就把燃料加倍，
//! 直到「不再需要續傳」（純遞迴跑完）或抵達安全上限 [`MAX_SAFE_FUEL`]。
//! 因為切換點不影響結果，每一輪的圓都相同 —— 這個不變量本身就是正確性證明，
//! 而且可以寫成編譯期斷言。

use crate::exact::*;
use crate::welzl::trivial;

/// const-eval 實測可用的最大遞迴深度（121 點即 `E0080`），留安全邊際。
///
/// 這個值刻意設得保守：`rec_fuel` 每層的 frame 比 `welzl_rec` 略大
/// （多了 `Partial` 的回傳與 fuel/depth 參數）。
pub const MAX_SAFE_FUEL: u32 = 64;

/// 預設起跳燃料。
pub const DEFAULT_FUEL: u32 = 16;

// ---------------------------------------------------------------------------
// 續傳核心：以 R 為強制邊界點，迭代求 MEC(P | R ⊂ ∂D)
// ---------------------------------------------------------------------------
//
// 這三個函式正是迭代 Welzl 三層迴圈的拆解版。把它們獨立出來，
// 遞迴才能在**任意 |R| 狀態**下把棒子交給迭代版。

/// |R| = 2：q1、q2 已在邊界上，求覆蓋 `pts[0..n]` 的最小圓。
pub const fn mec_with_2(pts: &[Pt], n: usize, q1: Pt, q2: Pt) -> Circle {
    let mut c = circ2(q1, q2);
    let mut k = 0;
    while k < n {
        if !in_circle(c, pts[k]) {
            // 三點確定唯一圓，無需再遞迴
            c = circ3(q1, q2, pts[k]);
        }
        k += 1;
    }
    c
}

/// |R| = 1：q 已在邊界上，求覆蓋 `pts[0..n]` 的最小圓。
pub const fn mec_with_1(pts: &[Pt], n: usize, q: Pt) -> Circle {
    if n == 0 {
        return circ1(q);
    }
    let mut c = circ2(pts[0], q);
    let mut j = 1;
    while j < n {
        if !in_circle(c, pts[j]) {
            // pts[j] 必在邊界上 → 降到 |R| = 2
            c = mec_with_2(pts, j, pts[j], q);
        }
        j += 1;
    }
    c
}

/// |R| = 0：求覆蓋 `pts[0..n]` 的最小圓（標準增量 Welzl）。
pub const fn mec_with_0(pts: &[Pt], n: usize) -> Circle {
    let mut c = EMPTY;
    let mut i = 0;
    while i < n {
        if !in_circle(c, pts[i]) {
            // pts[i] 必在邊界上 → 降到 |R| = 1
            c = mec_with_1(pts, i, pts[i]);
        }
        i += 1;
    }
    c
}

/// **續傳入口**：遞迴觸頂時呼叫，以迭代方式解完 `MEC(pts[0..n] | R ⊂ ∂D)`。
///
/// 完全不消耗遞迴深度（三層 `while`，const-eval frame 為常數）。
pub const fn resume(pts: &[Pt], n: usize, r: [Pt; 3], rn: usize) -> Circle {
    match rn {
        0 => mec_with_0(pts, n),
        1 => mec_with_1(pts, n, r[0]),
        2 => mec_with_2(pts, n, r[0], r[1]),
        _ => circ3(r[0], r[1], r[2]),
    }
}

// ---------------------------------------------------------------------------
// 帶燃料的遞迴
// ---------------------------------------------------------------------------

/// 遞迴的中間結果 —— 除了圓，還帶回「觸頂了幾次、最深走到哪」。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Partial {
    pub c: Circle,
    /// 續傳世代數：遞迴觸頂並移交給迭代的次數。0 = 全程純遞迴。
    pub gens: u32,
    /// 實際到達的最大遞迴深度。
    pub depth: u32,
}

const fn umax(a: u32, b: u32) -> u32 {
    if a > b { a } else { b }
}

/// 燃料版 Welzl 遞迴：深度恆 ≤ `fuel`，永不觸發 `E0080`。
pub const fn rec_fuel(
    pts: &[Pt],
    n: usize,
    r: [Pt; 3],
    rn: usize,
    fuel: u32,
    depth: u32,
) -> Partial {
    if n == 0 || rn == 3 {
        return Partial { c: trivial(r, rn), gens: 0, depth };
    }
    if fuel == 0 {
        // ── 觸頂：自帶遞增，返回演算法 ──────────────────────────────
        return Partial { c: resume(pts, n, r, rn), gens: 1, depth };
    }
    let p = pts[n - 1];
    let d = rec_fuel(pts, n - 1, r, rn, fuel - 1, depth + 1);
    if in_circle(d.c, p) {
        return d;
    }
    let mut r2 = r;
    r2[rn] = p;
    let e = rec_fuel(pts, n - 1, r2, rn + 1, fuel - 1, depth + 1);
    Partial {
        c: e.c,
        gens: d.gens + e.gens,
        depth: umax(d.depth, e.depth),
    }
}

// ---------------------------------------------------------------------------
// 自動遞增驅動
// ---------------------------------------------------------------------------

/// 完整解與遙測。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Solution {
    pub circle: Circle,
    /// 最終採用的燃料額度。
    pub fuel: u32,
    /// 最終一輪的續傳世代數（0 = 純遞迴跑完，未觸頂）。
    pub gens: u32,
    /// 最終一輪實際遞迴深度。
    pub depth: u32,
    /// 燃料遞增了幾輪。
    pub rounds: u32,
    /// 是否曾經觸頂並續傳。
    pub resumed: bool,
}

/// **自動遞增求解**：燃料不足就加倍重試，直到不再需要續傳或抵達安全上限。
///
/// 不論在哪一輪停下，`circle` 都是同一個精確解 —— 續傳點的位置不影響結果。
pub const fn welzl_adaptive(pts: &[Pt], start_fuel: u32, max_fuel: u32) -> Solution {
    let mut fuel = if start_fuel == 0 { 1 } else { start_fuel };
    let mut rounds = 1;
    let mut resumed = false;
    loop {
        let p = rec_fuel(pts, pts.len(), [(0, 0), (0, 0), (0, 0)], 0, fuel, 0);
        if p.gens > 0 {
            resumed = true;
        }
        // 收斂條件：不再觸頂，或燃料已到頂（再加倍就會撞 E0080）
        if p.gens == 0 || fuel * 2 > max_fuel {
            return Solution {
                circle: p.c,
                fuel,
                gens: p.gens,
                depth: p.depth,
                rounds,
                resumed,
            };
        }
        fuel *= 2; // ← 遞增
        rounds += 1;
    }
}

/// 陣列版入口（含編譯期洗牌），使用預設燃料策略。
pub const fn welzl_escalate<const N: usize>(pts: [Pt; N]) -> Solution {
    let a = crate::welzl::shuffle(pts, crate::welzl::DEFAULT_SEED);
    welzl_adaptive(&a, DEFAULT_FUEL, MAX_SAFE_FUEL)
}

/// 只要圓（丟棄遙測），供 `welzl!(@auto ...)` 使用。
pub const fn welzl_auto<const N: usize>(pts: [Pt; N]) -> Circle {
    welzl_escalate(pts).circle
}

/// 指定燃料的單輪求解（不自動遞增），供觀察切換行為。
pub const fn welzl_fueled<const N: usize>(pts: [Pt; N], fuel: u32) -> Solution {
    let a = crate::welzl::shuffle(pts, crate::welzl::DEFAULT_SEED);
    let p = rec_fuel(&a, N, [(0, 0), (0, 0), (0, 0)], 0, fuel, 0);
    Solution {
        circle: p.c,
        fuel,
        gens: p.gens,
        depth: p.depth,
        rounds: 1,
        resumed: p.gens > 0,
    }
}
