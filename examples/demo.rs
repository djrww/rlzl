//! Welzl-in-macro 展示：所有圓都在**編譯期**算完，執行期只是 println。
//!
//!   cargo run --example demo

#![allow(long_running_const_eval)]

use welzl_macro::exact::*;
use welzl_macro::welzl;
use welzl_macro::welzl::*;
use welzl_macro::adaptive::*;

fn show(label: &str, c: Circle) {
    if c.is_empty() {
        println!("{label:<34} ∅");
        return;
    }
    let (x, y) = c.center_f64();
    println!(
        "{label:<34} 圓心=({x:.6}, {y:.6})  r={:.6}   精確: ({}/{}, {}/{}) r²={}/{}",
        c.radius_f64(),
        c.cx, c.cd, c.cy, c.cd, c.r2, c.cd * c.cd
    );
}

// ── 1. 基底情形：宏在 token 層直接分派，不進主迴圈 ───────────────────────
const B0: Circle = welzl!();
const B1: Circle = welzl!((3, 4));
const B2: Circle = welzl!((0, 0), (4, 0));
const B3: Circle = welzl!((0, 0), (4, 0), (0, 3));

// ── 2. 一般情形 ───────────────────────────────────────────────────────────
const SQUARE: Circle = welzl!((0, 0), (10, 0), (10, 10), (0, 10), (5, 5), (3, 7), (8, 2));

// ── 3. 三種實作在編譯期互相驗證 ───────────────────────────────────────────
const P: [Pt; 9] = [(0,0),(9,1),(4,8),(2,3),(7,7),(1,6),(8,4),(5,0),(3,9)];
const V_ITER: Circle = welzl_arr(P);
const V_REC: Circle = welzl_rec_arr(P);
const V_BRUTE: Circle = brute(&P);
/// 編譯期斷言：三者必須完全一致，否則編譯失敗。
const _: () = assert!(cmp_radius(V_ITER, V_BRUTE) == 0);
const _: () = assert!(cmp_radius(V_REC, V_BRUTE) == 0);
const _: () = assert!(covers(V_ITER, &P));

// ── 4. 精確算術：無浮點誤差 ───────────────────────────────────────────────
const EXACT: (i128, i128) = welzl!(@radius2 (0, 0), (4, 0), (0, 3));
const _: () = assert!(EXACT.0 == 25 && EXACT.1 == 4); // r² 恰為 25/4

// ── 5. 洗牌不變性（Welzl 隨機化不改變最優解） ─────────────────────────────
const SH1: Circle = welzl!(@seed 1; (0,0),(10,0),(10,10),(0,10),(5,5),(2,8));
const SH2: Circle = welzl!(@seed 987654321; (0,0),(10,0),(10,10),(0,10),(5,5),(2,8));
const _: () = assert!(SH1.cx == SH2.cx && SH1.cy == SH2.cy && SH1.r2 == SH2.r2);

// ── 6. 大規模：10 萬點，全部在編譯期解完 ──────────────────────────────────
const BIG: Circle = welzl!(@random 100_000, 0xC0FFEE);

// ── 7. 自適應遞迴：觸頂自帶遞增後返回演算法 ───────────────────────────────
const A_SMALL: Solution = welzl!(@trace (0,0),(9,1),(4,8),(2,3),(7,7),(1,6),(8,4));
/// 500 點：純遞迴版必定 E0080 編譯失敗，自適應版通過
const A_BIG: Solution = welzl_escalate(gen_pts::<500>(0xC0FFEE));
const A_REF: Circle = welzl_arr(gen_pts::<500>(0xC0FFEE));
/// 編譯期鎖死不變量：切換點不影響結果
const _: () = assert!(cmp_radius(A_BIG.circle, A_REF) == 0);
/// 同一問題、四種切換點，解必須完全相同
const A_F1: Solution = welzl_fueled(gen_pts::<80>(0xBEEF), 1);
const A_F2: Solution = welzl_fueled(gen_pts::<80>(0xBEEF), 16);
const A_F3: Solution = welzl_fueled(gen_pts::<80>(0xBEEF), 48);
const _: () = assert!(cmp_radius(A_F1.circle, A_F2.circle) == 0);
const _: () = assert!(cmp_radius(A_F2.circle, A_F3.circle) == 0);

// ── 8. 自我生成的宏塔：每層自己生成下一層 ─────────────────────────────────
welzl_macro::welzl_tower!(TOWER, 64, + + + + + +); // 64<<6 = 4096 點

fn main() {
    println!("═══ 1. 基底情形（宏層直接分派）═══");
    show("welzl!()", B0);
    show("welzl!((3,4))", B1);
    show("welzl!((0,0),(4,0))", B2);
    show("welzl!((0,0),(4,0),(0,3))", B3);

    println!("\n═══ 2. 一般情形 ═══");
    show("7 點正方形+內點", SQUARE);

    println!("\n═══ 3. 三實作編譯期交叉驗證（9 點）═══");
    show("迭代 Welzl", V_ITER);
    show("遞迴 welzl(P,R)", V_REC);
    show("暴力 O(n³)", V_BRUTE);
    println!("{:<34} {}", "編譯期斷言三者相等", "✓ 已在編譯期通過");

    println!("\n═══ 4. 精確有理數算術 ═══");
    println!("{:<34} r² = {}/{}  (恰為 6.25，無浮點誤差)", "直角三角形", EXACT.0, EXACT.1);

    println!("\n═══ 5. 洗牌不變性 ═══");
    show("seed=1", SH1);
    show("seed=987654321", SH2);

    println!("\n═══ 6. 大規模編譯期求解 ═══");
    show("100,000 點（編譯期解完）", BIG);

    println!("\n═══ 7. 自適應遞迴（觸頂自帶遞增後返回演算法）═══");
    println!(
        "{:<34} fuel={} gens={} depth={} rounds={} resumed={}",
        "7 點（未觸頂，純遞迴）", A_SMALL.fuel, A_SMALL.gens, A_SMALL.depth,
        A_SMALL.rounds, A_SMALL.resumed
    );
    show("  └─ 解", A_SMALL.circle);
    println!(
        "{:<34} fuel={} gens={} depth={} rounds={} resumed={}",
        "500 點（觸頂→遞增→續傳）", A_BIG.fuel, A_BIG.gens, A_BIG.depth,
        A_BIG.rounds, A_BIG.resumed
    );
    show("  └─ 解", A_BIG.circle);
    println!("{:<34} {}", "  └─ 純遞迴版 @rec 於此規模", "E0080 編譯失敗（自適應版通過）");
    println!("\n  切換點不變性（80 點，強制不同交棒深度）:");
    for (l, s) in [("fuel=1", A_F1), ("fuel=16", A_F2), ("fuel=48", A_F3)] {
        println!(
            "    {l:<10} depth={:<3} gens={:<3} r²={}/{}",
            s.depth, s.gens, s.circle.r2, s.circle.cd * s.circle.cd
        );
    }
    println!("    → 三者 r² 完全相同，且已在編譯期 assert");

    println!("\n═══ 8. 自我生成宏塔 ═══");
    show("TOWER (64<<6 = 4096 點)", TOWER);

    println!("\n以上每一個圓都是 const —— 二進位檔裡只有結果，沒有任何 Welzl 執行期程式碼。");
}
