use super::tables::{
    one, Gen, EXP_T1, EXP_T2, INDEX_BITS, INDEX_MASK, LN2, LN2_GEN, LOG2E_Q504, Q184,
};
use super::wide::{add, cmp, div_small_const, is_zero, mul_shr, shl, shr, sub};
use super::{
    as_q143, from_q, full, head, join, mul_by_u128, mul_high_128_called,
    mul_high_approx, mul_small_shr, scaled, settle, settle_word, split, top_q120, Scale, Split,
    Unsettled, MASK56,
};
use crate::kernel::{widening_mul, Round};

#[inline(always)]
pub(super) fn tiny(negative: bool, bits: u128, pow10: u128, round: Round) -> u128 {
    if bits == 0 {
        return pow10;
    }
    let base = if negative { pow10 - bits } else { pow10 + bits };
    base + u128::from(matches!(round, Round::Ceil))
}

/// `floor(2^31 log2 e)`.
const LOG2E_Q31: u64 = LOG2E_Q504[7] >> 25;
/// `ln 2 / 2^16` at Q184, the exponent step of one level-2 table entry.
static LN2_STEP: Q184 = shr(&LN2, 2 * INDEX_BITS);
/// Below `ln 2` by this much, `s + 2 ln 2` puts the result under `1/2`
/// for certain: `2^-96` covers the conversion error at every exponent
/// the tier runs at.
const UNDER_HALF: u128 = top_q120(&LN2) - (1 << 24);
/// The same margin at Q184 for the middle tier, whose sum is within
/// `2^-180.5` of the truth, so `2^-176` is certain.
static UNDER_HALF_Q184: Q184 = sub(&LN2, &[1 << 8, 0, 0]).0;

/// Tier 1.
#[inline(always)]
pub(super) fn fast(
    negative: bool,
    a: u128,
    scale: &Scale,
    round: Round,
) -> Result<Option<u128>, Unsettled> {
    let c = &scale.inv_pow10;
    let (a1, a0) = ((a >> 64) as u64, a as u64);
    let mut x = u128::from(mul_high_approx(a0, c[2])) + full(a0, c[3]) + (full(a0, c[4]) << 64);
    if a1 != 0 {
        x += full(a1, c[2]) + (full(a1, c[3]) << 64);
    }
    // s here is s + 2 ln 2 at Q120; under ln 2 the result is under 1/2.
    let biased = top_q120(&scale.exp_ln10_biased);
    let s = if negative {
        let s = biased.wrapping_sub(x);
        if s >> 127 != 0 || s < UNDER_HALF {
            return Ok(Some(u128::from(matches!(round, Round::Ceil))));
        }
        s
    } else {
        biased + x
    };
    debug_assert!(s >> 120 < 185);
    let j_biased = (((s >> 96) as u64) * LOG2E_Q31) >> 39;
    let k = (j_biased >> (2 * INDEX_BITS)) as i32 - 2;
    if k > 83 {
        return Err(Unsettled);
    }
    let j1 = ((j_biased >> INDEX_BITS) & INDEX_MASK as u64) as usize;
    let j2 = (j_biased & INDEX_MASK as u64) as usize;
    // J ln 2 / 2^16 at Q120 from the Q184 step: the top limb's product is
    // below 2^64 since s is below 2^8, the middle one is a full product
    // and the low one keeps 32 bits, within two units.
    let step = (u128::from(j_biased * LN2_STEP[2]) << 64)
        + full(j_biased, LN2_STEP[1])
        + u128::from((j_biased * (LN2_STEP[0] >> 32)) >> 32);
    let h = s - step;

    let h64 = (h >> 40) as u64;
    let h2 = mul_high_approx(h64, h64);
    let h3 = mul_high_approx(h2, h64);
    let h4 = (h2 >> 32) * (h2 >> 32);
    let series = h2 / 2 + ((h3 / 6) >> 16) + ((h4 / 24) >> 32);
    let e1 = h + (u128::from(series) << 24);

    let p = mul_high_128_called(top_q120(&EXP_T1[j1]), top_q120(&EXP_T2[j2])) << 8;
    let m = p + (mul_high_128_called(p, e1) << 8);

    let (m1, m0) = ((m >> 64) as u64, m as u64);
    let (n, f_high) = if k <= 56 {
        let t = (56 - k) as u32;
        (u128::from(m1 >> t), ((m1 << (63 - t)) << 1) | (m0 >> t))
    } else {
        let t = (k - 56) as u32;
        ((u128::from(m1) << t) | u128::from(m0 >> (64 - t)), m0 << t)
    };
    let eps = (((n + 1) << 42) >> 64) as u64 + 1;
    settle_word(n, f_high, eps, round).ok_or(Unsettled)
}

/// `exp(h) - 1` for `0 <= h < 2^-16.5` at Q184, within `2^-150.3`: eight
/// Taylor terms as `h + h^2 E_2`, the dropped tail below `2^-166.9`.
///
/// `E_k = 1/k! + h E_(k+1)` runs from `E_8` at Q68 down to `E_6`, within
/// `2^-67.5`, then `E_5` at Q69 and `E_4` at Q128 from exact products of
/// the Q79 word of `h` (each within its own truncation plus the previous
/// error times `h` plus the word's `2^-79` times the previous value:
/// `2^-68.6` and `2^-84.4`), then `E_3` and `E_2` at Q128 from top-word
/// products under seven units of `2^-143` short: within `2^-100.9` and
/// `2^-117.4`. The square `h^2` from the top-word product of `h` at Q143
/// is within eight units of `2^-158`; `h^2 E_2` from one more top-word
/// product at Q158 is within seven units plus `h^2`'s error times `E_2`
/// plus `E_2`'s times `h^2`, under `2^-150.39`. The stored factorials'
/// exact errors are counted.
#[inline(always)]
fn expm1(h: &Q184) -> Q184 {
    debug_assert!(h[2] < 1 << 40);
    let a = head(h);
    let h128 = as_q143(h);
    let e = ((1u128 << 68) / 40320) as u64;
    let e = ((1u128 << 68) / 5040) as u64 + (mul_high_approx(a, e) >> 15);
    let e = ((1u128 << 68) / 720) as u64 + (mul_high_approx(a, e) >> 15);
    let e = ((1u128 << 69) / 120) as u64 + (full(a, e) >> 78) as u64;
    let e = u128::MAX / 24 + (full(a, e) >> 20);
    let e = u128::MAX / 6 + (mul_high_128_called(h128, e) >> 15);
    let e = u128::MAX / 2 + (mul_high_128_called(h128, e) >> 15);
    let h2 = mul_high_128_called(h128, h128);
    add(h, &from_q::<26>(mul_high_128_called(h2, e))).0
}

/// `x (1 + d)` for `x` in `[1, 2)` and `d < 2^-5` at Q184, within
/// `2^-173.1`: the top word of `d` (Q69) times `x`, exact but for the
/// lowest partial's top word (under three units of `2^-189` short) and
/// the shift down to Q184 (one unit), the rest times the top two limbs of
/// `x` by a top-word product under seven units of `2^-176` short.
#[inline(always)]
fn mul_one_plus(x: &Q184, d: &Q184) -> Q184 {
    debug_assert!(d[2] < 1 << 51);
    let d_hi = (d[2] << 13) | (d[1] >> 51);
    let d_lo = join(d[1] & ((1 << 51) - 1), d[0]);
    let (h1, l1) = widening_mul(x[1], d_hi);
    let (h2, l2) = widening_mul(x[2], d_hi);
    let (p0, c1) = mul_high_approx(x[0], d_hi).overflowing_add(l1);
    let (p1, c2) = h1.overflowing_add(l2);
    let (p1, c3) = p1.overflowing_add(u64::from(c1));
    let hi = shr(&[p0, p1, h2 + u64::from(c2) + u64::from(c3)], 5);
    let lo = mul_high_128_called(d_lo, join(x[2], x[1]));
    add(&add(x, &hi).0, &from_q::<8>(lo)).0
}

/// Tier 2 settled on the fraction's top word; [`middle_full`] retries a
/// refusal at the start of [`last`], as in the ln tier and for the same
/// reason.
#[inline(never)]
pub(super) fn middle(negative: bool, a: u128, scale: &Scale, round: Round) -> Result<Option<u128>, Unsettled> {
    middle_at::<false>(negative, a, scale, round)
}

/// Tier 2 settled on the whole fraction against the exact `eps`.
#[cold]
#[inline(never)]
pub(super) fn middle_full(negative: bool, a: u128, scale: &Scale, round: Round) -> Result<Option<u128>, Unsettled> {
    middle_at::<true>(negative, a, scale, round)
}

/// Tier 2 in base e like [`fast`], on the fraction's top word or, with
/// `FULL`, on the whole fraction.
#[inline(always)]
fn middle_at<const FULL: bool>(
    negative: bool,
    a: u128,
    scale: &Scale,
    round: Round,
) -> Result<Option<u128>, Unsettled> {
    let c = &scale.inv_pow10;
    let (a1, a0) = ((a >> 64) as u64, a as u64);
    let x = {
        let (h02, l02) = widening_mul(a0, c[2]);
        let (h11, l11) = widening_mul(a1, c[1]);
        let (h03, l03) = widening_mul(a0, c[3]);
        let (h12, l12) = widening_mul(a1, c[2]);
        let (h04, l04) = widening_mul(a0, c[4]);
        let (h13, l13) = widening_mul(a1, c[3]);
        let (x0, c1) = mul_high_approx(a0, c[1]).overflowing_add(mul_high_approx(a1, c[0]));
        let (x0, c2) = x0.overflowing_add(l02);
        let (x0, c3) = x0.overflowing_add(l11);
        let (x1, c4) = h02.overflowing_add(h11);
        let (x1, c5) = x1.overflowing_add(l03);
        let (x1, c6) = x1.overflowing_add(l12);
        let (x1, c7) = x1.overflowing_add(u64::from(c1) + u64::from(c2) + u64::from(c3));
        let x2 = h03 + h12 + l04 + l13 + u64::from(c4) + u64::from(c5) + u64::from(c6) + u64::from(c7);
        debug_assert!(h04 == 0 && h13 == 0 && a1.checked_mul(c[4]) == Some(0));
        [x0, x1, x2]
    };
    // s here is s + 2 ln 2; under ln 2 the result is under 1/2.
    let biased = &scale.exp_ln10_biased;
    let s = if negative {
        let (s, borrow) = sub(biased, &x);
        if borrow || cmp(&s, &UNDER_HALF_Q184).is_lt() {
            return Ok(Some(u128::from(matches!(round, Round::Ceil))));
        }
        s
    } else {
        add(biased, &x).0
    };
    debug_assert!(s[2] >> 56 < 218);
    let j = ((s[2] >> 32) * LOG2E_Q31) >> 39;
    let k = (j >> (2 * INDEX_BITS)) as i32 - 2;
    let j1 = ((j >> INDEX_BITS) & INDEX_MASK as u64) as usize;
    let j2 = (j & INDEX_MASK as u64) as usize;
    let (h, borrow) = sub(&s, &mul_small_shr(&LN2, j, 2 * INDEX_BITS));
    debug_assert!(!borrow);

    let e1 = expm1(&h);
    let t2 = &EXP_T2[j2];
    let d2: Q184 = [t2[0], t2[1], t2[2] & MASK56];
    // d2 e1 from d2 at Q136 and e1 at Q144, so the product is at Q152.
    let cross = mul_high_128_called(
        (join(d2[2], d2[1]) << 16) | u128::from(d2[0] >> 48),
        (join(e1[2], e1[1]) << 24) | u128::from(e1[0] >> 40),
    );
    let d = add(&add(&d2, &e1).0, &from_q::<32>(cross)).0;
    let m = mul_one_plus(&EXP_T1[j1], &d);

    let (hi, lo) = (join(m[2], m[1]), join(m[1], m[0]));
    let (n, f_top) = if k <= 120 {
        let b = (120 - k) as u32;
        (hi >> b, if b < 64 { (lo >> b) as u64 } else { (hi >> (b - 64)) as u64 })
    } else if k <= 128 {
        let u = (k - 120) as u32;
        let n = (hi << u) | u128::from(m[0] >> (64 - u));
        match hi >> (128 - u) {
            0 => (n, m[0] << u),
            1 if n == 0 => return Err(Unsettled),
            _ => return Ok(None),
        }
    } else {
        return Ok(None);
    };
    // The whole fraction is m 2^(k + 8) modulo 2^192.
    let settled = if FULL {
        settle(n, &shl(&m, (k + 8) as u32), &scaled(n.saturating_add(1), 48), round)
    } else {
        settle_word(n, f_top, ((n.saturating_add(1) >> 80) as u64) + 1, round)
    };
    settled.ok_or(Unsettled)
}

const SQUARINGS: u32 = 16;
const LAST_TERMS: usize = 17;

static FACT_Q376: [Gen; LAST_TERMS + 1] = {
    let mut table = [[0; 6]; LAST_TERMS + 1];
    let mut value = one();
    let mut k = 1;
    while k <= LAST_TERMS {
        value = div_small_const(&value, k as u64);
        table[k] = value;
        k += 1;
    }
    table
};

/// `exp(h)` for `0 <= h <= ln 2` at Q376, within `2^-333.3` relative:
/// seventeen Taylor terms on `h / 2^16`, then sixteen squarings.
#[inline(never)]
pub(super) fn exp_q376(h: &Gen) -> Gen {
    let small = shr(h, SQUARINGS);
    let mut p = FACT_Q376[LAST_TERMS];
    let mut k = LAST_TERMS - 1;
    while k > 0 {
        p = add(&FACT_Q376[k], &mul_shr(&small, &p, 8)).0;
        k -= 1;
    }
    let mut e = add(&one(), &mul_shr(&small, &p, 8)).0;
    for _ in 0..SQUARINGS {
        e = mul_shr(&e, &e, 8);
    }
    e
}

/// Tier 3 in base 2 from the Q504 constant, after [`middle_full`] has had
/// its try.
#[inline(never)]
pub(super) fn last(
    negative: bool,
    a: u128,
    scale: &Scale,
    round: Round,
) -> Result<Option<u128>, Unsettled> {
    if let Ok(decided) = middle_full(negative, a, scale, round) {
        return Ok(decided);
    }
    let y = mul_by_u128::<8, 10>(&scale.log2e_over_pow10, a);
    debug_assert!(y[8] == 0 && y[9] == 0);
    let mut f: Gen = [y[2], y[3], y[4], y[5], y[6], y[7]];
    let k = (f[5] >> 56) as i32;
    f[5] &= MASK56;
    let k = if !negative {
        k
    } else if is_zero(&f) {
        -k
    } else {
        f = sub(&one(), &f).0;
        -k - 1
    };
    let m = exp_q376(&mul_shr(&f, &LN2_GEN, 8));
    let product = mul_by_u128::<6, 8>(&m, scale.pow10);
    let shifted = if k > 0 {
        if !is_zero(&shr(&product, 512 - k as u32)) {
            return Ok(None);
        }
        shl(&product, k as u32)
    } else {
        shr(&product, k.unsigned_abs())
    };
    let (n, f) = match split::<6, 8>(&shifted) {
        Split::Fits(n, f) => (n, f),
        Split::AtLimit => return Err(Unsettled),
        Split::Overflow => return Ok(None),
    };
    let eps = scaled(n.saturating_add(1), 54);
    if n == 0 && matches!(cmp(&f, &eps), core::cmp::Ordering::Less) {
        return Ok(Some(u128::from(matches!(round, Round::Ceil))));
    }
    settle(n, &f, &eps, round).ok_or(Unsettled)
}
