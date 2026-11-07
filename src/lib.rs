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
//! which categories the attribute rewrites. Nested items and const expressions
//! are left unchanged.
//! The optional `convert` argument accepts any Rust expression containing the
//! `literal` placeholder, including expression-shaped macro arguments.
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
    Expr, Ident, ItemFn, Lit, Macro, Result, Stmt, Token, UnOp,
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
    visit_macros: MacroTraversal,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MacroTraversal {
    None,
    Auto,
    All,
}

impl Parse for Options {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let target: Ident = input.parse()?;
        let mut ignore = HashSet::new();
        let mut include = Inclusion::default();
        let mut seen_ignore = false;
        let mut seen_include = false;
        let mut conversion = None;
        let mut visit_macros = MacroTraversal::Auto;
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
                    let mode: Ident = input.parse()?;
                    visit_macros = match mode.to_string().as_str() {
                        "none" => MacroTraversal::None,
                        "auto" => MacroTraversal::Auto,
                        "all" => MacroTraversal::All,
                        _ => {
                            return Err(syn::Error::new_spanned(
                                mode,
                                "unknown visit_macros mode; expected none, auto, or all",
                            ));
                        }
                    };
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

// Macro bodies are opaque to syn, so expression-shaped arguments need to be
// parsed explicitly before visiting them.
fn visit_macro_arguments(visitor: &mut impl VisitMut, mac: &mut Macro, is_vec: bool) {
    if let Ok(mut expression) = mac.parse_body::<Expr>() {
        visitor.visit_expr_mut(&mut expression);
        mac.tokens = expression.into_token_stream();
        return;
    }

    if let Ok(mut expressions) =
        mac.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated)
    {
        for expression in &mut expressions {
            visitor.visit_expr_mut(expression);
        }
        mac.tokens = expressions.into_token_stream();
        return;
    }

    if let Ok(mut expressions) =
        mac.parse_body_with(Punctuated::<Expr, Token![;]>::parse_terminated)
    {
        // In vec![value; count], count must remain an integer expression.
        if is_vec {
            if let Some(value) = expressions.first_mut() {
                visitor.visit_expr_mut(value);
            }
        } else {
            for expression in &mut expressions {
                visitor.visit_expr_mut(expression);
            }
        }
        mac.tokens = expressions.into_token_stream();
    }
}

impl VisitMut for SubstituteLiteral<'_> {
    fn visit_macro_mut(&mut self, mac: &mut Macro) {
        visit_macro_arguments(self, mac, false);
    }

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
            "EPSILON" | "EPS" => Some("epsilon"),
            "INFINITY" | "INF" => Some("infinity"),
            "NEG_INFINITY" | "NEG_INF" => Some("neg_infinity"),
            "NAN" => Some("nan"),
            "MIN_VAL" => Some("min_value"),
            "MAX_VAL" => Some("max_value"),
            "MIN_POSITIVE" => Some("min_positive_value"),
            "NEG_ZERO" => Some("neg_zero"),
            _ => None,
        };
        if let Some(method) = special {
            let method = Ident::new(method, Span::call_site());
            return Some(parse_quote!(#target::#method()));
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

        let constant_name = match name {
            "PI_2" | "FRAC_PI_2" => "FRAC_PI_2",
            "PI_3" | "FRAC_PI_3" => "FRAC_PI_3",
            "PI" | "FRAC_1_PI" | "FRAC_2_PI" | "FRAC_2_SQRT_PI" | "TAU" | "E" | "LN_2"
            | "LN_10" | "LOG2_10" | "LOG2_E" | "LOG10_2" | "LOG10_E" | "SQRT_2"
            | "FRAC_1_SQRT_2" => name,
            _ => return None,
        };
        let constant = Ident::new(constant_name, Span::call_site());
        let base: Expr = parse_quote!(core::f64::consts::#constant);
        Some(self.convert(base))
    }
}

impl VisitMut for ReplaceNumbers {
    fn visit_macro_mut(&mut self, mac: &mut Macro) {
        if self.options.visit_macros == MacroTraversal::None
            || (self.options.visit_macros == MacroTraversal::Auto && !mac.path.is_ident("vec"))
        {
            return;
        }
        visit_macro_arguments(self, mac, mac.path.is_ident("vec"));
    }

    fn visit_stmt_mut(&mut self, statement: &mut Stmt) {
        // Nested items have their own generic scope and cannot capture T.
        if !matches!(statement, Stmt::Item(_)) {
            visit_mut::visit_stmt_mut(self, statement);
        }
    }

    fn visit_type_mut(&mut self, _ty: &mut syn::Type) {
        // Array lengths and other const expressions in types require integer types.
    }

    fn visit_path_mut(&mut self, _path: &mut syn::Path) {
        // Const generic arguments inside paths must retain their original types.
    }

    fn visit_generic_argument_mut(&mut self, _argument: &mut syn::GenericArgument) {
        // Also protect turbofish arguments on method calls (receiver.method::<N>()).
    }

    fn visit_expr_repeat_mut(&mut self, repeat: &mut syn::ExprRepeat) {
        self.visit_expr_mut(&mut repeat.expr);
    }

    fn visit_expr_index_mut(&mut self, index: &mut syn::ExprIndex) {
        self.visit_expr_mut(&mut index.expr);
    }

    fn visit_expr_mut(&mut self, node: &mut Expr) {
        match node {
            Expr::Const(_) => {} // T::from is not generally callable in a const context.
            Expr::Unary(unary)
                if self.options.include.integer
                    && matches!(unary.op, UnOp::Neg(_))
                    && matches!(unary.expr.as_ref(), Expr::Lit(lit) if matches!(lit.lit, Lit::Int(_))) =>
            {
                *node = self.convert(node.clone());
            }
            Expr::Path(path) if self.options.include.constants && path.qself.is_none() => {
                if let Some(ident) = path.path.get_ident() {
                    let name = ident.to_string();
                    if !self.options.ignore.contains(&name) {
                        if let Some(replacement) = self.constant(&name) {
                            *node = replacement;
                        }
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
/// Even when included, integer literals in indices, array/repetition lengths,
/// and type or const generic arguments remain unchanged.
/// Use `ignore = [E]` to prevent the macro from replacing that identifier.
///
/// By default, included literals are converted using `T::from(literal).unwrap()`.
/// The optional `convert` argument accepts any Rust expression containing the
/// `literal` placeholder, including expression-shaped macro arguments. Special
/// constants such as `EPSILON` use methods on the target type.
///
/// `visit_macros = auto` (the default) currently rewrites `vec!` arguments,
/// including nested `vec!` invocations. Use `visit_macros = all` to rewrite expression-shaped
/// arguments in other macros, including comma- or semicolon-separated expressions.
/// This setting can alter macros that inspect token syntax or expect literal or
/// pattern arguments. Use `visit_macros = none` to leave all macro arguments unchanged.
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
        assert!(matches!(options.visit_macros, MacroTraversal::Auto));
    }

    #[test]
    fn macro_traversal_modes_are_explicit() {
        for (attribute, mode) in [
            ("T, visit_macros = none", MacroTraversal::None),
            ("T, visit_macros = auto", MacroTraversal::Auto),
            ("T, visit_macros = all", MacroTraversal::All),
        ] {
            let parsed: Options = syn::parse_str(attribute).unwrap();
            assert!(parsed.visit_macros == mode);
        }
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
            "T, ignore = [PI], ignore = [E]",
            "T, convert = literal, convert = literal",
            "T, convert = f32::from(1.0)",
            "T, convert = custom!(value => literal)",
            "T, visit_macros = all, visit_macros = none",
            "T, visit_macros = true",
            "T, visit_macros = false",
            "T, visit_macros = safe",
            "T, visit_macros = vec",
            "T, visit_macros = other",
            "T, visit_macros = 1",
        ] {
            assert!(
                syn::parse_str::<Options>(attribute).is_err(),
                "expected invalid attribute: {attribute}"
            );
        }
    }
}
