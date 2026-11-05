//! Natural logarithm and exponential for decimal `Fix` types, correctly
//! rounded: the same promise `mul_div_floor`, `_ceil` and `_round` make.
//!
//! # Strategy
//!
//! Ziv's: each tier computes the result with a certified error bound and
//! returns when the true value is provably on one side of the rounding
//! boundary; otherwise the next, wider tier recomputes. Three tiers:
//!
//! 1. `u128` arithmetic on Q120/Q128 words, bound `2^-81` on `ln v` and
//!    `2^-86` relative on `exp`; runs for `E <= 22` (ln) and `E <= 24`
//!    (exp), where that bound is still well inside an ulp.
//! 2. Three limbs at Q184, bound `2^-145` on `ln v` and `2^-144` relative
//!    on `exp`; the normal path above those exponents and tier 1's
//!    fallback below them.
//! 3. Six limbs at Q376 from compile-time constants alone, bound `2^-329`
//!    on `ln v` and `2^-330` relative on `exp`.
//!
//! Tiers 1 and 2 share the compile-time tables in [`tables`]: two
//! eight-bit reduction levels per function, so the series argument is
//! below `2^-16`. Before any tier, the exact cases `ln(10^E) = 0` and
//! `exp(0) = 10^E` return, and so do arguments within `tiny` of them,
//! where `2 tiny^2 < 10^E`: there `exp(x) 10^E = 10^E + bits + e` with
//! `e` in `(0, 1/2)` (`10^E - bits + e` for `x < 0`), and `ln v 10^E` is
//! `d - e` with `e` in `(0, 1/4)` for `v = 1 + d / 10^E` and `-(d + e)`
//! with `e` in `(0, 1/2)` for `v = 1 - d / 10^E`, so every rounding
//! follows from the sign.
//!
//! `ln(bits / 10^E)` is `s ln 2 - E ln 10 + ln(m / 2^127)` for the
//! mantissa `m = bits << lz`, `s = 127 - lz`, taken as `(s + 2) ln 2 -
//! (E ln 10 + 2 ln 2) + ln m` so that ln and exp share one stored
//! constant, `ln(4 10^E)`; `ln m` reduces through
//! `m R1[j1] R2[j2] = 1 + r` with `T[j] = -ln(R[j] / 2^31)` stored for the
//! reciprocals as stored, so the reductions are exact up to the products'
//! own truncation. `exp(bits / 10^E) 10^E` is `exp(s)` for `s = bits /
//! 10^E + E ln 10`, with `bits / 10^E` one multiply by the compile-time
//! constant `2^312 / 10^E`, and `s = J ln 2 / 2^16 + h` with `J = k 2^16
//! + j1 2^8 + j2` splits, as in Tang's method, into the shift `2^k`, two
//! table factors and `exp(h)`; the last tier works in base 2 from
//! `log2(e) / 10^E` instead. The decimal scale therefore costs one
//! constant subtraction (ln) or addition (exp), one constant multiply, and
//! for ln one multiply by `10^E` at the end, and no division.
//!
//! # Stored values
//!
//! Table entries, `ln 2` and the Q184 view of `E ln 10 + 2 ln 2` are
//! each within one unit of `2^-184` of the truth, `2^312 / 10^E` within
//! one unit of `2^-312`, and the table of `(s + 2) ln 2` within one unit
//! at Q184 and at Q120. The Q376 generator constants are series sums
//! whose truncations accumulate: `ln 2` is within `2^-369`, `E ln 10 + 2
//! ln 2` within `2^-361.9` for `E <= 38`, and `log2(e) / 10^E` at Q504
//! within `2^-495 / 10^E + 2^-503`, so its Q312 view (top five limbs) is
//! within `2^-311.99` and its Q184 view within `2^-184 + 2^-495 / 10^E`.
//! An out-of-tree checker recomputes every one of these from the dump in
//! [`tables`] against Python's `decimal` and fails on any excess.
//!
//! # Tier 1, ln (absolute, on `ln v`)
//!
//! Every product is a native one. The mantissa `m = mh 2^64 + ml` enters
//! as `ln(m / 2^127) = ln(mh / 2^63) + log1p(c1)` with `c1 = ml / (mh
//! 2^64) < 2^-63`, computed from the top 32 bits of each limb by one
//! division: within nine units of `2^-96` (`2^-92.8`), the dropped
//! `c1^2 / 2` below `2^-127`. The level-1 product `x1 = mh R1 / 2^94` is
//! exact and its low 32 bits fold the same way, `ln x1 = ln(hi1 / 2^62) +
//! log1p(c2)` with `c2 = lo1 / (hi1 2^32) < 2^-62` within nine units of
//! `2^-95` (`2^-91.8`), its square below `2^-125`. The level-2 product
//! `x2 = hi1 R2 / 2^93 = 1 + r` is exact as well, so the residue carries
//! no reduction error, and `r < 2^-16 (1 + 2^-11)` by the bound on `x2`
//! given under tier 2. Series: `log1p(r) = r - r^2/2 + r^3/3 - r^4/4` with
//! `r` exact at Q120 and the tail from `r` truncated to Q79 (`2^-79` on
//! `r`): the square's three partials and its halving lose under
//! `2^-92.6`, the cube's under `2^-93.9`, the fourth power's under
//! `2^-93.4`, so the tail is within `2^-91.6`; the dropped `r^5/5` is
//! below `2^-82.32`. Constants: `(s + 2) ln 2` from its table, `E ln 10 +
//! 2 ln 2` and the two reduction entries are each within one unit of
//! `2^-120`. Total below `2^-82.3`; the bound used is `2^-81`, so `eps =
//! 10^E 2^47` in units of `2^-128` output ulps. While `10^E < 2^64` the product by
//! `10^E` keeps its top 128 bits from three native products, at most
//! three units of `2^-56` output ulps low, and the fraction is settled on
//! its top word with `ln_eps_word = ceil(eps / 2^64) + 2^11`, which
//! covers that and the word's own truncation (under `2^10` of its units
//! together); for `E` from 20 to 22 the product by `5^E < 2^52` is exact
//! in three words and the cut sits at bit `120 - E`. Assumes `ln_eps <
//! 2^126` so the word cannot wrap, which `E <= 22` gives and a
//! compile-time assert on [`LN_TIER1_MAX_EXP`] holds.
//!
//! # Tier 1, exp (relative)
//!
//! In base e: `exp(x) 10^E = exp(s)` for `s = E ln 10 + x`, with `x = bits
//! 2^184 / 10^E` from the truncated constant within `128 10^E 2^-184 + 3 *
//! 2^-120` (`2^-97.3` at the last exponent the tier runs at, `2^-117.2`
//! at N18) and `E ln 10 + 2 ln 2` at Q120 within `2^-120`. The tier works
//! on `s + 2 ln 2`, within `2^-97.2` of the truth, which is positive for every
//! result it settles; the index `J = floor((s + 2 ln 2) 2^16 / ln 2)` from
//! the top 32 bits of that sum and `floor(2^31 log2 e)` rounds down by
//! less than `2^-6.3`, so the residue `h = s + 2 ln 2 - J ln 2 / 2^16` is
//! never negative and below `2^-16.53 (1 + 2^-6.3) < 2^-16.5`; the step
//! `J ln 2 / 2^16` at Q120 is within `2^-119`. `h` is thus within
//! `2^-97.1`, which enters `exp(h)` as that relative error. The result's
//! binary exponent is `k = J / 2^16 - 2`, and `j1` and `j2` are the next
//! two bytes of `J`. Series: `h + h^2/2 + h^3/6 + h^4/24`, dropped tail
//! below `h^5/120 < 2^-89.4`, truncations below `2^-93.5` (the fourth
//! power from the top word of the square alone is within `2^-30` relative
//! on a term below `2^-70.6`). The two 128-bit truncated products (`T1
//! T2`, then by the series) are each under seven units of `2^-112` short
//! (`2^-109.1`). Total below `2^-89.3` relative on the mantissa `m` in
//! `[1, 2 (1 + 2^-15))`; the bound used is `2^-86`,
//! applied to `n + 1` where the result `m 2^k` is `n + f`. The cut at
//! bit `120 - k` of the Q120 mantissa is exact, and the fraction is
//! settled on its top word with `eps = floor((n + 1) 2^42 / 2^64) + 1`,
//! the one unit covering the bits below the word. Assumes `10^E < 2^86`
//! (a compile-time assert on [`EXP_TIER1_MAX_EXP`]) and `bits <
//! 128 10^E` (the caller's prelude), which keep `s + 2 ln 2` under `185`
//! so the products that form the step fit (a `debug_assert`). When the
//! sum is below `ln 2 - 2^-96` the result is under `1/2` and rounds from
//! the sign alone; `k > 83` is unsettled so that `n + 1 <= 2^85` keeps
//! the bound inside the word (both checked at run time).
//!
//! # Tier 2, ln (absolute, on `ln v`)
//!
//! The mantissa `m / 2^127` in `[1, 2)` is exact at Q184. Each reciprocal
//! is stored at Q31, rounded up and pushed four more units, so between one
//! and five units of `2^-31` above `1 / c_j`; after the truncation of a
//! product by at most one unit of `2^-184` the reduced value stays at or
//! above 1 (a `debug_assert`), and that truncation lowers the logarithm
//! by at most `2^-184`. Level 1 leaves `x_1 < 1 + 2^-8 + 2^-27.7`, so the
//! level-2 index saturates at 255 for `x_1 - 1` up to `2^-27.7` past
//! `2^-8`; with the push there, `x_2 < (1 + 2^-16 + 2^-27.7)(1 +
//! 2^-28.6)`, so the residue is `r < 2^-16 (1 + 2^-11)`. `ln::log1p` is
//! within `2^-147`; the two entries, `(s + 2) ln 2` and `E ln 10 + 2 ln
//! 2`, each within one unit, add under `2^-181`. Total below `2^-146.99`;
//! the bound used is `2^-145`, so `eps = 10^E 2^47` in units of `2^-192`,
//! settled on the fraction's top word with `floor(eps / 2^128) + 1`, the
//! one unit covering the floor and the two limbs below the word; tier 3
//! begins by retrying that settle on the whole fraction against the
//! exact `eps`, so only a result inside this bound pays for its Q376
//! work. Assumes `bits < 2^128` (so `s <= 127` and the Q184 sums stay below
//! `2^8`), `E <= 38` (`eps < 2^192`, one ulp, holds since `10^38 2^47 <
//! 2^192`), and `r 2^56 <= 2^40` for the series' word splits (a
//! `debug_assert` in [`head`] and [`as_q143`]).
//!
//! # Tier 2, exp (relative)
//!
//! In base e like tier 1: `exp(x) 10^E = exp(s)` for `s = E ln 10 + x`,
//! with `x = a 2^312 / 10^E` from the truncated Q312 constant, within
//! `2^127 2^-312 + 2^-184 < 2^-183.4` for the full product's limbs 2 to 4,
//! and seven more units of `2^-184` for what the tier leaves out of the
//! product: the lowest partial and the low words of the next two, under
//! three units together, and the two top-word partials, each at most two
//! units short; `E ln 10 + 2 ln 2` at Q184 is within one unit. The tier
//! works on `s + 2 ln 2`, within `2^-180.5`,
//! which is positive for every result it settles; below `ln 2 - 2^-176`
//! the result is under `1/2` and rounds from the sign. `J = floor((s + 2
//! ln 2) 2^16 / ln 2)` comes from the top 32 bits of the sum and
//! `floor(2^31 log2 e)` as in tier 1, so the residue `h = s + 2 ln 2 - J
//! ln 2 / 2^16` is never negative and below `2^-16.5`; the step from the
//! Q184 `ln 2` is within `J 2^-200 + 2^-184 < 2^-175.6` since `J < 2^24.4`.
//! `h` is thus within `2^-175.5`, which enters `exp(h)` as that relative
//! error. `k = J / 2^16 - 2` is the result's binary exponent, `j1` and `j2`
//! the next two bytes of `J`. `exp::expm1` is within `2^-150.3` of
//! `exp(h) - 1`. The level-2 entry `1 + d2` and the series factor combine
//! as `(1 + d2)(1 + e1) = 1 + d` with `d = d2 + e1 + d2 e1`, where the
//! cross term (below `2^-24`) comes from one top-word product of the top
//! 128 bits of each factor at Q152: under seven units short plus each
//! factor's truncation times the other, under nine units (`2^-148.8`), so
//! `d` is within `2^-148.4`. Then `T1 (1 + d)` goes through
//! `exp::mul_one_plus` once, within `2^-173.1` with `d < 2^-5` (a
//! `debug_assert`), and the two entries are within one unit, so the
//! mantissa `m` is within `2^-148.4` relative of `exp(s) / 2^k`. The
//! result `m 2^k` is a shift, exact. Total below `(n + 1) 2^-148.4`; the
//! bound used is `(n + 1) 2^-144`, so `eps = (n + 1) 2^48`
//! in units of `2^-192`, with `n + 1` saturated at `2^128 - 1`, which still
//! exceeds the error, settled on the fraction's top word with `floor(eps /
//! 2^128) + 1` and retried on the whole fraction at the start of tier 3,
//! as in tier 2 ln. `a < 128 10^E` (the caller's prelude) keeps
//! `s + 2 ln 2` under `218`, inside the eight integer bits (a
//! `debug_assert`); `k >= -2` keeps the cut inside the mantissa's three
//! limbs, and above `k = 128` the result exceeds `2^128` for certain since
//! `m >= 1`.
//!
//! # Tier 3, exp (relative)
//!
//! `y` from the whole Q504 constant is within `2^-375` for `a < 2^127`.
//! `h = f ln 2` inherits `ln 2`'s `2^-369` plus one truncation.
//! `exp::exp_q376` computes `exp(h)` for `h <= ln 2` as seventeen Taylor
//! terms of `h / 2^16 < 2^-16.5` (dropped tail below `2^-349.5`, Horner
//! truncations below `2^-374.9`) followed by sixteen squarings, each
//! doubling the relative error and adding one unit: within `2^-333.3`
//! relative. Total below `(n + 1) 2^-333.2`; the bound used is
//! `(n + 1) 2^-330`, so `eps = (n + 1) 2^54` in units of `2^-384`.
//!
//! # Tier 3, ln (absolute, on `ln v`)
//!
//! With `y0` tier 2's mantissa logarithm, `ln(m / 2^127) = y0 + log1p(d)`
//! for `1 + d = (m / 2^127) exp(-y0)` exactly, and `|d| < 2^-146.9` by
//! tier 2's bound. `exp(-y0)` comes from `exp_q376(ln 2 - y0) / 2`, which
//! needs `0 <= y0 < ln 2` (a `debug_assert` on the subtraction's borrow),
//! so `d` is computed within `2^-333.28`; `log1p(d) = d - d^2/2` with the
//! dropped `d^3/3` below `2^-442` and the square from the top word of `d`
//! within `2^-355`. `(s + 2) ln 2` and `E ln 10 + 2 ln 2` add under
//! `2^-360.9`. Total below `2^-333.2`; the bound used is `2^-329`, so
//! `eps = 10^E 2^55` in units of `2^-384`.
//!
//! # Limits
//!
//! A product whose integer part is exactly `2^128` is passed to the next
//! tier rather than declared an overflow, since the true value may round
//! below the limit under floor; tier 3 reports it unsettled, so the
//! method returns `None`. No exp result comes that close: exp is
//! increasing, and the nearest argument at any `E`, at N38, lands `0.735`
//! units below `2^128`. Above `2^128` the overflow is certain. A
//! magnitude of exactly `2^127` with a negative sign is `i128::MIN`.
//!
//! # What tier 3 cannot settle
//!
//! A result within its bound of a rounding boundary would leave tier 3
//! undecided and the method returns `None`. Results are transcendental
//! (the exact cases return before any tier), so none sits on a boundary,
//! but no exhaustive search over `2^128` inputs exists to rule out one that
//! close. On the usual model that boundary distances are uniform, the
//! per-call probability is `2 eps / ulp`: for ln `2^-266` at N18 and
//! `2^-202` at N38; for exp `2^-203` at N38 for results near `2^126`
//! units. The expected number of such inputs across a whole 128-bit type
//! is `2^128` times that, `2^-74` at N38. This is the Ziv guarantee as
//! every such library gives it, not a proof.
//!
//! # Tier rates
//!
//! A tier is undecided when the true result lies within its bound of a
//! boundary, so the rate is `2 eps / ulp`; the middle tiers' top-word
//! test refuses more often, by its one-unit floor, but tier 3 retries
//! those refusals on the whole fraction first, so the rates below are
//! those of paying for the next tier. Tier 1: `2^-20` for ln at N18 and
//! `2^-7` at N22; for exp `(n + 1) 2^-85`, `2^-24` for results near
//! `10^18` units and `2^-4` near `10^24`. Tier 2: `10^E 2^-144`, `2^-24`
//! for ln at N36 and `2^-18` at N38; `(n + 1) 2^-143` for exp, `2^-22` at
//! N36 for results near `10^36` units and `2^-16` near `10^38`.

mod exp;
mod ln;
mod tables;
mod wide;

use crate::kernel::{widening_mul, Round};
use tables::{Q184, LN10_GEN, LN2_GEN, LOG2E_Q504};
use wide::{add, cmp, div_small_const, mul_small, shl, shr, sub, Limbs};

/// Compile-time constants of one decimal scale `10^exp`, evaluated once per
/// `Fix` instantiation through an associated const. Exists only while
/// `10^exp` fits `u128`, so an entry point cannot forget the check.
pub(crate) struct Scale {
    exp: u32,
    pow10: u128,
    /// `exp ln 10 + 2 ln 2`, which is `ln(4 10^exp)`.
    exp_ln10_biased: Q184,
    exp_ln10_biased_low: Q184,
    /// `log2(e) / 10^exp` at Q504.
    log2e_over_pow10: Limbs<8>,
    /// `2^312 / 10^exp`, truncated.
    inv_pow10: Limbs<5>,
    /// `128 * 10^exp`, saturated: exp arguments at or beyond it overflow
    /// or vanish without any arithmetic.
    exp_limit: u128,
    ln_eps_word: u64,
    tiny: u128,
    tiny_lo: u128,
}

const fn isqrt(v: u128) -> u128 {
    if v < 2 {
        return v;
    }
    let mut x = 1u128 << (128 - v.leading_zeros()).div_ceil(2);
    loop {
        let next = (x + v / x) / 2;
        if next >= x {
            return x;
        }
        x = next;
    }
}

/// Largest exponents at which tier 1 runs: above them its bound is too
/// close to an ulp for the attempt to pay.
const LN_TIER1_MAX_EXP: u32 = 22;
const EXP_TIER1_MAX_EXP: u32 = 24;
const _: () = assert!(10u128.pow(LN_TIER1_MAX_EXP) << 47 < 1 << 126);
const _: () = assert!(10u128.pow(EXP_TIER1_MAX_EXP) < 1 << 86);

impl Scale {
    pub const fn new(exp: u32) -> Option<Self> {
        if exp > 38 {
            return None;
        }
        let pow10 = 10u128.pow(exp);
        let biased = add(&mul_small(&LN10_GEN, exp as u64).0, &shl(&LN2_GEN, 1)).0;
        let tiny = isqrt((pow10 - 1) / 2);
        let (high, low) = if exp > 19 {
            (10u64.pow(exp - 19), 10u64.pow(19))
        } else {
            (1, 10u64.pow(exp))
        };
        Some(Self {
            exp,
            pow10,
            exp_ln10_biased: [biased[3], biased[4], biased[5]],
            exp_ln10_biased_low: [biased[0], biased[1], biased[2]],
            log2e_over_pow10: div_small_const(&div_small_const(&LOG2E_Q504, low), high),
            inv_pow10: {
                let q = div_small_const(&div_small_const(&tables::one::<8>(), low), high);
                [q[3], q[4], q[5], q[6], q[7]]
            },
            exp_limit: pow10.saturating_mul(128),
            ln_eps_word: ((pow10.saturating_mul(1 << 47) >> 64) as u64).saturating_add(1 + (1 << 11)),
            tiny,
            tiny_lo: pow10 - tiny,
        })
    }
}

#[inline(always)]
const fn top_q120<const N: usize>(entry: &Limbs<N>) -> u128 {
    join(entry[N - 1], entry[N - 2])
}

struct Unsettled;

/// The top word of `a * b` from three of the four partial products: at
/// most two units below the exact word, never above.
#[inline(always)]
fn mul_high_approx(a: u64, b: u64) -> u64 {
    let (a_high, a_low) = (a >> 32, a & MASK32);
    let (b_high, b_low) = (b >> 32, b & MASK32);
    a_high * b_high + ((a_high * b_low) >> 32) + ((a_low * b_high) >> 32)
}

#[inline(always)]
const fn join(high: u64, low: u64) -> u128 {
    ((high as u128) << 64) | low as u128
}

#[inline(always)]
fn full(a: u64, b: u64) -> u128 {
    let (high, low) = widening_mul(a, b);
    join(high, low)
}

/// `floor(a * b / 2^128)` from three products: the dropped low-by-low
/// partial (under one unit) and the two [`mul_high_approx`] cross terms
/// (each under three units short of its real value) leave it under seven
/// units below `a b / 2^128`, so at most six below the exact word, never
/// above. Six is reached (a test pins it), so every bound derived from
/// this helper counts seven units per call.
#[inline(always)]
fn mul_high_128_approx(a: u128, b: u128) -> u128 {
    let (a1, a0) = ((a >> 64) as u64, a as u64);
    let (b1, b0) = ((b >> 64) as u64, b as u64);
    let (hh_high, hh_low) = widening_mul(a1, b1);
    join(hh_high, hh_low)
        + u128::from(mul_high_approx(a0, b1))
        + u128::from(mul_high_approx(a1, b0))
}

/// [`mul_high_128_approx`] through a call: SBF saves `r6` to `r9` across
/// one, so the caller's live values stay in registers instead of being
/// spilled around the eleven partial products. The exp paths measure
/// cheaper this way, the ln series does not.
#[inline(never)]
fn mul_high_128_called(a: u128, b: u128) -> u128 {
    mul_high_128_approx(a, b)
}

/// `(a * m) >> bits` for `0 < bits <= 64`, keeping the carry limb.
#[inline(always)]
fn mul_small_shr<const N: usize>(a: &Limbs<N>, m: u64, bits: u32) -> Limbs<N> {
    let (product, carry) = mul_small(a, m);
    let mut out = shr(&product, bits);
    out[N - 1] |= carry << (64 - bits);
    out
}

#[inline(always)]
fn mul_by_u128<const N: usize, const M: usize>(a: &Limbs<N>, b: u128) -> Limbs<M> {
    debug_assert_eq!(M, N + 2);
    let (low, low_carry) = mul_small(a, b as u64);
    let (high, high_carry) = mul_small(a, (b >> 64) as u64);
    let mut out = [0; M];
    let mut carry = false;
    for i in 0..M {
        let l = match i.cmp(&N) {
            core::cmp::Ordering::Less => low[i],
            core::cmp::Ordering::Equal => low_carry,
            core::cmp::Ordering::Greater => 0,
        };
        let h = match i {
            0 => 0,
            _ if i <= N => high[i - 1],
            _ => high_carry,
        };
        let (sum, c1) = l.overflowing_add(h);
        let (sum, c2) = sum.overflowing_add(u64::from(carry));
        out[i] = sum;
        carry = c1 | c2;
    }
    out
}

/// The integer part of a product in the eight-integer-bit format of
/// `M = N + 2` limbs, with its fraction moved up to `Q(64 N)`.
enum Split<const N: usize> {
    Fits(u128, Limbs<N>),
    AtLimit,
    Overflow,
}

#[inline(always)]
fn split<const N: usize, const M: usize>(product: &Limbs<M>) -> Split<N> {
    debug_assert_eq!(M, N + 2);
    let (top, mid, low) = (product[N + 1], product[N], product[N - 1]);
    if top >> 56 != 0 {
        return if top == 1 << 56 && mid == 0 && low >> 56 == 0 {
            Split::AtLimit
        } else {
            Split::Overflow
        };
    }
    let n = (u128::from(top) << 72) | (u128::from(mid) << 8) | u128::from(low >> 56);
    let mut fraction = [0; N];
    fraction.copy_from_slice(&product[..N]);
    fraction[N - 1] &= MASK56;
    Split::Fits(n, shl(&fraction, 8))
}

#[inline(always)]
fn scaled<const N: usize>(count: u128, shift: u32) -> Limbs<N> {
    let mut out = [0; N];
    out[0] = count as u64;
    out[1] = (count >> 64) as u64;
    shl(&out, shift)
}

const MASK32: u64 = u32::MAX as u64;
const MASK56: u64 = (1 << 56) - 1;

/// The top word of a Q184 value below `2^-15`, at Q79.
#[inline(always)]
fn head(x: &Q184) -> u64 {
    debug_assert!(x[2] < 1 << 41);
    (x[2] << 23) | (x[1] >> 41)
}

/// A Q184 value below `2^-15` as a `u128` at Q143.
#[inline(always)]
fn as_q143(x: &Q184) -> u128 {
    debug_assert!(x[2] < 1 << 41);
    (u128::from(x[2]) << 87) | (u128::from(x[1]) << 23) | u128::from(x[0] >> 41)
}

/// A `u128` at `Q(184 - S)` as Q184 limbs; it must be below `2^(192 - S)`.
#[inline(always)]
fn from_q<const S: u32>(x: u128) -> Q184 {
    [(x as u64) << S, (x >> (64 - S)) as u64, (x >> (128 - S)) as u64]
}

/// `floor`, `ceil` or nearest of `n + f / 2^(64 N)` whose true value sits
/// within `eps` of that; `None` while a rounding boundary is within reach,
/// `Some(None)` when the rounded value exceeds `u128`. Ties cannot occur:
/// every value that reaches here is transcendental.
#[inline(always)]
fn settle<const N: usize>(
    n: u128,
    f: &Limbs<N>,
    eps: &Limbs<N>,
    round: Round,
) -> Option<Option<u128>> {
    let (f_minus, below) = sub(f, eps);
    let (f_plus, above) = add(f, eps);
    let mut half = [0; N];
    half[N - 1] = 1 << 63;
    let up = match round {
        Round::Floor if !below && !above => false,
        Round::Ceil if !below && !above => true,
        Round::Nearest if !below && cmp(&f_minus, &half).is_gt() => true,
        Round::Nearest if !above && cmp(&f_plus, &half).is_lt() => false,
        _ => return None,
    };
    Some(if up { n.checked_add(1) } else { Some(n) })
}

/// [`settle`] on the top word of the fraction alone: `eps` is in units of
/// `2^-64` and already covers whatever the dropped low limbs could hold,
/// so every decision here is one the full compare would also make.
#[inline(always)]
fn settle_word(n: u128, f: u64, eps: u64, round: Round) -> Option<Option<u128>> {
    const HALF: u64 = 1 << 63;
    let below = f < eps;
    let above = f > u64::MAX - eps;
    let up = match round {
        Round::Floor if !below && !above => false,
        Round::Ceil if !below && !above => true,
        Round::Nearest if !below && f - eps > HALF => true,
        Round::Nearest if !above && f + eps < HALF => false,
        _ => return None,
    };
    Some(if up { n.checked_add(1) } else { Some(n) })
}

/// `ln(bits / 10^exp) * 10^exp` rounded as `round`; `bits` is positive.
/// `None` when the result does not fit `i128`, or in the unreachable case
/// the module doc describes where even the slow path cannot settle the
/// rounding.
#[inline(always)]
pub(crate) fn ln(bits: u128, scale: &Scale, round: Round) -> Option<i128> {
    if bits.wrapping_sub(scale.tiny_lo) <= 2 * scale.tiny {
        return Some(ln::tiny(bits, scale.pow10, round));
    }
    if scale.exp <= LN_TIER1_MAX_EXP {
        if let Ok(result) = ln::fast(bits, scale, round) {
            return result;
        }
    }
    if let Ok(result) = ln::middle(bits, scale, round) {
        return result;
    }
    ln::last(bits, scale, round).ok()?
}

/// `exp(bits / 10^exp) * 10^exp` rounded as `round`. `None` when the result
/// exceeds `u128`, or in the unreachable unsettled case.
#[inline(always)]
pub(crate) fn exp(bits: i128, scale: &Scale, round: Round) -> Option<u128> {
    let magnitude = bits.unsigned_abs();
    if magnitude <= scale.tiny {
        return Some(exp::tiny(bits < 0, magnitude, scale.pow10, round));
    }
    if magnitude >= scale.exp_limit {
        return if bits > 0 {
            None
        } else {
            Some(u128::from(matches!(round, Round::Ceil)))
        };
    }
    if scale.exp <= EXP_TIER1_MAX_EXP {
        if let Ok(result) = exp::fast(bits < 0, magnitude, scale, round) {
            return result;
        }
    }
    if let Ok(result) = exp::middle(bits < 0, magnitude, scale, round) {
        return result;
    }
    exp::last(bits < 0, magnitude, scale, round).ok()?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_inputs(count: usize) -> impl Iterator<Item = u128> {
        let mut x = 0x9e37_79b9_7f4a_7c15_u128;
        (0..count).map(move |_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x >> (x as u32 % 128)
        })
    }

    #[test]
    fn tiers_agree_where_both_decide() {
        type Ln = fn(u128, &Scale, Round) -> Result<Option<i128>, Unsettled>;
        type Exp = fn(bool, u128, &Scale, Round) -> Result<Option<u128>, Unsettled>;
        type Pair = (&'static [u32], usize, [Ln; 2], [Exp; 2], [usize; 2]);
        let pairs: [Pair; 2] = [
            (&[9, 18, 22, 24], 4000, [ln::fast, ln::middle], [exp::fast, exp::middle], [30_000, 40_000]),
            (&[1, 9, 18, 36, 38], 300, [ln::middle, ln::last], [exp::middle, exp::last], [3_000, 5_000]),
        ];
        for (exps, count, [ln_a, ln_b], [exp_a, exp_b], [ln_min, exp_min]) in pairs {
            let (mut ln_both, mut exp_both) = (0, 0);
            for &exp in exps {
                let scale = Scale::new(exp).unwrap();
                for bits in random_inputs(count) {
                    for round in [Round::Floor, Round::Ceil, Round::Nearest] {
                        if bits != 0 {
                            if let (Ok(a), Ok(b)) = (ln_a(bits, &scale, round), ln_b(bits, &scale, round)) {
                                ln_both += 1;
                                assert_eq!(a, b, "ln {bits} N{exp} {}", round as u8);
                            }
                        }
                        let a = bits % scale.exp_limit;
                        for negative in [false, true] {
                            if let (Ok(x), Ok(y)) = (exp_a(negative, a, &scale, round), exp_b(negative, a, &scale, round)) {
                                exp_both += 1;
                                assert_eq!(x, y, "exp {negative} {a} N{exp} {}", round as u8);
                            }
                        }
                    }
                }
            }
            assert!(ln_both > ln_min && exp_both > exp_min, "ln {ln_both} exp {exp_both}");
        }
    }

    #[test]
    fn tiny_shortcuts_match_the_tiers_at_their_threshold() {
        let rounds = [Round::Floor, Round::Ceil, Round::Nearest];
        for e in 1..=38 {
            let scale = Scale::new(e).unwrap();
            let (tiny, pow10) = (scale.tiny, scale.pow10);
            assert!(2 * tiny * tiny < pow10 && 2 * (tiny + 1) * (tiny + 1) >= pow10, "N{e}");
            for d in [tiny - 1, tiny] {
                if d == 0 {
                    continue;
                }
                for round in rounds {
                    for bits in [pow10 - d, pow10 + d] {
                        let tiers = ln::middle(bits, &scale, round).or_else(|_| ln::last(bits, &scale, round));
                        assert_eq!(tiers.ok(), Some(Some(ln::tiny(bits, pow10, round))), "ln {bits} N{e} {}", round as u8);
                    }
                    for negative in [false, true] {
                        let tiers = exp::middle(negative, d, &scale, round)
                            .or_else(|_| exp::last(negative, d, &scale, round));
                        assert_eq!(tiers.ok(), Some(Some(exp::tiny(negative, d, pow10, round))), "exp {negative} {d} N{e} {}", round as u8);
                    }
                }
            }
            for round in rounds {
                assert!(ln(pow10 + tiny + 1, &scale, round).is_some());
                assert!(ln(pow10 - tiny - 1, &scale, round).is_some());
                assert!(exp((tiny + 1) as i128, &scale, round).is_some());
                assert!(exp(-((tiny + 1) as i128), &scale, round).is_some());
            }
        }
    }

    /// Inputs whose true result lies inside the middle tier's bound of a
    /// rounding boundary, found by an out-of-tree search against the
    /// decimal oracle at less than 0.4 of the bound (ln at N36, exp at
    /// N38; at N38 the ln results it found do not fit `i128`). The actual error is below 0.25
    /// of the bound for ln and 0.06 for exp, so the computed value is
    /// inside the bound too and the tier must refuse one of the
    /// roundings: deciding there would rest on luck, not on the bound.
    #[test]
    fn middle_tier_refuses_results_inside_its_bound() {
        const LN: [u128; 6] = [
            186364145721029,
            278849288475791755794012563259478,
            38485643395260411826362222068106,
            41912455264469968826624536918616116177,
            4745050012716690239809504,
            65284637060569430856452,
        ];
        const EXP: [i128; 12] = [
            -119938054491975753941334034100356456653,
            -128416076648647888376553354367828516521,
            -13706095479233183339876563467018658734,
            -16856744391029476089208509620686668460,
            -37374284894881799394920939966256789907,
            -48046385354058851712720781767171164500,
            -85949014747045749554628773164234269104,
            116750866141903384939216505387746424756,
            42879423171258692582537405929171322858,
            52378501080224287777933743672902726152,
            61987404249000620603813733045243304831,
            6571122548045107468891155504318028935,
        ];
        const EXP36: [i128; 4] = [
            4770610981901154268084043187751647799,
            1650786403422495196276461721222797599,
            3652101599526530388672036683757094853,
            4852246607573067531990503552005603769,
        ];
        let rounds = [Round::Floor, Round::Ceil, Round::Nearest];
        let scale = Scale::new(36).unwrap();
        for bits in LN {
            assert!(rounds.iter().any(|&r| ln::middle(bits, &scale, r).is_err()), "ln {bits}");
            assert!(rounds.iter().any(|&r| ln::middle_full(bits, &scale, r).is_err()), "ln {bits} full");
        }
        for bits in EXP36 {
            let a = bits.unsigned_abs();
            assert!(rounds.iter().any(|&r| exp::middle(false, a, &scale, r).is_err()), "exp {bits} N36");
            assert!(rounds.iter().any(|&r| exp::middle_full(false, a, &scale, r).is_err()), "exp {bits} N36 full");
        }
        let scale = Scale::new(38).unwrap();
        for bits in EXP {
            let a = bits.unsigned_abs();
            assert!(rounds.iter().any(|&r| exp::middle(bits < 0, a, &scale, r).is_err()), "exp {bits}");
            assert!(rounds.iter().any(|&r| exp::middle_full(bits < 0, a, &scale, r).is_err()), "exp {bits} full");
        }
    }

    /// Inputs the fuzzer built inside the top word's one-unit floor but
    /// well outside the bound: the whole-fraction retry must decide them.
    #[test]
    fn full_settle_decides_what_the_word_refused() {
        const LN18: [u128; 4] = [
            104433643796557395681959880720076730675,
            146532079751328478592782034714026114436,
            48223670289923573992330025119396702440,
            8297649189657764808569885442027238311,
        ];
        let rounds = [Round::Floor, Round::Ceil, Round::Nearest];
        let scale = Scale::new(18).unwrap();
        let mut word_refusals = 0;
        for bits in LN18 {
            for round in rounds {
                word_refusals += usize::from(ln::middle(bits, &scale, round).is_err());
                let full = ln::middle_full(bits, &scale, round).ok().expect("inside the word's floor, outside the bound");
                assert_eq!(full, ln::last(bits, &scale, round).ok().unwrap(), "ln {bits} {}", round as u8);
            }
        }
        let a = 36913932180739566375u128;
        for round in rounds {
            word_refusals += usize::from(exp::middle(true, a, &scale, round).is_err());
            let full = exp::middle_full(true, a, &scale, round).ok().expect("inside the word's floor, outside the bound");
            assert_eq!(full, exp::last(true, a, &scale, round).ok().unwrap(), "exp {}", round as u8);
        }
        assert!(word_refusals > 0, "no input exercised the word's refusal");
    }

    /// The same for tier 1, with hits at less than 0.4 of its bound at
    /// N18 and at its last exponents, N22 (ln) and N24 (exp). Its actual
    /// error is below 0.45 of the bound for ln and 0.3 for exp.
    #[test]
    fn tier1_refuses_results_inside_its_bound() {
        const LN: [(u32, u128); 9] = [
            (18, 646535453544439447441),
            (18, 2496832231097046077),
            (18, 10599862765274006757564034),
            (18, 4607293506975691),
            (18, 710485243621367),
            (18, 3238467200816),
            (22, 547198238989013996208625),
            (22, 6984260657134927307914149918),
            (22, 3708656880553412142925596891985318),
        ];
        const EXP: [(u32, i128); 8] = [
            (18, 2857050424094096196),
            (18, 4891180995561013436),
            (18, 3451409090561628814),
            (18, 4909336948095243933),
            (18, 1783714594075163242),
            (24, -1397672107270246186389862),
            (24, 3251489259182021884412723),
            (24, 3683083008842114588630930),
        ];
        let rounds = [Round::Floor, Round::Ceil, Round::Nearest];
        for (e, bits) in LN {
            let scale = Scale::new(e).unwrap();
            assert!(rounds.iter().any(|&r| ln::fast(bits, &scale, r).is_err()), "ln {bits} N{e}");
        }
        for (e, bits) in EXP {
            let scale = Scale::new(e).unwrap();
            let a = bits.unsigned_abs();
            assert!(rounds.iter().any(|&r| exp::fast(bits < 0, a, &scale, r).is_err()), "exp {bits} N{e}");
        }
    }

    /// The exact cases sit on a boundary, so the last tier must refuse
    /// their floor and ceil: the only input-level check on its bound.
    #[test]
    fn last_tier_refuses_the_exact_cases() {
        for e in 1..=38 {
            let scale = Scale::new(e).unwrap();
            for round in [Round::Floor, Round::Ceil] {
                assert!(ln::last(scale.pow10, &scale, round).is_err(), "ln N{e}");
                assert!(exp::last(false, 0, &scale, round).is_err(), "exp N{e}");
            }
            assert_eq!(ln::last(scale.pow10, &scale, Round::Nearest).ok(), Some(Some(0)));
            assert_eq!(exp::last(false, 0, &scale, Round::Nearest).ok(), Some(Some(scale.pow10)));
        }
    }

    /// The first two pairs are review repros, the third the largest
    /// deficit a search found; the rest are 32-bit halves at extremes.
    #[test]
    fn mul_high_128_approx_deficit_is_at_most_six() {
        fn deficit(a: u128, b: u128) -> u128 {
            let (a1, a0) = ((a >> 64) as u64, a as u64);
            let (b1, b0) = ((b >> 64) as u64, b as u64);
            let (mid, c1) = full(a0, b1).overflowing_add(full(a1, b0));
            let (mid, c2) = mid.overflowing_add(full(a0, b0) >> 64);
            let exact = full(a1, b1) + (mid >> 64) + ((u128::from(c1) + u128::from(c2)) << 64);
            exact - mul_high_128_approx(a, b)
        }
        let pinned = [
            (0x4e8a0554ffffffff1e3e5011ffffffff, 0x753e7816ffffffff02a30f77ffffffff, 5),
            (0xba12e343803eff9dffffffffffffff01, 0x3572cc52143d0bfffffffc9ecb8ed1, 5),
            (0x3e68d65299e4c59b3ffffffff, 0x2fffffffeea043bc6fffffffe, 6),
        ];
        for (a, b, expected) in pinned {
            assert_eq!(deficit(a, b), expected, "{a:#x} {b:#x}");
        }
        const EXTREMES: [u64; 8] = [0, 1, 2, 3, 0xffff_fffe, 0xffff_ffff, 0x7fff_ffff, 0x8000_0000];
        let mut max = 0;
        let mut stream = random_inputs(400_000);
        let mut half = move || {
            let x = stream.next().unwrap();
            if x % 4 == 0 { x as u64 & 0xffff_ffff } else { EXTREMES[(x >> 8) as usize % 8] }
        };
        for _ in 0..50_000 {
            let a = (u128::from(half()) << 96) | (u128::from(half()) << 64) | (u128::from(half()) << 32) | u128::from(half());
            let b = (u128::from(half()) << 96) | (u128::from(half()) << 64) | (u128::from(half()) << 32) | u128::from(half());
            let d = deficit(a, b);
            assert!(d <= 6, "{a:#x} {b:#x} deficit {d}");
            max = max.max(d);
        }
        assert!(max >= 4, "the set is not adversarial enough: max {max}");
    }

    fn to_limbs2(value: u128) -> Limbs<2> {
        [value as u64, (value >> 64) as u64]
    }

    #[test]
    fn settle_decides_only_away_from_boundaries() {
        let eps = to_limbs2(1 << 100);
        let near_zero = to_limbs2(1 << 90);
        let mid = to_limbs2(1 << 126);
        let near_half = to_limbs2((1 << 127) + (1 << 90));
        let above_half = to_limbs2((1 << 127) + (1 << 110));
        assert_eq!(settle(5, &near_zero, &eps, Round::Floor), None);
        assert_eq!(settle(5, &mid, &eps, Round::Floor), Some(Some(5)));
        assert_eq!(settle(5, &mid, &eps, Round::Ceil), Some(Some(6)));
        assert_eq!(settle(5, &mid, &eps, Round::Nearest), Some(Some(5)));
        assert_eq!(settle(5, &near_half, &eps, Round::Nearest), None);
        assert_eq!(settle(5, &near_half, &eps, Round::Floor), Some(Some(5)));
        assert_eq!(settle(5, &above_half, &eps, Round::Nearest), Some(Some(6)));
        assert_eq!(settle(u128::MAX, &mid, &eps, Round::Ceil), Some(None));
        for (f, round) in [
            (1u128 << 90, Round::Floor),
            (1 << 126, Round::Floor),
            (1 << 126, Round::Ceil),
            (1 << 126, Round::Nearest),
            ((1 << 127) + (1 << 90), Round::Nearest),
            ((1 << 127) + (1 << 90), Round::Floor),
            ((1 << 127) + (1 << 110), Round::Nearest),
            (u128::MAX - (1 << 90), Round::Ceil),
            ((1 << 127) - (1 << 90), Round::Nearest),
        ] {
            assert_eq!(settle_word(5, (f >> 64) as u64, 1 << 36, round), settle(5, &to_limbs2(f), &eps, round));
        }
    }
}
