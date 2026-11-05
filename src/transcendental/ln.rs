use super::exp::exp_q376;
use super::tables::{
    one, Gen, INDEX_BITS, INDEX_MASK, LN2_GEN, LN_R1, LN_R2, LN_T1, LN_T2, Q184, S_LN2,
};
use super::wide::{abs_diff, add, mul_shr, mul_small, shr, sub};
use super::{
    as_q143, from_q, full, head, join, mul_by_u128, mul_high_128_approx, mul_high_approx,
    mul_small_shr, scaled, settle, settle_word, split, top_q120,
    Scale, Split, Unsettled, MASK32, MASK56,
};
use crate::kernel::{widening_mul, Round};
use core::num::NonZeroU64;

#[inline(always)]
fn settle_signed(
    negative: bool,
    round: Round,
    settle: impl FnOnce(Round) -> Option<Option<u128>>,
) -> Result<Option<i128>, Unsettled> {
    let round = match (negative, round) {
        (true, Round::Floor) => Round::Ceil,
        (true, Round::Ceil) => Round::Floor,
        _ => round,
    };
    let magnitude = settle(round).ok_or(Unsettled)?;
    Ok(magnitude.and_then(|m| {
        if negative && m == 1 << 127 {
            return Some(i128::MIN);
        }
        let value = i128::try_from(m).ok()?;
        Some(if negative { -value } else { value })
    }))
}

// An if chain: a match on `cmp` costs 2 to 4 CU more on SBF.
#[allow(clippy::comparison_chain)]
#[inline(always)]
pub(super) fn tiny(bits: u128, pow10: u128, round: Round) -> i128 {
    let down = i128::from(matches!(round, Round::Floor));
    if bits > pow10 {
        (bits - pow10) as i128 - down
    } else if bits < pow10 {
        -((pow10 - bits) as i128) - down
    } else {
        0
    }
}

/// Tier 1.
#[inline(always)]
pub(super) fn fast(bits: u128, scale: &Scale, round: Round) -> Result<Option<i128>, Unsettled> {
    let (s, mh, ml) = normalize(bits);
    // c1 = ml / (mh 2^64) as q1 / 2^96.
    let q1 = ((ml >> 32) << 32) / nonzero(mh >> 32);

    let j1 = ((mh >> 55) & INDEX_MASK as u64) as usize;
    let r1 = u64::from(LN_R1[j1]);
    let (a, b) = ((mh >> 32) * r1, (mh & MASK32) * r1);
    let (hi1, lo1) = (a + (b >> 32), b & MASK32);
    // c2 = lo1 / (hi1 2^32) as q2 / 2^95.
    let q2 = (lo1 << 31) / nonzero(hi1 >> 32);

    let j2 = (((hi1 - (1 << 62)) >> (62 - 2 * INDEX_BITS)) as usize).min(INDEX_MASK);
    let r2 = u64::from(LN_R2[j2]);
    let (a, b) = ((hi1 >> 32) * r2, (hi1 & MASK32) * r2);
    let (hi2, lo2) = (a + (b >> 32), b & MASK32);
    // r = rh 2^32 + lo2 at Q93.
    let rh = hi2 - (1 << 61);
    debug_assert!(rh < 1 << 46);
    let r79 = (rh << 18) | (lo2 >> 14);
    let rr = mul_high_approx(r79, r79);
    let rrr = mul_high_approx(rr, r79);
    let rrrr = (rr >> 32) * (rr >> 32);
    // r^2/2 - r^3/3 + r^4/4 at Q94, below 2^61.
    let tail = rr / 2 + ((rrrr / 4) >> 30) - ((rrr / 3) >> 15);

    let s_ln2 = top_q120(&S_LN2[s as usize]);
    let r120 = (u128::from(rh) << 59) | (u128::from(lo2) << 27);
    let c = (q1 << 24) + (q2 << 25);
    let positive = s_ln2 + top_q120(&LN_T1[j1]) + top_q120(&LN_T2[j2]) + r120 + u128::from(c);
    let negative_part = top_q120(&scale.exp_ln10_biased) + (u128::from(tail) << 26);
    // |ln v| 2^120 < 2^127, so the difference's top bit is its sign and
    // the magnitude is the conditional negation.
    let difference = positive.wrapping_sub(negative_part);
    let negative = difference >> 127 != 0;
    let mask = 0u128.wrapping_sub(u128::from(negative));
    let magnitude = (difference ^ mask).wrapping_sub(mask);

    let (n, f_high) = if scale.pow10 >> 64 == 0 {
        let p = scale.pow10 as u64;
        let (high, low) = widening_mul((magnitude >> 64) as u64, p);
        let q = join(high, low) + u128::from(mul_high_approx(magnitude as u64, p));
        (q >> 56, (q as u64 & MASK56) << 8)
    } else {
        let five = (scale.pow10 >> scale.exp) as u64;
        let (h0, l0) = widening_mul(magnitude as u64, five);
        let (h1, l1) = widening_mul((magnitude >> 64) as u64, five);
        let (p1, carry) = h0.overflowing_add(l1);
        let p2 = h1 + u64::from(carry);
        let cut = 56 - scale.exp;
        (join(p2, p1) >> cut, (join(p1, l0) >> cut) as u64)
    };
    settle_signed(negative, round, |round| settle_word(n, f_high, scale.ln_eps_word, round))
}

/// A divisor that is nonzero by construction (`mh >= 2^63`, `hi1 >=
/// 2^62`), as the type that divides without a zero check. The fallback
/// never fires; it replaces the panic path a plain division would carry.
#[inline(always)]
fn nonzero(x: u64) -> NonZeroU64 {
    debug_assert!(x != 0);
    NonZeroU64::new(x).unwrap_or(NonZeroU64::MIN)
}

/// `bits = (mh 2^64 + ml) 2^(s - 127)` with `mh >= 2^63`, by a six-step
/// binary search on whichever limb leads: SBF has no count-leading-zeros
/// instruction, and this is shorter than the bit-smearing and popcount
/// expansion LLVM emits for `leading_zeros` (85 instructions for `u128`).
#[inline(always)]
fn normalize(bits: u128) -> (u32, u64, u64) {
    let (high, low) = ((bits >> 64) as u64, bits as u64);
    if high == 0 {
        let (shift, mh) = lead(low);
        (63 - shift, mh, 0)
    } else {
        let (shift, mh) = lead(high);
        let mh = if shift == 0 { mh } else { mh | (low >> (64 - shift)) };
        (127 - shift, mh, low << shift)
    }
}

/// `(x.leading_zeros(), x << x.leading_zeros())` for `x != 0`.
#[inline(always)]
fn lead(mut x: u64) -> (u32, u64) {
    debug_assert!(x != 0);
    let mut shift = 0;
    if x >> 32 == 0 {
        shift += 32;
        x <<= 32;
    }
    if x >> 48 == 0 {
        shift += 16;
        x <<= 16;
    }
    if x >> 56 == 0 {
        shift += 8;
        x <<= 8;
    }
    if x >> 60 == 0 {
        shift += 4;
        x <<= 4;
    }
    if x >> 62 == 0 {
        shift += 2;
        x <<= 2;
    }
    if x >> 63 == 0 {
        shift += 1;
        x <<= 1;
    }
    (shift, x)
}

/// `log1p(r)` for `0 <= r < 2^-16 (1 + 2^-11)` at Q184, within `2^-147`
/// of the truth: eight Taylor terms as `r - r^2 P_2`, the dropped tail
/// below `2^-147.16`.
///
/// `P_k = 1/k - r P_(k+1)` runs from `P_8 = 1/8` at Q64 down to `P_6`,
/// within `2^-63.2`, then at Q128 down to `P_2`. `P_5` takes the exact
/// product of the Q79 word of `r` and `P_6`: within `2^-78.9` (`P_6`'s
/// error times `r`, the word's `2^-79` times `P_6`). Each later stage is a
/// top-word product under seven units of `2^-143` short, truncated to
/// Q128, and carries the previous error times `r`: `P_4` within
/// `2^-94.9`, `P_3` within `2^-110.9`, `P_2` within `2^-125.9`. The
/// square `r^2` from the top-word product of `r` at Q143 is within eight
/// units of `2^-158` (one from `r`'s truncation, seven from the product);
/// `r^2 P_2` from one more top-word product at Q158 is within seven units
/// plus `r^2`'s error times `1/2` plus `P_2`'s times `r^2`, under
/// `2^-154.4`. Total below `2^-147.15`, with the stored reciprocals'
/// exact errors counted.
#[inline(always)]
fn log1p(r: &Q184) -> Q184 {
    let a = head(r);
    let r128 = as_q143(r);
    let p = u64::MAX / 8;
    let p = u64::MAX / 7 - (mul_high_approx(a, p) >> 15);
    let p = u64::MAX / 6 - (mul_high_approx(a, p) >> 15);
    let p = u128::MAX / 5 - (full(a, p) >> 15);
    let p = u128::MAX / 4 - (mul_high_128_approx(r128, p) >> 15);
    let p = u128::MAX / 3 - (mul_high_128_approx(r128, p) >> 15);
    let p = u128::MAX / 2 - (mul_high_128_approx(r128, p) >> 15);
    let r2 = mul_high_128_approx(r128, r128);
    sub(r, &from_q::<26>(mul_high_128_approx(r2, p))).0
}

/// `ln(m / 2^127)` for `m >= 2^127` at Q184, within `2^-146.99`: the two
/// table reductions on the full mantissa, then [`log1p`] of the residue.
#[inline(never)]
fn ln_mantissa(m: u128) -> Q184 {
    let x: Q184 = shr(&[0, m as u64, (m >> 64) as u64], 7);
    let j1 = ((x[2] & MASK56) >> (56 - INDEX_BITS)) as usize;
    let x = mul_small_shr(&x, u64::from(LN_R1[j1]), 31);
    debug_assert_eq!(x[2] >> 56, 1);
    let j2 = (((x[2] & MASK56) >> (56 - 2 * INDEX_BITS)) as usize).min(INDEX_MASK);
    let x = mul_small_shr(&x, u64::from(LN_R2[j2]), 31);
    debug_assert_eq!(x[2] >> 56, 1);
    let r = [x[0], x[1], x[2] & MASK56];
    add(&add(&LN_T1[j1], &LN_T2[j2]).0, &log1p(&r)).0
}

/// `|ln v| 10^E` at tier 2 as integer part, Q192 fraction and sign.
#[inline(always)]
fn middle_value(bits: u128, scale: &Scale) -> Option<(u128, Q184, bool)> {
    let (s, mh, ml) = normalize(bits);
    let positive = add(&S_LN2[s as usize], &ln_mantissa(join(mh, ml))).0;
    let (magnitude, negative) = abs_diff(&positive, &scale.exp_ln10_biased);
    let Split::Fits(n, f) = split::<3, 5>(&mul_by_u128(&magnitude, scale.pow10)) else {
        return None;
    };
    Some((n, f, negative))
}

/// Tier 2, settled on the fraction's top word. A refusal is retried by
/// [`middle_full`] at the start of [`last`], not here: anything kept alive
/// for a call out of this refusal branch costs spills on the common path.
#[inline(never)]
pub(super) fn middle(bits: u128, scale: &Scale, round: Round) -> Result<Option<i128>, Unsettled> {
    let Some((n, f, negative)) = middle_value(bits, scale) else {
        return Ok(None);
    };
    let eps_word = ((scale.pow10 >> 81) as u64) + 1;
    settle_signed(negative, round, |round| settle_word(n, f[2], eps_word, round))
}

/// Tier 2 settled on the whole fraction against the exact `eps`, deciding
/// what the top word's one-unit floor refused.
#[inline(never)]
pub(super) fn middle_full(bits: u128, scale: &Scale, round: Round) -> Result<Option<i128>, Unsettled> {
    let Some((n, f, negative)) = middle_value(bits, scale) else {
        return Ok(None);
    };
    settle_signed(negative, round, |round| settle(n, &f, &scaled(scale.pow10, 47), round))
}

/// Tier 3: one correction step on the tier-2 mantissa logarithm through
/// [`exp_q376`], after [`middle_full`] has had its try.
#[inline(never)]
pub(super) fn last(bits: u128, scale: &Scale, round: Round) -> Result<Option<i128>, Unsettled> {
    if let Ok(decided) = middle_full(bits, scale, round) {
        return Ok(decided);
    }
    let (s, mh, ml) = normalize(bits);
    let m = join(mh, ml);
    let y0 = ln_mantissa(m);
    let y0: Gen = [0, 0, 0, y0[0], y0[1], y0[2]];
    let (t, borrow) = sub(&LN2_GEN, &y0);
    debug_assert!(!borrow);
    let e = exp_q376(&t);
    let m6: Gen = shr(&[0, 0, 0, 0, m as u64, (m >> 64) as u64], 7);
    let half = shr(&mul_shr(&m6, &e, 8), 1);
    let (d, d_negative) = abs_diff(&half, &one());
    debug_assert!(d[5] == 0 && d[4] == 0 && d[3] < 1 << 38);
    // d^2 / 2 at Q376 from the top word of |d| at Q209.
    let d_top = (d[3] << 25) | (d[2] >> 39);
    let (dd_high, dd_low) = widening_mul(d_top, d_top);
    let dd = join(dd_high, dd_low) >> 43;
    let dd: Gen = [dd as u64, (dd >> 64) as u64, 0, 0, 0, 0];
    let mut positive = add(&mul_small(&LN2_GEN, u64::from(s) + 2).0, &y0).0;
    let (lo, hi) = (scale.exp_ln10_biased_low, scale.exp_ln10_biased);
    let mut negative_part: Gen = [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]];
    if d_negative {
        negative_part = add(&add(&negative_part, &d).0, &dd).0;
    } else {
        positive = sub(&add(&positive, &d).0, &dd).0;
    }
    let (magnitude, negative) = abs_diff(&positive, &negative_part);
    let Split::Fits(n, f) = split::<6, 8>(&mul_by_u128(&magnitude, scale.pow10)) else {
        return Ok(None);
    };
    settle_signed(negative, round, |round| settle(n, &f, &scaled(scale.pow10, 55), round))
}
