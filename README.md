# Fix

[![Crate Version][crate-badge]][crate]
![Build Status](https://github.com/hylo-so/fix/actions/workflows/pr.yml/badge.svg)

[crate-badge]: https://img.shields.io/crates/v/hylo-fix.svg
[crate]: https://crates.io/crates/hylo-fix

Fixed-point number types with Solana Anchor support.

## Cargo features

* `std` *(enabled by default)* — standard-library support, including decimal
  formatting.
* `alloc` — allocation-backed functionality, including decimal formatting,
  without requiring the full standard library.
* `anchor` *(enabled by default)* — Solana Anchor support: Borsh
  serialization and account-space derives for on-chain storage.
* `idl-build` — Anchor IDL generation. Implies `anchor`.
* `typed-floats` — floating-point conversions via the `typed_floats`
  crate.

This crate supports `no-std`:

```toml
[dependencies]
hylo-fix = { version = "0.8", default-features = false }
```

## [Documentation]

[Documentation]: https://docs.rs/hylo-fix
