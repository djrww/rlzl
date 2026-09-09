//! 宏層：`macro_rules!` 遞迴展開 + `const fn` 編譯期求值。
//!
//! 分工原則：
//!
//! * **宏** 負責 *token 層* 的工作 —— 解析點列、遞迴計數、分派 |R| ≤ 3 的
//!   基底情形、以及自我生成更深的宏調用。這一層的天花板是 `recursion_limit`。
//! * **`const fn`** 負責 *數值層* 的工作 —— i128 精確算術與 Welzl 主迴圈。
//!   這一層的天花板是 const-eval 的 step limit 與堆疊深度。
//!
//! 兩層各有各的極限，`welzl_tower!` 會把兩者一起推到爆。

// ---------------------------------------------------------------------------
// 1. 純宏遞迴：計數（Peano 風格 tt-munching）
// ---------------------------------------------------------------------------

/// 用宏遞迴數出 token 群組個數。每個點消耗一層宏遞迴，
/// 因此點數直接受 `recursion_limit` 約束。
#[macro_export]
macro_rules! welzl_count {
    () => { 0usize };
    (($($_t:tt)*) $($rest:tt)*) => { 1usize + $crate::welzl_count!($($rest)*) };
}

// ---------------------------------------------------------------------------
// 2. 主宏：welzl!
// ---------------------------------------------------------------------------

/// 在**編譯期**求出點集的最小包覆圓（Welzl 算法，i128 精確算術）。
///
/// ```ignore
/// const C: Circle = welzl!((0,0), (4,0), (0,3));
/// ```
///
/// 支援的形式：
///
/// * `welzl!((x,y), ...)`            —— 迭代 Welzl（預設，可撐最大 N）
/// * `welzl!(@rec (x,y), ...)`       —— 教科書遞迴 Welzl `welzl(P,R)`（120 點觸頂）
/// * `welzl!(@auto (x,y), ...)`      —— 自適應遞迴：觸頂自帶遞增後返回演算法
/// * `welzl!(@trace (x,y), ...)`     —— 同上，附 fuel/gens/depth/rounds 遙測
/// * `welzl!(@fuel F; (x,y), ...)`   —— 指定燃料單輪求解，觀察切換點
/// * `welzl!(@brute (x,y), ...)`     —— O(n³) 暴力法，驗證用
/// * `welzl!(@seed S; (x,y), ...)`   —— 指定洗牌種子
/// * `welzl!(@checked (x,y), ...)`   —— 額外做編譯期座標範圍斷言
/// * `welzl!(@radius2 (x,y), ...)`   —— 只取半徑²（有理數 tuple）
/// * `welzl!(@random N, seed)`       —— 編譯期生成 N 個隨機點再求解
#[macro_export]
macro_rules! welzl {
    // ---- 基底情形：宏層直接分派，不進主迴圈 -------------------------------
    () => { $crate::exact::EMPTY };
    (($x:expr, $y:expr)) => {
        $crate::exact::circ1(($x as i128, $y as i128))
    };
    (($x1:expr, $y1:expr), ($x2:expr, $y2:expr)) => {
        $crate::exact::circ2(($x1 as i128, $y1 as i128), ($x2 as i128, $y2 as i128))
    };
    (($x1:expr, $y1:expr), ($x2:expr, $y2:expr), ($x3:expr, $y3:expr)) => {
        $crate::exact::circ3(
            ($x1 as i128, $y1 as i128),
            ($x2 as i128, $y2 as i128),
            ($x3 as i128, $y3 as i128),
        )
    };

    // ---- 一般情形：n ≥ 4 ---------------------------------------------------
    ($(($x:expr, $y:expr)),+ $(,)?) => {
        $crate::welzl::welzl_arr([ $(($x as i128, $y as i128)),+ ])
    };

    // ---- 遞迴形式 welzl(P, R) ---------------------------------------------
    (@rec $(($x:expr, $y:expr)),+ $(,)?) => {
        $crate::welzl::welzl_rec_arr([ $(($x as i128, $y as i128)),+ ])
    };

    // ---- 自適應遞迴：觸頂自帶遞增後返回演算法 ------------------------------
    (@auto $(($x:expr, $y:expr)),+ $(,)?) => {
        $crate::adaptive::welzl_auto([ $(($x as i128, $y as i128)),+ ])
    };

    // ---- 自適應 + 完整遙測（fuel / gens / depth / rounds） ------------------
    (@trace $(($x:expr, $y:expr)),+ $(,)?) => {
        $crate::adaptive::welzl_escalate([ $(($x as i128, $y as i128)),+ ])
    };

    // ---- 指定燃料的單輪求解（觀察切換點） ----------------------------------
    (@fuel $f:expr; $(($x:expr, $y:expr)),+ $(,)?) => {
        $crate::adaptive::welzl_fueled([ $(($x as i128, $y as i128)),+ ], $f)
    };

    // ---- 暴力驗證 ----------------------------------------------------------
    (@brute $(($x:expr, $y:expr)),+ $(,)?) => {
        $crate::welzl::brute(&[ $(($x as i128, $y as i128)),+ ])
    };

    // ---- 指定種子 ----------------------------------------------------------
    (@seed $s:expr; $(($x:expr, $y:expr)),+ $(,)?) => {
        $crate::welzl::welzl_slice(
            &$crate::welzl::shuffle([ $(($x as i128, $y as i128)),+ ], $s)
        )
    };

    // ---- 帶座標範圍斷言（超界則編譯失敗） ---------------------------------
    (@checked $(($x:expr, $y:expr)),+ $(,)?) => {
        $crate::welzl::welzl_arr([
            $($crate::exact::assert_bound(($x as i128, $y as i128))),+
        ])
    };

    // ---- 只要半徑²（精確有理數 numer/denom） ------------------------------
    (@radius2 $($t:tt)*) => {{
        const __C: $crate::exact::Circle = $crate::welzl!($($t)*);
        (__C.r2, __C.cd * __C.cd)
    }};

    // ---- 編譯期生成隨機點 --------------------------------------------------
    (@random $n:expr, $seed:expr) => {
        $crate::welzl::welzl_arr($crate::welzl::gen_pts::<{ $n }>($seed))
    };
}

// ---------------------------------------------------------------------------
// 3. 自我生成：宏塔（macro tower）
// ---------------------------------------------------------------------------

/// **自我複製宏**：每展開一層，就生成「下一層自己」＋ 一個 `const` 求解項。
///
/// 深度以 unary token（`+`）編碼；每個 `+` 消耗一層宏遞迴，並讓問題規模
/// 依 `base * 2^level` 成長。也就是說：
///
/// * 宏遞迴深度 ∝ 層數 → 撞 `recursion_limit`
/// * const-eval 工作量 ∝ 2^層數 → 撞 const-eval step limit
///
/// 兩個極限誰先到，取決於 `base` 與層數的配比。
///
/// ```ignore
/// welzl_tower!(TOWER, 8, + + + +);  // 4 層：8, 16, 32, 64 個點
/// ```
#[macro_export]
macro_rules! welzl_tower {
    // 終止：層數用盡，生成「這一層」的解
    ($name:ident, $base:expr, ) => {
        $crate::paste_level!($name, 0, $base);
    };
    // 遞迴：先展開下一層（自我生成），再生成本層
    ($name:ident, $base:expr, + $($rest:tt)*) => {
        $crate::welzl_tower_inner!($name, $base, 1, $($rest)*);
    };
}

/// 宏塔內層：`$lvl` 為已累積層數（用 `expr` 攜帶，避免 ident 拼接依賴）。
#[macro_export]
macro_rules! welzl_tower_inner {
    ($name:ident, $base:expr, $lvl:expr, ) => {
        $crate::paste_level!($name, $lvl, $base);
    };
    ($name:ident, $base:expr, $lvl:expr, + $($rest:tt)*) => {
        $crate::welzl_tower_inner!($name, $base, $lvl + 1, $($rest)*);
    };
}

/// 生成單一層的 `const`：規模 `base << lvl`，種子隨層數變化。
#[macro_export]
macro_rules! paste_level {
    ($name:ident, $lvl:expr, $base:expr) => {
        pub const $name: $crate::exact::Circle =
            $crate::welzl::welzl_arr($crate::welzl::gen_pts::<{ $base << ($lvl) }>(
                0x1234_5678_9ABC_DEF0u64 ^ (($lvl) as u64),
            ));
    };
}

/// **宏遞迴深度壓力測試**：純 token 層遞迴，不做任何數值運算。
///
/// 用來單獨量測 `recursion_limit`，把宏層極限與 const-eval 極限分離開來。
#[macro_export]
macro_rules! welzl_depth {
    (0) => { 0usize };
    ($n:expr) => { 1usize + $crate::welzl_depth!($n - 1) };
}

/// 生成 `n` 個 `(i, i*i % 997)` 形式的點並求解 —— 讓宏在 token 層
/// 真的吐出 n 個點字面量（而非交給 `const fn` 生成），以壓測 token 展開量。
#[macro_export]
macro_rules! welzl_grid {
    ($n:expr) => {
        $crate::welzl::welzl_arr($crate::welzl::gen_pts::<{ $n }>(0xDEAD_BEEF))
    };
}
