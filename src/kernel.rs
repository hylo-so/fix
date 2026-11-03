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
fn widening_mul(a: u64, b: u64) -> (u64, u64) {
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
