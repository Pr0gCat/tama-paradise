//! Tween curves (`FUN_1008ebec`, weights at 0x10061d40..0x10062224) and the additive oscillators
//! (`FUN_1008ea08`), in 16.16 fixed point like the firmware (docs/spec/07-animation.md §2.8).

const ONE: i32 = 0x10000;

/// 16.16 sine of a 16.16 turn (`FUN_10031b74`: a 1024-step table of 4096·sin, shifted to 16.16).
// ponytail: the table is computed, not copied from the ROM; entries can differ by 1/4096.
fn sin(x: i32) -> i32 {
    let i = ((x & 0xffff) >> 6) as f64;
    ((4096.0 * (std::f64::consts::TAU * i / 1024.0).sin()).round() as i32) << 4
}

fn mul(x: i32, y: i32) -> i32 {
    ((x as i64 * y as i64) >> 16) as i32
}

fn div_2pi(x: i32) -> i32 {
    (((x as i64) << 16) / 0x6487e) as i32
}

/// Weight 0..1 (16.16) of curve `c` (internal id: script arg B + 1) at progress `p` in (0, 1).
pub fn weight(c: u8, p: i32) -> i32 {
    match c {
        2 => ONE - sin((ONE - p) >> 2),
        3 => sin(p >> 2),
        4 => p - div_2pi(sin(p)),
        5 => {
            let s = ONE - sin((ONE - p) >> 2);
            mul(s, s)
        }
        6 => {
            let s = ONE - sin(p >> 2);
            ONE - mul(s, s)
        }
        7 => {
            let x = 2 * (p - div_2pi(sin(p))) - ONE;
            let e = ONE - mul(ONE - x.abs(), ONE - x.abs());
            ((x.signum().max(-1) | 1) * e >> 1) + 0x8000
        }
        _ => p,
    }
}

/// Value of a tween from `a` to `b` (whole units) `t` of `n` steps in, on curve `c` (0 = jump).
pub fn ease(c: u8, a: i32, b: i32, t: u32, n: u32) -> i32 {
    ease_fixed(c, a << 16, b << 16, t, n) >> 16
}

/// The same on values that are already 16.16 (`FUN_100669e8`): each term floored on its own.
pub fn ease_fixed(c: u8, a: i32, b: i32, t: u32, n: u32) -> i32 {
    if c == 0 || a == b || n == 0 || t >= n {
        return b;
    }
    let p = ((t as i64) << 16) / n as i64;
    if p < 1 {
        return a;
    }
    let w = weight(c, p as i32);
    mul(a, ONE - w) + mul(b, w)
}

/// Additive oscillator `mode` with amplitude `amp` at `q` (16.16 periods since it started).
pub fn osc(mode: u8, amp: i32, q: i32) -> i32 {
    let shape = match mode {
        1 if q < 0 => 0,
        1 => sin(q),
        2 => sin(q).abs(),
        3 => sin(q.clamp(0, 0x8000)),
        4 => sin(q.clamp(0, ONE)),
        5 => {
            let u = q & 0xffff;
            if u < 0x4000 {
                4 * u
            } else if u < 0xc000 {
                0x20000 - 4 * u
            } else {
                4 * u - 0x40000
            }
        }
        6 => {
            let u = q.clamp(0, 0x8000);
            if u < 0x4000 {
                4 * u
            } else {
                0x20000 - 4 * u
            }
        }
        7 => ONE - (q & 0xffff),
        8 if q < 0 => 0,
        8 | 9 if q >= 0 => ONE - q.clamp(0, 0xffff),
        9 => ONE,
        10 if q < 0 => 0,
        10 => ((q & 0xffff) >= 0x8000) as i32 * ONE,
        11 if !(0..=0xffff).contains(&q) => ONE,
        11 => (q >= 0x8000) as i32 * ONE,
        12 => ONE,
        _ => 0,
    };
    mul(shape, amp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_match_the_firmware() {
        // From the ROM's table at p = k·0x2000 (k = 1..7); the computed sine may differ by a few.
        let want: [(u8, [i32; 7]); 7] = [
            (1, [8192, 16384, 24576, 32768, 40960, 49152, 57344]),
            (2, [1264, 4992, 11056, 19200, 29136, 40464, 52752]),
            (3, [12784, 25072, 36400, 46336, 54480, 60544, 64272]),
            (4, [818, 5954, 17202, 32768, 48334, 59582, 64718]),
            (5, [24, 380, 1865, 5625, 12953, 24983, 42461]),
            (6, [23075, 40553, 52583, 59911, 63671, 65156, 65512]),
            (7, [20, 1081, 9030, 32768, 56506, 64454, 65516]),
        ];
        for (c, ws) in want {
            for (k, w) in ws.into_iter().enumerate() {
                let got = weight(c, (k as i32 + 1) * 0x2000);
                assert!((got - w).abs() <= 48, "curve {c} at {}/8: {got} vs {w}", k + 1);
            }
        }
    }

    #[test]
    fn tween_ends_and_middle() {
        assert_eq!(ease(4, 0, 100, 0, 10), 0);
        assert_eq!(ease(4, 0, 100, 10, 10), 100);
        assert_eq!(ease(4, 0, 100, 5, 10), 50);
        assert_eq!(ease(7, 0, 100, 1, 10), 0, "S curve: flat start");
        assert_eq!(ease(0, 0, 100, 1, 10), 100, "no curve: jump");
    }

    #[test]
    fn square_blink() {
        assert_eq!(osc(10, ONE, 0x4000), 0);
        assert_eq!(osc(10, ONE, 0xc000), ONE);
    }
}
