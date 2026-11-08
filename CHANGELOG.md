# Changelog

## 0.3

### 0.3.0
**New Features**
- `generic_float`: Support `FRAC_PI_4`, `FRAC_PI_6`, and `FRAC_PI_8` with aliases `PI_4`, `PI_6`, and `PI_8`; `GOLDEN_RATIO` as an alias for `PHI`; and `EULER_GAMMA`. Preserve Rust 1.88 compatibility without requiring nightly features.
- `constants = [...]`: Explicitly enable experimental constants `SQRT_3`, `SQRT_5`, `FRAC_1_SQRT_3`, `FRAC_1_SQRT_5`, `FRAC_1_SQRT_PI`, and `FRAC_1_SQRT_2PI`. Stable constants and existing aliases remain automatic. `ignore` takes precedence, and `include` still controls whether constants are rewritten.

## 0.2

### 0.2.0
**New Features**
- `visit_macros = auto`: Expand default traversal beyond `vec!` to standard assertion, debugging, formatting, and output macros. Preserve explicitly suffixed floats outside `vec!` and standalone literals in `dbg!`, while leaving token-sensitive macros unchanged. Add `skip_macros = [...]` to exclude specific macro names. ([#1](https://github.com/p-sira/num-lazy-proc/pull/1))
- `visit_macros = [case, declare]`: Support explicitly selected custom macros alongside the standard macros in `auto`. Extend `visit_macros = all` and selected traversal to custom token syntax by rewriting floating-point literals and recognized constants while preserving macro punctuation, assignment targets, and ambiguous integer tokens. ([#2](https://github.com/p-sira/num-lazy-proc/pull/2))

**Performance Improvements**
- `generic_float`: Return the original function tokens when no literals or constants were rewritten, avoiding unnecessary token regeneration and preserving the original attributes and token spans. ([#3](https://github.com/p-sira/num-lazy-proc/pull/3))

## 0.1
### 0.1.0
- Initial release of `num-lazy-proc`.
