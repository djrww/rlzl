#!/usr/bin/env python3
"""極限探測器：對每個維度做指數擴張 + 二分搜尋，找出「還能編譯過」的最大值。

三個維度：
  macro_tt      —— 純宏 tt-munching 遞迴深度（recursion_limit）
  macro_tt_hi   —— 同上，但 #![recursion_limit] 拉高
  const_iter    —— 迭代 Welzl 的 const-eval 規模（step limit / 時間）
  const_rec     —— 遞迴 Welzl 的 const-eval 深度（const-eval 堆疊）
  tower         —— 自我生成宏塔的層數（宏遞迴 × 2^n 工作量）
"""
import subprocess, tempfile, time, os, sys, json

LIB = os.path.abspath("target/release/libwelzl_macro.rlib")
TMP = tempfile.mkdtemp(prefix="welzl_probe_")
TIMEOUT = 300


def compile_src(src, extra=None):
    """回傳 (ok, 秒數, 診斷訊息)。"""
    p = os.path.join(TMP, "p.rs")
    with open(p, "w") as f:
        f.write(src)
    cmd = ["rustc", "--edition", "2021", "--crate-type", "lib",
           "--extern", f"welzl_macro={LIB}",
           "--emit", "metadata", "-o", os.path.join(TMP, "p.meta"), p]
    if extra:
        cmd[1:1] = extra
    t0 = time.time()
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=TIMEOUT)
    except subprocess.TimeoutExpired:
        return False, TIMEOUT, "TIMEOUT"
    dt = time.time() - t0
    if r.returncode == 0:
        return True, dt, ""
    err = r.stderr
    for key in ("recursion limit", "exceeded interpreter step limit",
                "constant evaluation is taking a long time",
                "reached the configured maximum", "memory exhausted",
                "stack overflow", "error["):
        if key in err:
            line = next((l.strip() for l in err.splitlines() if key in l), key)
            return False, dt, line[:160]
    line = next((l.strip() for l in err.splitlines() if "error" in l), "unknown")
    return False, dt, line[:160]


# ---------------- 各維度的原始碼生成器 ----------------

def src_macro_tt(n, limit=None):
    hdr = f"#![recursion_limit=\"{limit}\"]\n" if limit else ""
    pts = " ".join(f"({i},{i})" for i in range(n))
    return hdr + f"const C: usize = welzl_macro::welzl_count!({pts});\n"


def src_macro_literal(n, limit=None):
    """宏在 token 層吐出 n 個點字面量，並實際走 Welzl。"""
    hdr = f"#![recursion_limit=\"{limit}\"]\n" if limit else ""
    pts = ", ".join(f"({(i*7919)%2001-1000},{(i*104729)%2001-1000})" for i in range(n))
    return hdr + f"const C: welzl_macro::Circle = welzl_macro::welzl!({pts});\n"


def src_const_iter(n):
    return (f"const C: welzl_macro::Circle = "
            f"welzl_macro::welzl::welzl_arr(welzl_macro::welzl::gen_pts::<{n}>(0xC0FFEE));\n")


def src_const_rec(n):
    return (f"const C: welzl_macro::Circle = "
            f"welzl_macro::welzl::welzl_rec_arr(welzl_macro::welzl::gen_pts::<{n}>(0xC0FFEE));\n")


def src_tower(levels, base=1, limit=None):
    hdr = f"#![recursion_limit=\"{limit}\"]\n" if limit else ""
    plus = " ".join("+" for _ in range(levels))
    return hdr + f"welzl_macro::welzl_tower!(T, {base}, {plus});\n"


# ---------------- 探測驅動 ----------------

def probe(name, gen, lo=1, cap=10**9, note=""):
    """指數擴張找到失敗點，再二分搜尋最大可行值。"""
    print(f"\n=== {name} {note} ===", flush=True)
    ok, last_t = lo, 0.0
    n, diag = lo, ""
    # 指數擴張
    while n <= cap:
        good, dt, d = compile_src(gen(n))
        print(f"  n={n:<9} {'OK ' if good else 'FAIL'} {dt:6.1f}s {d}", flush=True)
        if good:
            ok, last_t = n, dt
            n *= 2
        else:
            diag = d
            break
    else:
        return {"name": name, "max": ok, "time": last_t, "diag": "hit cap", "note": note}
    if n > cap:
        return {"name": name, "max": ok, "time": last_t, "diag": "hit cap", "note": note}
    # 二分
    hi = n
    while hi - ok > max(1, ok // 50):
        mid = (ok + hi) // 2
        good, dt, d = compile_src(gen(mid))
        print(f"  n={mid:<9} {'OK ' if good else 'FAIL'} {dt:6.1f}s {d}", flush=True)
        if good:
            ok, last_t = mid, dt
        else:
            hi, diag = mid, d
    print(f"  --> 上限 {ok}  ({diag})", flush=True)
    return {"name": name, "max": ok, "time": last_t, "diag": diag, "note": note}


if __name__ == "__main__":
    which = sys.argv[1] if len(sys.argv) > 1 else "all"
    res = []
    if which in ("all", "macro"):
        res.append(probe("macro_tt_default", src_macro_tt, cap=4096,
                         note="純 tt-munching，預設 recursion_limit=128"))
        res.append(probe("macro_tt_1M", lambda n: src_macro_tt(n, 1_000_000), cap=200_000,
                         note="recursion_limit 拉到 1e6"))
        res.append(probe("macro_literal", lambda n: src_macro_literal(n, 1_000_000), cap=200_000,
                         note="宏吐出 n 個點字面量 + 實跑 Welzl"))
    if which in ("all", "const"):
        res.append(probe("const_iter", src_const_iter, cap=4_000_000,
                         note="迭代 Welzl，const-eval 規模"))
        res.append(probe("const_rec", src_const_rec, cap=4_000_000,
                         note="遞迴 Welzl，const-eval 深度"))
    if which in ("all", "tower"):
        res.append(probe("tower_levels", lambda n: src_tower(n, 1, 1_000_000), cap=64,
                         note="自我生成宏塔層數，規模 2^level"))
    with open(f"probe_{which}.json", "w") as f:
        json.dump(res, f, indent=2, ensure_ascii=False)
    print("\n" + "=" * 60)
    for r in res:
        print(f"{r['name']:<20} max={r['max']:<10} {r['time']:.1f}s  {r['diag'][:70]}")
