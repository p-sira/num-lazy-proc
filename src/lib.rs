//! Make numeric literals and mathematical constants convenient in generic Rust functions.
//!
//! ```
//! use num_lazy_proc::generic_float;
//! use num_traits::Float;
//!
//! #[generic_float(T)]
//! fn circumference<T: Float>(radius: T) -> T {
//!     2.0 * PI * radius
//! }
//!
//! assert_eq!(circumference(1.0_f64), core::f64::consts::TAU);
//! ```
//!
//! By default, the macro converts floating-point literals and recognized constants.
//! Integer literals retain their normal types by default, so indices and arguments
//! such as `arr[0]` and `powi(2)` continue to work. Use `include = [...]` to select
//! which categories the attribute rewrites.
//! The optional `convert` argument accepts any Rust expression containing the
//! `literal` placeholder.
//!
//! Mathematical constants come from `core::f64::consts` and are converted to the
//! target type using the same conversion as numeric literals, so no `FloatConst`
//! bound is needed.
//!
//! ```
//! /// F = π² × E × I / L²
//! #[allow(non_snake_case)]
//! #[num_lazy_proc::generic_float(T, ignore = [E])]
//! fn euler_buckling_load<T: num_traits::Float>(E: T, second_moment: T, length: T) -> T {
//!     PI * PI * E * second_moment / (length * length)
//! }
//!
//! let load = euler_buckling_load(200e9_f64, 8.33e-6_f64, 2.0_f64);
//! assert!(load > 4.0e6 && load < 4.2e6);
//! ```

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, ToTokens};
use std::collections::HashSet;
use syn::{
    bracketed,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    visit_mut::{self, VisitMut},
    Expr, Ident, ItemFn, Lit, LitBool, Macro, Result, Token,
};

#[derive(Clone, Copy)]
struct Inclusion {
    constants: bool,
    float: bool,
    integer: bool,
}

impl Default for Inclusion {
    fn default() -> Self {
        Self {
            constants: true,
            float: true,
            integer: false,
        }
    }
}

struct Options {
    target: Ident,
    ignore: HashSet<String>,
    include: Inclusion,
    conversion: Option<Expr>,
    visit_macros: bool,
}

impl Parse for Options {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let target: Ident = input.parse()?;
        let mut ignore = HashSet::new();
        let mut include = Inclusion::default();
        let mut seen_ignore = false;
        let mut seen_include = false;
        let mut conversion = None;
        let mut visit_macros = true;
        let mut seen_visit_macros = false;

        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            match key.to_string().as_str() {
                "ignore" if !seen_ignore => {
                    seen_ignore = true;
                    let content;
                    bracketed!(content in input);
                    let names = Punctuated::<Ident, Token![,]>::parse_terminated(&content)?;
                    ignore.extend(names.into_iter().map(|name| name.to_string()));
                }
                "include" if !seen_include => {
                    seen_include = true;
                    let content;
                    bracketed!(content in input);
                    let names = Punctuated::<Ident, Token![,]>::parse_terminated(&content)?;
                    include = Inclusion {
                        constants: false,
                        float: false,
                        integer: false,
                    };
                    for name in names {
                        let enabled = match name.to_string().as_str() {
                            "constants" => &mut include.constants,
                            "float" => &mut include.float,
                            "integer" => &mut include.integer,
                            _ => {
                                return Err(syn::Error::new_spanned(
                                    name,
                                    "unknown include category; expected constants, float, or integer",
                                ));
                            }
                        };
                        if *enabled {
                            return Err(syn::Error::new_spanned(
                                name,
                                "duplicate include category",
                            ));
                        }
                        *enabled = true;
                    }
                }
                "convert" if conversion.is_none() => {
                    let template: Expr = input.parse()?;
                    let mut probe = template.clone();
                    let sample: Expr = parse_quote!(0.0);
                    let mut checker = SubstituteLiteral {
                        value: &sample,
                        count: 0,
                    };
                    checker.visit_expr_mut(&mut probe);
                    if checker.count == 0 {
                        return Err(syn::Error::new_spanned(
                            template,
                            "conversion expression must contain `literal` as an expression placeholder",
                        ));
                    }
                    conversion = Some(template);
                }
                "visit_macros" if !seen_visit_macros => {
                    seen_visit_macros = true;
                    let enabled: LitBool = input.parse()?;
                    visit_macros = enabled.value;
                }
                "ignore" | "include" | "convert" | "visit_macros" => {
                    return Err(syn::Error::new_spanned(key, "duplicate option"));
                }
                _ => return Err(syn::Error::new_spanned(key, "unknown option")),
            }
        }

        Ok(Self {
            target,
            ignore,
            include,
            conversion,
            visit_macros,
        })
    }
}

struct SubstituteLiteral<'a> {
    value: &'a Expr,
    count: usize,
}

impl VisitMut for SubstituteLiteral<'_> {
    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        if let Expr::Path(path) = expression {
            if path.qself.is_none() && path.path.is_ident("literal") {
                *expression = self.value.clone();
                self.count += 1;
                return;
            }
        }
        visit_mut::visit_expr_mut(self, expression);
    }
}

struct ReplaceNumbers {
    options: Options,
}

impl ReplaceNumbers {
    fn convert(&self, expression: Expr) -> Expr {
        if let Some(template) = &self.options.conversion {
            let mut converted = template.clone();
            SubstituteLiteral {
                value: &expression,
                count: 0,
            }
            .visit_expr_mut(&mut converted);
            return converted;
        }
        let target = &self.options.target;
        parse_quote!(#target::from(#expression).unwrap())
    }

    fn constant(&self, name: &str) -> Option<Expr> {
        let target = &self.options.target;
        let special = match name {
            "EPSILON" | "EPS" => Some(quote!(#target::epsilon())),
            "INFINITY" | "INF" => Some(quote!(#target::infinity())),
            "NEG_INFINITY" | "NEG_INF" => Some(quote!(#target::neg_infinity())),
            "NAN" => Some(quote!(#target::nan())),
            "MIN_VAL" => Some(quote!(#target::min_value())),
            "MAX_VAL" => Some(quote!(#target::max_value())),
            "MIN_POSITIVE" => Some(quote!(#target::min_positive_value())),
            "NEG_ZERO" => Some(quote!(#target::neg_zero())),
            _ => None,
        };
        if let Some(tokens) = special {
            return syn::parse2(tokens).ok();
        }

        // Use correctly rounded decimal literals for compatibility with Rust
        // versions before core::{f32,f64}::consts::GOLDEN_RATIO was stabilized.
        // Restrict the Clippy allowance to the generated constant declaration.
        if name == "PHI" {
            let phi: Expr = parse_quote!({
                #[allow(clippy::approx_constant)]
                const PHI_VALUE: f64 = 1.618_033_988_749_895_f64;
                PHI_VALUE
            });
            return Some(self.convert(phi));
        }

        let known = matches!(
            name,
            "PI" | "FRAC_PI_2"
                | "PI_2"
                | "FRAC_PI_3"
                | "PI_3"
                | "FRAC_1_PI"
                | "FRAC_2_PI"
                | "FRAC_2_SQRT_PI"
                | "TAU"
                | "E"
                | "LN_2"
                | "LN_10"
                | "LOG2_10"
                | "LOG2_E"
                | "LOG10_2"
                | "LOG10_E"
                | "SQRT_2"
                | "FRAC_1_SQRT_2"
        );
        if !known {
            return None;
        }

        let constant_name = match name {
            "PI_2" => "FRAC_PI_2",
            "PI_3" => "FRAC_PI_3",
            _ => name,
        };
        let constant = Ident::new(constant_name, Span::call_site());
        let base: Expr = parse_quote!(core::f64::consts::#constant);
        Some(self.convert(base))
    }
}

impl VisitMut for ReplaceNumbers {
    fn visit_macro_mut(&mut self, mac: &mut Macro) {
        if !self.options.visit_macros {
            return;
        }

        // Match numeric_literals' traversal: only rewrite macro bodies that
        // can be parsed as Rust expressions. Other token syntaxes are left as-is.
        if let Ok(mut expression) = mac.parse_body::<Expr>() {
            self.visit_expr_mut(&mut expression);
            mac.tokens = expression.into_token_stream();
            return;
        }

        if let Ok(mut expressions) =
            mac.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated)
        {
            for expression in &mut expressions {
                self.visit_expr_mut(expression);
            }
            mac.tokens = expressions.into_token_stream();
            return;
        }

        if let Ok(mut expressions) =
            mac.parse_body_with(Punctuated::<Expr, Token![;]>::parse_terminated)
        {
            for expression in &mut expressions {
                self.visit_expr_mut(expression);
            }
            mac.tokens = expressions.into_token_stream();
        }
    }

    fn visit_expr_mut(&mut self, node: &mut Expr) {
        match node {
            Expr::Path(path)
                if self.options.include.constants
                    && path.qself.is_none()
                    && path.path.get_ident().is_some() =>
            {
                let name = path.path.get_ident().expect("checked above").to_string();
                if !self.options.ignore.contains(&name) {
                    if let Some(replacement) = self.constant(&name) {
                        *node = replacement;
                    }
                }
            }
            Expr::Lit(expr) => {
                let should_replace = match expr.lit {
                    Lit::Float(_) => self.options.include.float,
                    Lit::Int(_) => self.options.include.integer,
                    _ => false,
                };
                if should_replace {
                    *node = self.convert(Expr::Lit(expr.clone()));
                }
            }
            Expr::Macro(expression) => self.visit_macro_mut(&mut expression.mac),
            _ => visit_mut::visit_expr_mut(self, node),
        }
    }
}

/// Rewrite selected numeric literals and recognized constants to the target type at compile time.
///
/// By default, the macro converts floating-point literals and recognized constants:
/// `include = [constants, float]`. Use `include = [...]` to select which categories
/// to rewrite (`constants`, `float`, or `integer`), or `include = []` to disable
/// rewriting entirely. Integer literals retain their normal types by default.
/// Use `ignore = [E]` to prevent the macro from replacing that identifier.
///
/// By default, included literals are converted using `T::from(literal).unwrap()`.
/// The optional `convert` argument accepts any Rust expression containing the
/// `literal` placeholder. Special constants such as `EPSILON` use methods on
/// the target type.
///
/// By default, the attribute also visits macro arguments that can be parsed as
/// Rust expressions, including comma- or semicolon-separated expressions and
/// nested macros. Use `visit_macros = false` to leave macro arguments unchanged.
#[proc_macro_attribute]
pub fn generic_float(args: TokenStream, item: TokenStream) -> TokenStream {
    let options = parse_macro_input!(args as Options);
    let mut function = parse_macro_input!(item as ItemFn);
    ReplaceNumbers { options }.visit_block_mut(&mut function.block);
    quote!(#function).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_includes_constants_and_floats() {
        let options: Options = syn::parse_str("T").unwrap();
        assert!(options.include.constants);
        assert!(options.include.float);
        assert!(!options.include.integer);
    }

    #[test]
    fn empty_include_disables_every_category() {
        let options: Options = syn::parse_str("T, include = []").unwrap();
        assert!(!options.include.constants);
        assert!(!options.include.float);
        assert!(!options.include.integer);
    }

    #[test]
    fn invalid_inclusion_is_rejected() {
        for attribute in [
            "T, include = [float, float]",
            "T, include = [floats]",
            "T, include = [integer], include = [constants]",
            "T, literals = \"all\"",
        ] {
            assert!(
                syn::parse_str::<Options>(attribute).is_err(),
                "expected invalid attribute: {attribute}"
            );
        }
    }
}
