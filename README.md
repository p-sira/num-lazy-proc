# num-lazy-proc

<p>
    <a href="https://opensource.org/license/BSD-3-clause">
        <img src="https://img.shields.io/badge/License-BSD--3--Clause-brightgreen.svg" alt="License">
    </a>
    <a href="https://crates.io/crates/num-lazy-proc">
        <img src="https://img.shields.io/crates/v/num-lazy-proc" alt="Crate">
    </a>
    <a href="https://docs.rs/num-lazy-proc">
        <img src="https://img.shields.io/badge/Docs-docs.rs-blue" alt="Documentation">
    </a>
</p>

`num-lazy-proc` makes numeric literals and mathematical constants convenient in generic Rust functions. It is a standalone procedural macro crate, separate from [`num-lazy`](https://github.com/p-sira/num-lazy).

```rust
use num_lazy_proc::generic_float;
use num_traits::Float;

#[generic_float(T)]
fn circumference<T: Float>(radius: T) -> T {
    2.0 * PI * radius
}
```

## How It Works

`#[generic_float(T)]` is an attribute procedural macro that rewrites selected Rust expressions into values of the target numeric type at compile time. For example, the `circumference` function above is conceptually expanded to:

```rust
use num_traits::Float;

fn circumference<T: Float>(radius: T) -> T {
    T::from(2.0).unwrap()
        * T::from(core::f64::consts::PI).unwrap()
        * radius
}
```

By default, the macro converts **floating-point literals** and **recognized constants**, and the resulting code is type-checked normally by Rust. See [Detailed Usage](#detailed-usage) for extra control options.

Mathematical constants come from `core::f64::consts` and are converted to the target type using the same conversion as numeric literals, hence, it doesn't require the `T: FloatConst` bound. Special values such as `EPSILON` and `NAN` use methods on the target type.

## Detailed Usage

### Controlling what gets converted

Use `include = [...]` to select which categories the attribute rewrites:

| Category | Rewrites |
| --- | --- |
| `constants` | Named mathematical constants (`PI`, `E`, etc.) and special values (`EPS`, `NAN`, etc.) |
| `float` | Floating-point literals (`0.5`, `1.0_f64`) |
| `integer` | Integer literals (`1`, `2`, etc.) |

`#[generic_float(T)]` is equivalent to `#[generic_float(T, include = [constants, float])]`. Integer literals retain their normal types by default, so indices and arguments such as `arr[0]` and `powi(2)` continue to work. Use `include = [constants]` to rewrite constants only, or `include = []` to disable rewriting entirely.

Even with `include = [integer]`, integers in indices, array/repetition lengths, and type or const generic arguments remain unchanged. Other operations expecting integer arguments, such as `powi(2)`, may still require care when integer conversion is enabled.

Nested items and `const { ... }` expressions are left unchanged because they cannot generally use conversions involving the enclosing function's generic type.

### Ignoring identifiers

In Euler's column-buckling equation, `E` denotes Young's modulus rather than Euler's number. Use `ignore = [E]` to prevent the macro from replacing that parameter:

```rust
/// F = π² × E × I / L²
#[allow(non_snake_case)]
#[num_lazy_proc::generic_float(T, ignore = [E])]
fn euler_buckling_load<T: num_traits::Float>(E: T, second_moment: T, length: T) -> T {
    PI * PI * E * second_moment / (length * length)
}
```

Only standalone, unqualified identifiers are rewritten. Paths such as `my_module::E` and `E::my_method()` are left unchanged and do not need to be included in `ignore`.

Recognized special constants: `EPS` (`EPSILON`), `INF` (`INFINITY`), `NEG_INF` (`NEG_INFINITY`), `NAN`, `MIN_VAL`, `MAX_VAL`, `MIN_POSITIVE`, and `NEG_ZERO`. These use methods on the target type, so no `FloatConst` bound is needed.

Recognized mathematical constants: `PI`, `TAU`, `E`, `PHI` (the golden ratio), `PI_2` (`FRAC_PI_2`), `PI_3` (`FRAC_PI_3`), `FRAC_1_PI`, `FRAC_2_PI`, `FRAC_2_SQRT_PI`, `SQRT_2`, `FRAC_1_SQRT_2`, `LN_2`, `LN_10`, `LOG2_10`, `LOG2_E`, `LOG10_2`, and `LOG10_E`.

### Nested macros

`visit_macros` accepts three modes and a list of macro names:

| Mode | Rewrites |
| --- | --- |
| `none` | No macro arguments |
| `auto` (default) | Standard expression macros including `vec!`, `assert!`, `assert_eq!`, `assert_ne!`, `debug_assert!`, `debug_assert_eq!`, `debug_assert_ne!`, `dbg!`, and formatting/output macros |
| `all` | All macro arguments, including floating-point literals and constants in custom syntax |
| `[case, declare]` | The named macros plus the standard macros in `auto` (custom macro names are examples) |

For example, the default `auto` mode handles nested vectors:

```rust
#[num_lazy_proc::generic_float(T)]
fn samples<T: num_traits::Float>(n: usize) -> Vec<Vec<T>> {
    vec![vec![PI, 0.0, TAU]; n]
}
```

`auto` visits supported standard macros such as `debug_assert!(x <= 1.0)` while keeping token-sensitive macros such as `matches!`, `concat!`, and `stringify!` unchanged. Explicitly suffixed `f32`/`f64` literals retain their types in automatically visited macros except `vec!`, so `assert_eq!(some_f64, 1.0_f64)` remains a concrete comparison; use unsuffixed literals for generic comparisons. An isolated literal in `dbg!` also stays unchanged.

Use `#[generic_float(T, visit_macros = [case, declare])]` to rewrite selected custom macros. For example, it handles numeric expressions in `case!(@any [x, y] == INF, PI_2)` and initializers in `declare!(mut [x = NAN])`. Expression-shaped macro arguments are parsed as Rust expressions; other macro bodies are scanned for floating-point literals and constants, preserving punctuation, identifier assignment targets, and ambiguous integer tokens such as indices and repetition counts. A macro's grammar is not generally known to the proc macro, so traversal can change the meaning of custom macros matching `$value:literal`.

Use `skip_macros = [dbg]` to exclude a shadowing macro with incompatible syntax, `visit_macros = all` to attempt all macros, or `visit_macros = none` to leave all macro arguments unchanged. Bare `auto` and `all` select traversal modes, while bracketed `[auto, all]` selects custom macros with those literal names.

### Custom conversion for other numeric traits

By default, included literals are converted using `T::from(literal).unwrap()`. The optional `convert` argument accepts any Rust expression containing the `literal` placeholder. The macro substitutes each included numeric literal or mathematical constant into that expression; special constants still use the target type's methods. For example, a trait with a `from_f64` constructor can use:

```rust
use num_lazy_proc::generic_float;
use std::ops::{Add, Mul};

trait CustomScalar: Copy + Add<Output = Self> + Mul<Output = Self> {
    fn from_f64(value: f64) -> Self;
}

#[generic_float(T, convert = <T as CustomScalar>::from_f64(literal))]
fn circumference<T: CustomScalar>(radius: T) -> T {
    2.0 * PI * radius
}
```

You can write `convert = T::from(literal).unwrap()`, use a conversion function from another crate, or use an expression-shaped macro such as `convert = cast_scalar!(T, literal)` whose body accepts expression arguments. Nested, comma-separated, and semicolon-separated macro arguments are supported in conversion templates. The expression is substituted syntactically and remains responsible for ensuring the input type is accepted. When including integer literals, ensure your custom conversion accepts their input type (for example, `convert = T::from_f64(f64::from(literal))` for integers supported by `f64::from`).
