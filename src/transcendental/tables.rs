//! Compile-time constants and reduction tables, generated at Q376 (six
//! limbs, eight integer bits) and stored at Q184 (three limbs), so every
//! stored value is within one unit of the truth. Nothing here is
//! transcribed: `ln 2` and `ln 10` come from `2 atanh(z)` series on exact
//! rationals, `log2 e` from a long division by `ln 2`, and the tables from
//! those.

use super::wide::{
    add, div_frac_const, div_small_const, is_zero, mul_shr, mul_small, shl, shr, sub, Limbs,
};

pub(super) const INDEX_BITS: u32 = 8;
pub(super) const ENTRIES: usize = 1 << INDEX_BITS;
pub(super) const INDEX_MASK: usize = ENTRIES - 1;
/// A stored table entry and the middle tier's working format: three limbs
/// with eight integer bits in the top one, so Q184.
pub(super) type Q184 = Limbs<3>;

pub(super) type Gen = Limbs<6>;

/// `1` in any of the module's formats: `N` limbs with eight integer bits
/// in the top one, so `Q(64 N - 8)`.
pub(super) const fn one<const N: usize>() -> Limbs<N> {
    let mut one = [0; N];
    one[N - 1] = 1 << 56;
    one
}

/// `1 / den` at Q376.
const fn gen_frac(den: u64) -> Gen {
    div_small_const(&one(), den)
}

/// `2 atanh(z)` for `0 <= z < 1/2`, which is `ln((1 + z) / (1 - z))`, in
/// the eight-integer-bit format of `N` limbs.
const fn ln_series<const N: usize>(z: &Limbs<N>) -> Limbs<N> {
    let z2 = mul_shr(z, z, 8);
    let mut power = *z;
    let mut sum = *z;
    let mut n = 3;
    loop {
        power = mul_shr(&power, &z2, 8);
        if is_zero(&power) {
            break;
        }
        sum = add(&sum, &div_small_const(&power, n)).0;
        n += 2;
    }
    shl(&sum, 1)
}

/// `ln 2 = 2 atanh(1/3)` at `Q(64 N - 8)`.
const fn ln2_at<const N: usize>() -> Limbs<N> {
    ln_series(&div_small_const(&one::<N>(), 3))
}

/// `exp(y) - 1` for `0 <= y < 1/2`.
const fn exp_m1_series(y: &Gen) -> Gen {
    let mut term = *y;
    let mut sum = *y;
    let mut n = 2;
    loop {
        term = div_small_const(&mul_shr(&term, y, 8), n);
        if is_zero(&term) {
            break;
        }
        sum = add(&sum, &term).0;
        n += 1;
    }
    sum
}

/// `ln(1 + delta)` by the alternating series, for `delta < 2^-32`.
const fn log1p_small(delta: &Gen) -> Gen {
    let mut power = *delta;
    let mut sum = *delta;
    let mut n = 2;
    loop {
        power = mul_shr(&power, delta, 8);
        if is_zero(&power) {
            break;
        }
        let term = div_small_const(&power, n);
        sum = if n % 2 == 0 {
            sub(&sum, &term).0
        } else {
            add(&sum, &term).0
        };
        n += 1;
    }
    sum
}

const fn to_stored(value: &Gen) -> Q184 {
    [value[3], value[4], value[5]]
}

pub(super) const LN2_GEN: Gen = ln2_at();
pub(super) const LN10_GEN: Gen = add(&mul_small(&LN2_GEN, 3).0, &ln_series(&gen_frac(9))).0;
/// `1 / ln 2` at Q504: `2^496 * 2^512 / (ln 2 * 2^504)`. Eight limbs so
/// that `a log2(e) / 10^E` stays accurate to Q376 for `a` up to `2^127`.
pub(super) const LOG2E_Q504: Limbs<8> = {
    let mut a = [0; 8];
    a[7] = 1 << 48;
    div_frac_const(&a, &ln2_at::<8>())
};

pub(super) const LN2: Q184 = to_stored(&LN2_GEN);

/// `(s + 2) ln 2` at Q184 for every binary exponent `s` of a `u128`
/// mantissa, the two above `s` matching the bias of
/// `Scale::exp_ln10_biased`: the Q376 product truncated, so each entry and
/// its top two limbs at Q120 are within one unit.
pub(super) static S_LN2: [Q184; 128] = {
    let mut table = [[0; 3]; 128];
    let mut s = 0;
    while s < 128 {
        table[s] = to_stored(&mul_small(&LN2_GEN, s as u64 + 2).0);
        s += 1;
    }
    table
};

/// Reciprocal of the level's node `c_j = 1 + j / 2^bits` at Q31, pushed
/// four units above the exact value so `m R - 1` stays non-negative for
/// every `m >= c_j` after a product's own truncation; the table holds the
/// logarithm of what was stored. Node 0 is exactly 1 and its reciprocal
/// exactly `2^31`, which needs no push. Thirty-two bits so that the fast
/// tier's reductions are two native products each.
const fn reciprocal(bits: u32, j: usize) -> u32 {
    if j == 0 {
        return 1 << 31;
    }
    let node = (1u64 << bits) + j as u64;
    ((1u64 << (31 + bits)).div_ceil(node) + 4) as u32
}

const fn reciprocal_table(level2: bool) -> [u32; ENTRIES] {
    let bits = if level2 { 2 * INDEX_BITS } else { INDEX_BITS };
    let mut table = [0; ENTRIES];
    let mut j = 0;
    while j < ENTRIES {
        table[j] = reciprocal(bits, j);
        j += 1;
    }
    table
}

/// `-ln(R_j / 2^31)` for each stored reciprocal: `ln c_j` built up node by
/// node from `ln(c_j / c_(j-1))`, minus `log1p` of the reciprocal's push,
/// `delta = R_j c_j / 2^(31 + bits) - 1`.
const fn log_table(level2: bool) -> [Q184; ENTRIES] {
    let bits = if level2 { 2 * INDEX_BITS } else { INDEX_BITS };
    let mut table = [[0; 3]; ENTRIES];
    let mut ln_node: Gen = [0; 6];
    let mut j = 0;
    while j < ENTRIES {
        if j > 0 {
            // ln(c_j / c_(j-1)) = 2 atanh(1 / (2^(bits + 1) + 2 j - 1)).
            ln_node = add(&ln_node, &ln_series(&gen_frac((2 << bits) + 2 * j as u64 - 1))).0;
        }
        let node = (1u64 << bits) + j as u64;
        let push = reciprocal(bits, j) as u64 * node - (1u64 << (31 + bits));
        let delta = shl(&[push, 0, 0, 0, 0, 0], 376 - 31 - bits);
        let entry = sub(&ln_node, &log1p_small(&delta)).0;
        table[j] = to_stored(&entry);
        j += 1;
    }
    table
}

/// `2^(j / 2^bits)` from repeated products of `2^(1 / 2^bits)`.
const fn pow2_table(bits: u32) -> [Q184; ENTRIES] {
    let step = add(&one(), &exp_m1_series(&shr(&LN2_GEN, bits))).0;
    let mut table = [[0; 3]; ENTRIES];
    let mut value = one();
    let mut j = 0;
    while j < ENTRIES {
        table[j] = to_stored(&value);
        value = mul_shr(&value, &step, 8);
        j += 1;
    }
    table
}

pub(super) static LN_R1: [u32; ENTRIES] = reciprocal_table(false);
pub(super) static LN_R2: [u32; ENTRIES] = reciprocal_table(true);
pub(super) static LN_T1: [Q184; ENTRIES] = log_table(false);
pub(super) static LN_T2: [Q184; ENTRIES] = log_table(true);
pub(super) static EXP_T1: [Q184; ENTRIES] = pow2_table(INDEX_BITS);
pub(super) static EXP_T2: [Q184; ENTRIES] = pow2_table(2 * INDEX_BITS);

#[cfg(test)]
mod tests {
    use super::*;

    fn as_f64(limbs: &Q184) -> f64 {
        limbs
            .iter()
            .enumerate()
            .map(|(i, &w)| w as f64 * 2f64.powi(64 * i as i32 - 184))
            .sum()
    }

    #[test]
    fn constants_match_f64() {
        assert!((as_f64(&LN2) - core::f64::consts::LN_2).abs() < 1e-15);
        assert!((as_f64(&to_stored(&LN10_GEN)) - core::f64::consts::LN_10).abs() < 1e-15);
        let log2e = [LOG2E_Q504[5], LOG2E_Q504[6], LOG2E_Q504[7]];
        assert!((as_f64(&log2e) - core::f64::consts::LOG2_E).abs() < 1e-15);
    }

    #[test]
    fn ln_tables_are_logs_of_the_stored_reciprocals() {
        for j in 0..ENTRIES {
            let r1 = LN_R1[j] as f64 / 2f64.powi(31);
            assert!((as_f64(&LN_T1[j]) + r1.ln()).abs() < 1e-15, "level 1 entry {j}");
            let r2 = LN_R2[j] as f64 / 2f64.powi(31);
            assert!((as_f64(&LN_T2[j]) + r2.ln()).abs() < 1e-15, "level 2 entry {j}");
            let push1 = LN_R1[j] as u64 * ((1 << 8) + j as u64) - (1 << 39);
            let push2 = LN_R2[j] as u64 * ((1 << 16) + j as u64) - (1 << 47);
            assert!(push1 <= 5 * ((1 << 8) + j as u64) && push2 <= 5 * ((1 << 16) + j as u64));
            assert!((j == 0) == (push1 == 0) && (j == 0) == (push2 == 0));
        }
    }

    #[test]
    fn exp_tables_are_powers_of_two() {
        for j in 0..ENTRIES {
            let want1 = 2f64.powf(j as f64 / ENTRIES as f64);
            assert!((as_f64(&EXP_T1[j]) - want1).abs() < 1e-15, "level 1 entry {j}");
            let want2 = 2f64.powf(j as f64 / (ENTRIES * ENTRIES) as f64);
            assert!((as_f64(&EXP_T2[j]) - want2).abs() < 1e-15, "level 2 entry {j}");
        }
    }

}

#[cfg(all(test, feature = "std"))]
mod dump {
    use std::{format, println, string::String, vec::Vec};

    use super::*;

    fn hex(limbs: &[u64]) -> String {
        limbs.iter().rev().map(|w| format!("{w:016x}")).collect::<Vec<_>>().join("")
    }

    /// `cargo test --lib tables::dump -- --ignored --nocapture` feeds the
    /// out-of-tree table checker.
    #[test]
    #[ignore]
    fn dump_tables() {
        println!("LN2 {}", hex(&LN2));
        println!("LN10 {}", hex(&to_stored(&LN10_GEN)));
        println!("LN2_Q376 {}", hex(&LN2_GEN));
        println!("LN10_Q376 {}", hex(&LN10_GEN));
        println!("LOG2E_Q504 {}", hex(&LOG2E_Q504));
        for e in 0..=38 {
            let scale = super::super::Scale::new(e).unwrap();
            println!("LOG2E_OVER_POW10 {e} {}", hex(&scale.log2e_over_pow10));
            println!("EXP_LN10_BIASED {e} {}{}", hex(&scale.exp_ln10_biased), hex(&scale.exp_ln10_biased_low));
            println!("INV_POW10 {e} {}", hex(&scale.inv_pow10));
        }
        for (s, entry) in S_LN2.iter().enumerate() {
            println!("S_LN2_Q184 {s} {}", hex(entry));
        }
        for j in 0..ENTRIES {
            println!("LN_T1 {j} {} {}", LN_R1[j], hex(&LN_T1[j]));
            println!("LN_T2 {j} {} {}", LN_R2[j], hex(&LN_T2[j]));
            println!("EXP_T1 {j} {}", hex(&EXP_T1[j]));
            println!("EXP_T2 {j} {}", hex(&EXP_T2[j]));
        }
    }
}
