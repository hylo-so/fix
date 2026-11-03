//! On Solana's SBF target every 64-bit instruction costs one compute unit,
//! unsigned 64-bit division included, while `u128` arithmetic and signed
//! division are library calls costing hundreds. The 64-bit kernels here
//! match the `muldiv` crate bit for bit, `None` on overflow and the panic
//! on a zero denominator included.

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
