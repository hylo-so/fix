//! On Solana's SBF target every 64-bit instruction costs one compute unit,
//! unsigned 64-bit division included, while `u128` arithmetic and signed
//! division are library calls costing hundreds. The 64-bit kernels here
//! match the `muldiv` crate bit for bit, `None` on overflow and the panic
//! on a zero denominator included; the 128-bit kernels follow the same
//! rounding and sign rules.

#[derive(Clone, Copy)]
pub enum Round {
    Floor,
    Nearest,
    Ceil,
}

/// Every integer width implements this, because an inherent method on
/// `Fix` whose bound fails is an error rather than a fallback to the
/// `MulDiv` trait method; widths up to 32 bits delegate to `muldiv`.
/// `pub` only so the inherent methods can name it; the module is private.
pub trait MulDivKernel: Sized {
    fn mul_div(self, num: Self, denom: Self, round: Round) -> Option<Self>;
}

pub trait DivKernel: Sized {
    fn checked_div(self, d: Self) -> Option<Self>;
}

macro_rules! delegate_kernels {
    ($($t:ident),*) => {$(
        impl MulDivKernel for $t {
            #[inline(always)]
            fn mul_div(self, num: $t, denom: $t, round: Round) -> Option<$t> {
                match round {
                    Round::Floor => muldiv::MulDiv::mul_div_floor(self, num, denom),
                    Round::Nearest => muldiv::MulDiv::mul_div_round(self, num, denom),
                    Round::Ceil => muldiv::MulDiv::mul_div_ceil(self, num, denom),
                }
            }
        }
    )*};
}

delegate_kernels!(u8, u16, u32, i8, i16, i32);

macro_rules! delegate_div {
    ($($t:ident),*) => {$(
        impl DivKernel for $t {
            #[inline(always)]
            fn checked_div(self, d: $t) -> Option<$t> {
                $t::checked_div(self, d)
            }
        }
    )*};
}

delegate_div!(u8, u16, u32, u64, i8, i16, i32, usize, isize);

impl DivKernel for u128 {
    #[inline(always)]
    fn checked_div(self, d: u128) -> Option<u128> {
        if d == 0 {
            None
        } else {
            Some(div_u128(self, d))
        }
    }
}

const MASK: u64 = u32::MAX as u64;

/// From 32-bit partials because the native `u128` product lowers to a
/// builtin call on SBF.
#[inline(always)]
pub(crate) const fn widening_mul(a: u64, b: u64) -> (u64, u64) {
    let (a_high, a_low) = (a >> 32, a & MASK);
    let (b_high, b_low) = (b >> 32, b & MASK);
    let low_low = a_low * b_low;
    let low_high = a_low * b_high;
    let high_low = a_high * b_low;
    let middle = (low_low >> 32) + (low_high & MASK) + (high_low & MASK);
    let high = a_high * b_high + (low_high >> 32) + (high_low >> 32) + (middle >> 32);
    (high, a.wrapping_mul(b))
}

/// One 32-bit quotient digit of Knuth's Algorithm D: the native division
/// by the divisor's top digit overestimates by at most two, and each loop
/// pass fixes one. With a two-digit divisor the loop's test is exact, so
/// the digit is final and the remainder is the plain difference.
#[inline(always)]
fn divide_digit(u_high: u64, u_low: u64, normalized: u64) -> (u64, u64) {
    let top = normalized >> 32;
    let bottom = normalized & MASK;
    let mut digit = u_high / top;
    let mut rest = u_high % top;
    while digit > MASK || digit * bottom > ((rest << 32) | u_low) {
        digit -= 1;
        rest += top;
        if rest > MASK {
            break;
        }
    }
    let remainder = ((u_high << 32) | u_low).wrapping_sub(digit.wrapping_mul(normalized));
    (digit, remainder)
}

#[inline(always)]
fn div_rem_128_by_64(high: u64, low: u64, d: u64) -> (u64, u64) {
    debug_assert!(d != 0 && high < d);
    if high == 0 {
        return (low / d, low % d);
    }
    if d <= MASK {
        let upper = (high << 32) | (low >> 32);
        let lower = ((upper % d) << 32) | (low & MASK);
        return (((upper / d) << 32) | (lower / d), lower % d);
    }
    let shift = d.leading_zeros();
    div_rem_normalized(high, low, d << shift, shift)
}

/// Two Knuth digit steps of `(high:low)` by `d << shift`, for a divisor
/// `d` normalized by its caller.
#[inline(always)]
fn div_rem_normalized(high: u64, low: u64, normalized: u64, shift: u32) -> (u64, u64) {
    let (n2, n10) = if shift == 0 {
        (high, low)
    } else {
        ((high << shift) | (low >> (64 - shift)), low << shift)
    };
    let (q1, r1) = divide_digit(n2, n10 >> 32, normalized);
    let (q0, r0) = divide_digit(r1, n10 & MASK, normalized);
    ((q1 << 32) | q0, r0 >> shift)
}

/// Normalizes the divisor once for both word steps.
#[inline(always)]
fn div_rem_192_by_64(u2: u64, u1: u64, u0: u64, d: u64) -> (u128, u64) {
    debug_assert!(d != 0 && u2 < d);
    if u2 == 0 {
        return div_rem_wide(u1, u0, d);
    }
    if d <= MASK {
        let (q1, r1) = div_rem_128_by_64(u2, u1, d);
        let (q0, r0) = div_rem_128_by_64(r1, u0, d);
        return ((u128::from(q1) << 64) | u128::from(q0), r0);
    }
    let shift = d.leading_zeros();
    let normalized = d << shift;
    let (q1, r1) = div_rem_normalized(u2, u1, normalized, shift);
    let (q0, r0) = div_rem_normalized(r1, u0, normalized, shift);
    ((u128::from(q1) << 64) | u128::from(q0), r0)
}

#[inline(always)]
fn div_rem_wide(high: u64, low: u64, d: u64) -> (u128, u64) {
    let (q_low, r) = div_rem_128_by_64(high % d, low, d);
    ((u128::from(high / d) << 64) | u128::from(q_low), r)
}

/// Panics on `d == 0` like `muldiv`.
#[inline(always)]
fn mul_div_u64(a: u64, b: u64, d: u64) -> Option<(u64, u64)> {
    assert_ne!(d, 0);
    if a | b <= MASK {
        let p = a * b;
        return Some((p / d, p % d));
    }
    let (high, low) = widening_mul(a, b);
    if high >= d {
        return None;
    }
    Some(div_rem_128_by_64(high, low, d))
}

/// One 64-bit quotient digit of Knuth's Algorithm D by the normalized
/// two-word divisor `(v1:v0)`. As in [`divide_digit`], the loop's test is
/// exact for a two-digit divisor, so the remainder fits two words and is
/// computed as such.
#[inline(always)]
fn divide_word(high: u64, mid: u64, low: u64, v1: u64, v0: u64) -> (u64, u64, u64) {
    debug_assert!(
        (u128::from(high) << 64) | u128::from(mid) < (u128::from(v1) << 64) | u128::from(v0)
    );
    let (mut qhat, mut rhat, mut wide) = if high >= v1 {
        let (rhat, wide) = mid.overflowing_add(v1);
        (u64::MAX, rhat, wide)
    } else {
        let (qhat, rhat) = div_rem_128_by_64(high, mid, v1);
        (qhat, rhat, false)
    };
    while !wide {
        let (p_high, p_low) = widening_mul(qhat, v0);
        if p_high < rhat || (p_high == rhat && p_low <= low) {
            break;
        }
        qhat -= 1;
        (rhat, wide) = rhat.overflowing_add(v1);
    }
    let (p_high, p_low) = widening_mul(qhat, v0);
    let (r_low, borrow) = low.overflowing_sub(p_low);
    let r_mid = mid
        .wrapping_sub(qhat.wrapping_mul(v1))
        .wrapping_sub(p_high)
        .wrapping_sub(u64::from(borrow));
    (qhat, r_mid, r_low)
}

#[inline(always)]
fn div_rem_256_by_128(u3: u64, u2: u64, u1: u64, u0: u64, d: u128) -> (u128, u128) {
    debug_assert!(d > u128::from(u64::MAX) && (u128::from(u3) << 64) | u128::from(u2) < d);
    let shift = ((d >> 64) as u64).leading_zeros();
    let v = d << shift;
    let (v1, v0) = ((v >> 64) as u64, v as u64);
    let (n3, n2, n1, n0) = if shift == 0 {
        (u3, u2, u1, u0)
    } else {
        let back = 64 - shift;
        (
            (u3 << shift) | (u2 >> back),
            (u2 << shift) | (u1 >> back),
            (u1 << shift) | (u0 >> back),
            u0 << shift,
        )
    };
    let (q1, n2, n1) = divide_word(n3, n2, n1, v1, v0);
    let (q0, n1, n0) = divide_word(n2, n1, n0, v1, v0);
    let remainder = ((u128::from(n1) << 64) | u128::from(n0)) >> shift;
    ((u128::from(q1) << 64) | u128::from(q0), remainder)
}

/// Panics on `d == 0` like `muldiv`.
#[inline(always)]
fn mul_div_u128(a: u128, b: u128, d: u128) -> Option<(u128, u128)> {
    assert_ne!(d, 0);
    let word = u128::from(u64::MAX);
    if a | b | d <= word {
        let (high, low) = widening_mul(a as u64, b as u64);
        let (q, r) = div_rem_wide(high, low, d as u64);
        return Some((q, u128::from(r)));
    }
    let (a1, a0) = ((a >> 64) as u64, a as u64);
    let (b1, b0) = ((b >> 64) as u64, b as u64);
    let (ll_high, ll_low) = widening_mul(a0, b0);
    let (lh_high, lh_low) = widening_mul(a0, b1);
    let (hl_high, hl_low) = widening_mul(a1, b0);
    let (hh_high, hh_low) = widening_mul(a1, b1);
    let (u1, c1) = ll_high.overflowing_add(lh_low);
    let (u1, c2) = u1.overflowing_add(hl_low);
    let (u2, c3) = hh_low.overflowing_add(lh_high);
    let (u2, c4) = u2.overflowing_add(hl_high);
    let (u2, c5) = u2.overflowing_add(u64::from(c1) + u64::from(c2));
    let u3 = hh_high + u64::from(c3) + u64::from(c4) + u64::from(c5);
    if (u128::from(u3) << 64) | u128::from(u2) >= d {
        return None;
    }
    if d <= word {
        let (q, r) = div_rem_192_by_64(u2, u1, ll_low, d as u64);
        return Some((q, u128::from(r)));
    }
    Some(div_rem_256_by_128(u3, u2, u1, ll_low, d))
}

/// A zero divisor goes to `/` for the operator's own panic.
#[inline(always)]
pub(crate) fn div_u128(a: u128, d: u128) -> u128 {
    if d > u128::from(u64::MAX) || d == 0 {
        return a / d;
    }
    div_rem_wide((a >> 64) as u64, a as u64, d as u64).0
}

/// A zero divisor and `MIN / -1` go to `/` for the operator's own panic.
macro_rules! signed_div {
    ($name:ident, $t:ident, $div:expr) => {
        #[inline(always)]
        pub(crate) fn $name(a: $t, d: $t) -> $t {
            if d == 0 || (a == $t::MIN && d == -1) {
                return a / d;
            }
            let q = $div(a.unsigned_abs(), d.unsigned_abs()) as $t;
            if (a < 0) != (d < 0) {
                q.wrapping_neg()
            } else {
                q
            }
        }

        impl DivKernel for $t {
            #[inline(always)]
            fn checked_div(self, d: $t) -> Option<$t> {
                if d == 0 || (self == $t::MIN && d == -1) {
                    None
                } else {
                    Some($name(self, d))
                }
            }
        }
    };
}

signed_div!(div_i64, i64, |a: u64, d: u64| a / d);
signed_div!(div_i128, i128, div_u128);

macro_rules! unsigned_kernel {
    ($t:ident, $mul_div:ident) => {
        impl MulDivKernel for $t {
            #[inline(always)]
            fn mul_div(self, num: $t, denom: $t, round: Round) -> Option<$t> {
                let (q, r) = $mul_div(self, num, denom)?;
                match round {
                    Round::Nearest if r >= denom - (denom >> 1) => q.checked_add(1),
                    Round::Ceil if r != 0 => q.checked_add(1),
                    _ => Some(q),
                }
            }
        }
    };
}

unsigned_kernel!(u64, mul_div_u64);
unsigned_kernel!(u128, mul_div_u128);

macro_rules! signed_kernel {
    ($t:ident, $u:ident) => {
        impl MulDivKernel for $t {
            #[inline(always)]
            fn mul_div(self, num: $t, denom: $t, round: Round) -> Option<$t> {
                let negative = (self < 0) ^ (num < 0) ^ (denom < 0) && self != 0 && num != 0;
                let (a, b, d) = (
                    self.unsigned_abs(),
                    num.unsigned_abs(),
                    denom.unsigned_abs(),
                );
                let r = match (round, negative) {
                    (Round::Floor, false) | (Round::Ceil, true) => a.mul_div(b, d, Round::Floor),
                    (Round::Ceil, false) | (Round::Floor, true) => a.mul_div(b, d, Round::Ceil),
                    (Round::Nearest, _) => a.mul_div(b, d, Round::Nearest),
                };
                match r {
                    Some(r) if r <= $t::MAX as $u => {
                        Some(if negative { -(r as $t) } else { r as $t })
                    }
                    Some(r) if negative && r == $t::MIN.unsigned_abs() => Some($t::MIN),
                    _ => None,
                }
            }
        }
    };
}

signed_kernel!(i64, u64);
signed_kernel!(i128, u128);

#[cfg(test)]
mod tests {
    use super::{MulDivKernel, Round};
    use muldiv::MulDiv;

    fn next(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }

    fn sweep64(state: &mut u64, i: u64) -> u64 {
        next(state) >> (i % 64)
    }

    fn sweep128(state: &mut u64, i: u64) -> u128 {
        let x = (u128::from(next(state)) << 64) | u128::from(next(state));
        x >> (i % 128)
    }

    fn rounded<T: MulDivKernel + Copy>(a: T, b: T, d: T) -> [Option<T>; 3] {
        [Round::Floor, Round::Nearest, Round::Ceil].map(|round| a.mul_div(b, d, round))
    }

    fn by_muldiv<T: MulDiv<Output = T> + Copy>(a: T, b: T, d: T) -> [Option<T>; 3] {
        [
            a.mul_div_floor(b, d),
            a.mul_div_round(b, d),
            a.mul_div_ceil(b, d),
        ]
    }

    fn wide_mul(a: u128, b: u128) -> (u128, u128) {
        let word = u128::from(u64::MAX);
        let (a_hi, a_lo) = (a >> 64, a & word);
        let (b_hi, b_lo) = (b >> 64, b & word);
        let ll = a_lo * b_lo;
        let lh = a_lo * b_hi;
        let hl = a_hi * b_lo;
        let mid = (ll >> 64) + (lh & word) + (hl & word);
        (
            (a_hi * b_hi) + (lh >> 64) + (hl >> 64) + (mid >> 64),
            (ll & word) | (mid << 64),
        )
    }

    fn oracle_div(hi: u128, mut lo: u128, d: u128) -> Option<(u128, u128)> {
        if hi >= d {
            return None;
        }
        let (mut q, mut r) = (0u128, hi);
        for _ in 0..128 {
            let carry = r >> 127;
            r = (r << 1) | (lo >> 127);
            lo <<= 1;
            q <<= 1;
            if carry == 1 || r >= d {
                r = r.wrapping_sub(d);
                q |= 1;
            }
        }
        Some((q, r))
    }

    fn oracle_u128(a: u128, b: u128, d: u128) -> Option<(u128, u128)> {
        let (hi, lo) = wide_mul(a, b);
        oracle_div(hi, lo, d)
    }

    #[test]
    fn u64_matches_muldiv() {
        let mut state = 0x6b65_726e_656c_3634;
        for i in 0..2_000_000u64 {
            let a = sweep64(&mut state, i);
            let b = sweep64(&mut state, i / 64);
            let d = sweep64(&mut state, i / 4096).max(1);
            assert_eq!(rounded(a, b, d), by_muldiv(a, b, d), "{a} {b} {d}");
        }
    }

    #[test]
    fn i64_matches_muldiv() {
        let edges = [0, 1, -1, i64::MAX, i64::MIN, i64::MAX - 1, i64::MIN + 1];
        for &a in &edges {
            for &b in &edges {
                for &d in &edges {
                    if d != 0 {
                        assert_eq!(rounded(a, b, d), by_muldiv(a, b, d), "{a} {b} {d}");
                    }
                }
            }
        }
        let mut state = 0x6b65_726e_656c_3635;
        for i in 0..2_000_000u64 {
            let a = sweep64(&mut state, i) as i64;
            let b = sweep64(&mut state, i / 64) as i64;
            let d = (sweep64(&mut state, i / 4096) as i64).max(1) * if i % 2 == 0 { 1 } else { -1 };
            assert_eq!(rounded(a, b, d), by_muldiv(a, b, d), "{a} {b} {d}");
        }
    }

    #[test]
    fn u128_matches_oracle() {
        fn check(a: u128, b: u128, d: u128) {
            let expected = oracle_u128(a, b, d);
            assert_eq!(super::mul_div_u128(a, b, d), expected, "{a} {b} {d}");
            let [floor, nearest, ceil] = rounded(a, b, d);
            let Some((q, r)) = expected else {
                assert_eq!((floor, nearest, ceil), (None, None, None));
                return;
            };
            assert_eq!(floor, Some(q));
            assert_eq!(nearest, q.checked_add(u128::from(r >= d - (d >> 1))));
            assert_eq!(ceil, q.checked_add(u128::from(r != 0)));
        }
        let edges = [
            0,
            1,
            u128::MAX,
            u128::MAX - 1,
            u128::from(u64::MAX),
            u128::from(u64::MAX) + 1,
            1 << 127,
        ];
        for &a in &edges {
            for &b in &edges {
                for &d in &edges {
                    if d != 0 {
                        check(a, b, d);
                    }
                }
            }
        }
        let mut state = 0x6b65_726e_656c_3132;
        for i in 0..1_000_000u64 {
            check(
                sweep128(&mut state, i),
                sweep128(&mut state, i / 128),
                sweep128(&mut state, i / 16384).max(1),
            );
        }
    }

    #[test]
    fn i128_matches_sign_rules() {
        let mut state = 0x6b65_726e_656c_3133;
        for i in 0..1_000_000u64 {
            let a = sweep128(&mut state, i) as i128;
            let b = sweep128(&mut state, i / 128) as i128;
            let d =
                (sweep128(&mut state, i / 16384) as i128).max(1) * if i % 2 == 0 { 1 } else { -1 };
            let negative = (a < 0) ^ (b < 0) ^ (d < 0) && a != 0 && b != 0;
            let Some((q, r)) = oracle_u128(a.unsigned_abs(), b.unsigned_abs(), d.unsigned_abs())
            else {
                assert_eq!(rounded(a, b, d), [None; 3]);
                continue;
            };
            let fit = |m: u128| -> Option<i128> {
                if m <= i128::MAX as u128 {
                    Some(if negative { -(m as i128) } else { m as i128 })
                } else if negative && m == 1 << 127 {
                    Some(i128::MIN)
                } else {
                    None
                }
            };
            let up = q.checked_add(u128::from(r != 0));
            let floor = if negative { up.and_then(fit) } else { fit(q) };
            let ceil = if negative { fit(q) } else { up.and_then(fit) };
            let nearest = q
                .checked_add(u128::from(r >= d.unsigned_abs() - (d.unsigned_abs() >> 1)))
                .and_then(fit);
            assert_eq!(rounded(a, b, d), [floor, nearest, ceil], "{a} {b} {d}");
        }
    }

    #[test]
    fn signed_and_wide_division_match_the_operators() {
        let mut state = 0x6469_765f_6b65_726e;
        for i in 0..1_000_000u64 {
            let a = sweep64(&mut state, i) as i64;
            let d = (sweep64(&mut state, i / 64) as i64).max(1) * if i % 2 == 0 { 1 } else { -1 };
            assert_eq!(super::div_i64(a, d), a / d, "{a} / {d}");
            assert_eq!(
                super::DivKernel::checked_div(a, d),
                a.checked_div(d),
                "{a} / {d}"
            );
            let a = sweep128(&mut state, i) as i128;
            let d =
                (sweep128(&mut state, i / 128) as i128).max(1) * if i % 2 == 0 { 1 } else { -1 };
            assert_eq!(super::div_i128(a, d), a / d, "{a} / {d}");
            assert_eq!(
                super::DivKernel::checked_div(a, d),
                a.checked_div(d),
                "{a} / {d}"
            );
            let a = sweep128(&mut state, i);
            let d = sweep128(&mut state, i / 128).max(1);
            assert_eq!(super::div_u128(a, d), a / d, "{a} / {d}");
            assert_eq!(
                super::DivKernel::checked_div(a, d),
                a.checked_div(d),
                "{a} / {d}"
            );
        }
        for a in [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX] {
            for d in [i64::MIN, -1, 1, 2, i64::MAX] {
                assert_eq!(
                    super::DivKernel::checked_div(a, d),
                    a.checked_div(d),
                    "{a} / {d}"
                );
                if !(a == i64::MIN && d == -1) {
                    assert_eq!(super::div_i64(a, d), a / d, "{a} / {d}");
                }
            }
        }
        assert_eq!(super::DivKernel::checked_div(i64::MIN, 0), None);
        assert_eq!(super::DivKernel::checked_div(i128::MIN, -1), None);
        assert_eq!(super::DivKernel::checked_div(1u128, 0), None);
    }

    #[test]
    #[should_panic]
    fn zero_denominator_panics_like_muldiv() {
        let _ = 1u64.mul_div(1, 0, Round::Floor);
    }

    #[test]
    #[rustfmt::skip]
    fn knuth_corrections_at_32_bit_digits() {
        for (forces, high, low, d) in [
            ("estimate 2^32, one decrement",                           0x8000_0000_0000_0000, 0, 0x8000_0000_0000_0001),
            ("product test decrements once",                           0x8000_0000,           0, 0x8000_0000_0000_0001),
            ("two decrements, then the partial remainder overflows",   0x7fff_ffff_0000_0000, 0, 0x8000_0000_ffff_ffff),
            ("estimate 2^32 + 1, two decrements, remainder overflows", 0x8000_0000_ffff_fffe, 0, 0x8000_0000_ffff_ffff),
            ("two decrements after a one-bit normalization",           0x3fff_ffff_8000_0000, 0, 0x4000_0000_7fff_ffff),
        ] {
            let n = (u128::from(high) << 64) | u128::from(low);
            let expected = ((n / u128::from(d)) as u64, (n % u128::from(d)) as u64);
            assert_eq!(super::div_rem_128_by_64(high, low, d), expected, "{forces}");
        }
    }

    #[test]
    #[rustfmt::skip]
    fn knuth_corrections_at_64_bit_digits() {
        for (forces, u3, u2, d) in [
            ("estimate pinned at 2^64 - 1",                                0x8000_0000_0000_0000, 0,                     0x8000_0000_0000_0000_0000_0000_0000_0001),
            ("pinned estimate, remainder overflows so the test is skipped", 0x8000_0000_0000_0000, 0xffff_ffff_ffff_fffe, 0x8000_0000_0000_0000_ffff_ffff_ffff_ffff),
            ("product test decrements once",                               0,                     0x8000_0000_0000_0000, 0x8000_0000_0000_0000_0000_0000_0000_0001),
            ("two decrements, then the remainder overflows",               0x7fff_ffff_ffff_ffff, 0,                     0x8000_0000_0000_0000_ffff_ffff_ffff_ffff),
            ("two decrements after a one-bit normalization",               0x3fff_ffff_ffff_ffff, 0x8000_0000_0000_0000, 0x4000_0000_0000_0000_7fff_ffff_ffff_ffff),
        ] {
            let expected = oracle_div((u128::from(u3) << 64) | u128::from(u2), 0, d).unwrap();
            assert_eq!(super::div_rem_256_by_128(u3, u2, 0, 0, d), expected, "{forces}");
        }
    }
}
