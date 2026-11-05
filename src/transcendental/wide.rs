//! Multi-limb unsigned fixed-point arithmetic, `const` so the table
//! generators and the slow path share one implementation. Limbs are
//! little-endian `u64`; where the binary point sits is the caller's
//! business. Everything is built from 64-bit instructions because `u128`
//! multiplication and division are library calls on SBF.

use crate::kernel::widening_mul;

pub(super) type Limbs<const N: usize> = [u64; N];

pub(super) const fn add<const N: usize>(a: &Limbs<N>, b: &Limbs<N>) -> (Limbs<N>, bool) {
    let mut out = [0; N];
    let mut carry = false;
    let mut i = 0;
    while i < N {
        let (sum, c1) = a[i].overflowing_add(b[i]);
        let (sum, c2) = sum.overflowing_add(carry as u64);
        out[i] = sum;
        carry = c1 | c2;
        i += 1;
    }
    (out, carry)
}

pub(super) const fn sub<const N: usize>(a: &Limbs<N>, b: &Limbs<N>) -> (Limbs<N>, bool) {
    let mut out = [0; N];
    let mut borrow = false;
    let mut i = 0;
    while i < N {
        let (diff, b1) = a[i].overflowing_sub(b[i]);
        let (diff, b2) = diff.overflowing_sub(borrow as u64);
        out[i] = diff;
        borrow = b1 | b2;
        i += 1;
    }
    (out, borrow)
}

/// `|a - b|` and whether `a < b`.
#[inline(always)]
pub(super) const fn abs_diff<const N: usize>(a: &Limbs<N>, b: &Limbs<N>) -> (Limbs<N>, bool) {
    let (difference, negative) = sub(a, b);
    (if negative { sub(b, a).0 } else { difference }, negative)
}

pub(super) const fn cmp<const N: usize>(a: &Limbs<N>, b: &Limbs<N>) -> core::cmp::Ordering {
    let mut i = N;
    while i > 0 {
        i -= 1;
        if a[i] != b[i] {
            return if a[i] < b[i] {
                core::cmp::Ordering::Less
            } else {
                core::cmp::Ordering::Greater
            };
        }
    }
    core::cmp::Ordering::Equal
}

pub(super) const fn is_zero<const N: usize>(a: &Limbs<N>) -> bool {
    let mut i = 0;
    while i < N {
        if a[i] != 0 {
            return false;
        }
        i += 1;
    }
    true
}

/// `a * m` as `N` limbs plus the carry-out limb. Inlined by force: left to
/// itself LLVM outlines the loop as a function that moves every limb
/// through the stack.
#[inline(always)]
pub(super) const fn mul_small<const N: usize>(a: &Limbs<N>, m: u64) -> (Limbs<N>, u64) {
    let mut out = [0; N];
    let mut carry = 0;
    let mut i = 0;
    while i < N {
        let (high, low) = widening_mul(a[i], m);
        let (low, c) = low.overflowing_add(carry);
        out[i] = low;
        carry = high + c as u64;
        i += 1;
    }
    (out, carry)
}

/// Shift right by `bits < 64 * N`, dropping what falls off the end.
pub(super) const fn shr<const N: usize>(a: &Limbs<N>, bits: u32) -> Limbs<N> {
    let limbs = (bits / 64) as usize;
    let inner = bits % 64;
    let mut out = [0; N];
    let mut i = 0;
    while i + limbs < N {
        let mut word = a[i + limbs] >> inner;
        if inner != 0 && i + limbs + 1 < N {
            word |= a[i + limbs + 1] << (64 - inner);
        }
        out[i] = word;
        i += 1;
    }
    out
}

/// Shift left by `bits < 64 * N`, dropping what falls off the top.
pub(super) const fn shl<const N: usize>(a: &Limbs<N>, bits: u32) -> Limbs<N> {
    let limbs = (bits / 64) as usize;
    let inner = bits % 64;
    let mut out = [0; N];
    let mut i = N;
    while i > limbs {
        i -= 1;
        let mut word = a[i - limbs] << inner;
        if inner != 0 && i > limbs {
            word |= a[i - limbs - 1] >> (64 - inner);
        }
        out[i] = word;
    }
    out
}

/// Schoolbook `a * b` as `2N` limbs, low half first.
pub(super) const fn mul_full<const N: usize>(a: &Limbs<N>, b: &Limbs<N>) -> (Limbs<N>, Limbs<N>) {
    let mut low = [0u64; N];
    let mut high = [0u64; N];
    let mut i = 0;
    while i < N {
        let mut carry = 0u64;
        let mut j = 0;
        while j < N {
            let (p_high, p_low) = widening_mul(a[i], b[j]);
            let (slot, c1): (u64, bool) = if i + j < N {
                let (s, c) = low[i + j].overflowing_add(p_low);
                low[i + j] = s;
                (s, c)
            } else {
                let (s, c) = high[i + j - N].overflowing_add(p_low);
                high[i + j - N] = s;
                (s, c)
            };
            let (slot, c2) = slot.overflowing_add(carry);
            if i + j < N {
                low[i + j] = slot;
            } else {
                high[i + j - N] = slot;
            }
            carry = p_high + c1 as u64 + c2 as u64;
            j += 1;
        }
        high[i] = high[i].wrapping_add(carry);
        i += 1;
    }
    (low, high)
}

/// `floor(a * b / 2^(64 * N - shift))` for `shift < 64`: the top `N` limbs
/// of the product, moved up by `shift` bits. The caller keeps the result
/// below `2^(64 * N)`.
pub(super) const fn mul_shr<const N: usize>(a: &Limbs<N>, b: &Limbs<N>, shift: u32) -> Limbs<N> {
    let (low, high) = mul_full(a, b);
    if shift == 0 {
        return high;
    }
    let mut out = shl(&high, shift);
    out[0] |= low[N - 1] >> (64 - shift);
    out
}

/// `floor(a / d)` for a nonzero `d`; `const` only, so `u128` division is
/// fine here.
pub(super) const fn div_small_const<const N: usize>(a: &Limbs<N>, d: u64) -> Limbs<N> {
    let mut out = [0; N];
    let mut rem = 0u128;
    let mut i = N;
    while i > 0 {
        i -= 1;
        let cur = (rem << 64) | a[i] as u128;
        out[i] = (cur / d as u128) as u64;
        rem = cur % d as u128;
    }
    out
}

/// `floor(a * 2^(64 * N) / d)` for `0 < a < d`, by bitwise long division;
/// `const` only.
pub(super) const fn div_frac_const<const N: usize>(a: &Limbs<N>, d: &Limbs<N>) -> Limbs<N> {
    let mut rem = *a;
    let mut out = [0; N];
    let mut bit = 64 * N;
    while bit > 0 {
        bit -= 1;
        let top = rem[N - 1] >> 63;
        rem = shl(&rem, 1);
        if top != 0 || cmp(&rem, d).is_ge() {
            rem = sub(&rem, d).0;
            out[bit / 64] |= 1 << (bit % 64);
        }
    }
    out
}
