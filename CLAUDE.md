# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**hylo-fix** is a Rust library providing fixed-point number types with Solana Anchor support. It enables precise decimal arithmetic for blockchain applications where floating-point is unsuitable. Published as `hylo-fix` on crates.io, imported as `fix`.

## Build Commands

```bash
cargo build              # Build the library
cargo test               # Run all tests
cargo test <test_name>   # Run a single test by name
cargo check              # Type-check without building
cargo doc --open         # Generate and view documentation
```

CI runs `cargo build --verbose && cargo test --verbose` on PRs. Merges to master auto-publish to crates.io.

## Architecture

### Core Type: `Fix<Bits, Base, Exp>`

The central type is `Fix<Bits, Base, Exp>` in `src/lib.rs` — a generic fixed-point number parameterized by:
- **Bits**: Underlying integer type (u8–u128, i8–i128, usize, isize)
- **Base**: Type-level unsigned integer (e.g., `U2` for binary, `U10` for decimal) from `typenum`
- **Exp**: Type-level signed integer representing the exponent/scale from `typenum`

Scale changes are tracked at the type level. Multiplying two `Fix` values produces a new type with a combined exponent. Explicit `.convert()` is required to change the exponent — there are no silent precision conversions.

### Module Layout

| Module | Purpose |
|--------|---------|
| `lib.rs` | Core `Fix` struct, all arithmetic/comparison trait impls, checked/saturating operations |
| `fix_value.rs` | Concrete serializable types (`UFixValue8`–`UFixValue128`, `IFixValue8`–`IFixValue128`) for Anchor on-chain storage, generated via macros |
| `aliases.rs` | Pre-defined type aliases: `binary::` (base-2), `decimal::` (base-10), `si::` (SI prefixes like Milli, Kilo), `iec::` (IEC prefixes like Kibi, Mebi) |
| `util.rs` | `FixExt` trait providing `.one()` and `.zero()` for base-10 negative-exponent types, the `mul_floor`/`div_floor`/`checked_convert` helpers, and `MulDivExt` (the same helpers for 128-bit types) |
| `kernel.rs` | Multiply-divide and signed-division kernels in native 64-bit instructions for Solana's SBF target; results match `muldiv` bit for bit |
| `transcendental/` | `ln` and `exp` on the decimal types, correctly rounded through three Ziv tiers: `mod.rs` holds the entry points and the error argument, `ln.rs` and `exp.rs` the tiers, `tables.rs` the compile-time tables, `wide.rs` the multi-limb arithmetic |
| `prelude.rs` | Re-exports of all public items |

### Key Design Patterns

- **Macro-generated types**: `fix_value.rs` uses `define_fix_value!` macro to generate 10 unsigned + 10 signed concrete types with Anchor serialization support.
- **Type-level arithmetic**: Uses `typenum` crate for compile-time exponent tracking. Arithmetic operations produce types with correct combined exponents (e.g., `Mul` output has `ExpA + ExpB`).
- **Correctly rounded ln/exp**: `ln_*` and `exp_*` are correctly rounded at every exponent `N`; the composition guide in the crate docs depends on it, so keep it true.
- **MulDiv**: Combined multiply-then-divide operation (via `muldiv` crate) to avoid intermediate overflow. On `Fix` the inherent `mul_div_*` methods shadow the trait methods and run the crate's own kernel (`kernel.rs`), because `muldiv`'s u64/i64 path goes through `u128` arithmetic that SBF synthesizes in software.

### Cargo Features

- `idl-build`: Enables Anchor IDL generation support.

## Development Environment

Uses Nix flakes (see `flake.nix`) with Rust 1.81.0. If using direnv, the `.envrc` activates the Nix shell automatically.
