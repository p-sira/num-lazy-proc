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
//! Stable constants and existing aliases are recognized automatically. Use
//! `constants = [SQRT_3]` to additionally enable experimental constants.
//! The optional `convert` argument accepts any Rust expression containing the
//! `literal` placeholder, including expression-shaped macro arguments.
//!
//! Mathematical constants use `core::f64::consts` or equivalent `f64` literals
//! for constants unavailable on Rust 1.88. They use the same conversion as
//! numeric literals, so no `FloatConst` bound is needed.
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

use proc_macro2::{Delimiter, Group, Span, TokenStream as TokenStream2, TokenTree};
use quote::{quote, ToTokens};
use std::collections::HashSet;
use syn::{
    bracketed,
    parse::{Parse, ParseStream},
    parse_quote,
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

enum Constant<'a> {
    Special(&'static str),
    Math(&'a str),
    Value(f64),
    Experimental(f64),
}

impl<'a> Constant<'a> {
    // Literal fallbacks preserve Rust 1.88 compatibility and match Rust's values.
    #[allow(clippy::approx_constant)]
    fn from_name(name: &'a str) -> Option<Self> {
        Some(match name {
            "EPSILON" | "EPS" => Self::Special("epsilon"),
            "INFINITY" | "INF" => Self::Special("infinity"),
            "NEG_INFINITY" | "NEG_INF" => Self::Special("neg_infinity"),
            "NAN" => Self::Special("nan"),
            "MIN_VAL" => Self::Special("min_value"),
            "MAX_VAL" => Self::Special("max_value"),
            "MIN_POSITIVE" => Self::Special("min_positive_value"),
            "NEG_ZERO" => Self::Special("neg_zero"),
            "PHI" | "GOLDEN_RATIO" => Self::Value(1.618_033_988_749_895_f64),
            "EULER_GAMMA" => Self::Value(0.577_215_664_901_532_9_f64),
            "SQRT_3" => Self::Experimental(1.732_050_807_568_877_2_f64),
            "SQRT_5" => Self::Experimental(2.236_067_977_499_79_f64),
            "FRAC_1_SQRT_3" => Self::Experimental(0.577_350_269_189_625_7_f64),
            "FRAC_1_SQRT_5" => Self::Experimental(0.447_213_595_499_957_9_f64),
            "FRAC_1_SQRT_PI" => Self::Experimental(0.564_189_583_547_756_3_f64),
            "FRAC_1_SQRT_2PI" => Self::Experimental(0.398_942_280_401_432_7_f64),
            "PI_2" | "FRAC_PI_2" => Self::Math("FRAC_PI_2"),
            "PI_3" | "FRAC_PI_3" => Self::Math("FRAC_PI_3"),
            "PI_4" | "FRAC_PI_4" => Self::Math("FRAC_PI_4"),
            "PI_6" | "FRAC_PI_6" => Self::Math("FRAC_PI_6"),
            "PI_8" | "FRAC_PI_8" => Self::Math("FRAC_PI_8"),
            "PI" | "FRAC_1_PI" | "FRAC_2_PI" | "FRAC_2_SQRT_PI" | "TAU" | "E" | "LN_2"
            | "LN_10" | "LOG2_10" | "LOG2_E" | "LOG10_2" | "LOG10_E" | "SQRT_2"
            | "FRAC_1_SQRT_2" => Self::Math(name),
            _ => return None,
        })
    }
}

struct Options {
    target: Ident,
    ignore: HashSet<String>,
    skip_macros: HashSet<String>,
    constants: HashSet<String>,
    include: Inclusion,
    conversion: Option<Expr>,
    visit_macros: MacroTraversal,
}

#[derive(Clone, PartialEq, Eq)]
enum MacroTraversal {
    None,
    Auto,
    All,
    Selected(HashSet<String>),
}

// These standard macros accept expression arguments. Macros with pattern,
// literal, or token-sensitive arguments such as `matches!`, `concat!`, and
// `stringify!` must remain opaque in auto mode.
fn is_auto_macro_name(name: &str) -> bool {
    matches!(
        name,
        "vec"
            | "assert"
            | "assert_eq"
            | "assert_ne"
            | "debug_assert"
            | "debug_assert_eq"
            | "debug_assert_ne"
            | "dbg"
            | "format"
            | "format_args"
            | "print"
            | "println"
            | "eprint"
            | "eprintln"
            | "write"
            | "writeln"
            | "panic"
            | "unreachable"
            | "todo"
            | "unimplemented"
    )
}

fn is_auto_macro(path: &syn::Path) -> bool {
    let Some(last) = path.segments.last() else {
        return false;
    };
    is_auto_macro_name(&last.ident.to_string())
        && (path.segments.len() == 1
            || (path.segments.len() == 2
                && path.segments.first().is_some_and(|segment| {
                    matches!(segment.ident.to_string().as_str(), "std" | "core" | "alloc")
                })))
}

impl Parse for Options {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let target: Ident = input.parse()?;
        let mut ignore = HashSet::new();
        let mut skip_macros = HashSet::new();
        let mut include = Inclusion::default();
        let mut seen_ignore = false;
        let mut seen_skip_macros = false;
        let mut seen_include = false;
        let mut constants = HashSet::new();
        let mut seen_constants = false;
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
                "skip_macros" if !seen_skip_macros => {
                    seen_skip_macros = true;
                    let content;
                    bracketed!(content in input);
                    let names = Punctuated::<Ident, Token![,]>::parse_terminated(&content)?;
                    for name in names {
                        if !skip_macros.insert(name.to_string()) {
                            return Err(syn::Error::new_spanned(name, "duplicate macro name"));
                        }
                    }
                }
                "constants" if !seen_constants => {
                    seen_constants = true;
                    let content;
                    bracketed!(content in input);
                    let names = Punctuated::<Ident, Token![,]>::parse_terminated(&content)?;
                    for name in names {
                        let value = name.to_string();
                        if Constant::from_name(&value).is_none() {
                            return Err(syn::Error::new_spanned(name, "unknown constant"));
                        }
                        if !constants.insert(value) {
                            return Err(syn::Error::new_spanned(name, "duplicate constant name"));
                        }
                    }
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
                    if input.peek(syn::token::Bracket) {
                        let content;
                        bracketed!(content in input);
                        let names = Punctuated::<Ident, Token![,]>::parse_terminated(&content)?;
                        let mut selected = HashSet::new();
                        for name in names {
                            if !selected.insert(name.to_string()) {
                                return Err(syn::Error::new_spanned(name, "duplicate macro name"));
                            }
                        }
                        visit_macros = MacroTraversal::Selected(selected);
                    } else {
                        let mode: Ident = input.parse()?;
                        visit_macros = match mode.to_string().as_str() {
                            "none" => MacroTraversal::None,
                            "auto" => MacroTraversal::Auto,
                            "all" => MacroTraversal::All,
                            _ => {
                                return Err(syn::Error::new_spanned(
                                    mode,
                                    "unknown visit_macros mode; expected none, auto, all, or a list of macro names",
                                ));
                            }
                        };
                    }
                }
                "ignore" | "skip_macros" | "include" | "convert" | "visit_macros" | "constants" => {
                    return Err(syn::Error::new_spanned(key, "duplicate option"));
                }
                _ => return Err(syn::Error::new_spanned(key, "unknown option")),
            }
        }

        Ok(Self {
            target,
            ignore,
            skip_macros,
            constants,
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
fn visit_macro_expression(visitor: &mut impl VisitMut, expression: &mut Expr) {
    // `PI = value` may bind a named argument or an identifier in a custom
    // macro. Preserve its left-hand side regardless of the number of arguments.
    if let Expr::Assign(assignment) = expression {
        if matches!(assignment.left.as_ref(), Expr::Path(path)
            if path.qself.is_none() && path.path.get_ident().is_some())
        {
            visitor.visit_expr_mut(&mut assignment.right);
            return;
        }
    }
    visitor.visit_expr_mut(expression);
}

fn is_literal_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(_) => true,
        Expr::Paren(paren) => is_literal_expression(&paren.expr),
        Expr::Group(group) => is_literal_expression(&group.expr),
        Expr::Unary(unary) if matches!(unary.op, UnOp::Neg(_)) => {
            is_literal_expression(&unary.expr)
        }
        Expr::Cast(cast) => is_literal_expression(&cast.expr),
        _ => false,
    }
}

fn visit_macro_arguments(visitor: &mut impl VisitMut, mac: &mut Macro, is_vec: bool) -> bool {
    if let Ok(mut expression) = mac.parse_body::<Expr>() {
        visit_macro_expression(visitor, &mut expression);
        mac.tokens = expression.into_token_stream();
        return true;
    }

    if let Ok(mut expressions) =
        mac.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated)
    {
        for expression in &mut expressions {
            visit_macro_expression(visitor, expression);
        }
        mac.tokens = expressions.into_token_stream();
        return true;
    }

    if let Ok(mut expressions) =
        mac.parse_body_with(Punctuated::<Expr, Token![;]>::parse_terminated)
    {
        // In vec![value; count], count must remain an integer expression.
        if is_vec {
            if let Some(value) = expressions.first_mut() {
                visit_macro_expression(visitor, value);
            }
        } else {
            for expression in &mut expressions {
                visit_macro_expression(visitor, expression);
            }
        }
        mac.tokens = expressions.into_token_stream();
        return true;
    }
    false
}

impl VisitMut for SubstituteLiteral<'_> {
    fn visit_macro_mut(&mut self, mac: &mut Macro) {
        let _ = visit_macro_arguments(self, mac, false);
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
    preserve_explicit_float_types: bool,
    changed: bool,
}

impl ReplaceNumbers {
    // Opted-in macro traversal also accepts custom macro grammars, which may not
    // parse as Rust expressions (e.g. `@any [...] == INF` or `mut [x = NAN]`).
    // Rewrite floating-point atoms and constants, retaining the surrounding
    // macro punctuation. Bare integer atoms have ambiguous roles (index,
    // repetition count, const generic, or numeric value) and stay unchanged.
    // This is intentionally opt-in: we cannot know whether a macro expects
    // expression, literal, pattern, or identifier fragments from its tokens.
    fn preceding_macro_path(trees: &[TokenTree], group_index: usize) -> Option<syn::Path> {
        let bang = group_index.checked_sub(1)?;
        if !matches!(&trees[bang], TokenTree::Punct(p) if p.as_char() == '!') {
            return None;
        }
        let mut start = bang.checked_sub(1)?;
        if !matches!(&trees[start], TokenTree::Ident(_)) {
            return None;
        }
        while start >= 3
            && matches!(&trees[start - 1], TokenTree::Punct(p) if p.as_char() == ':')
            && matches!(&trees[start - 2], TokenTree::Punct(p) if p.as_char() == ':')
            && matches!(&trees[start - 3], TokenTree::Ident(_))
        {
            start -= 3;
        }
        if start >= 2
            && matches!(&trees[start - 1], TokenTree::Punct(p) if p.as_char() == ':')
            && matches!(&trees[start - 2], TokenTree::Punct(p) if p.as_char() == ':')
        {
            start -= 2;
        }
        syn::parse2(trees[start..bang].iter().cloned().collect()).ok()
    }

    fn is_index_group(trees: &[TokenTree], index: usize) -> bool {
        let Some(before) = index.checked_sub(1) else {
            return false;
        };
        if matches!(before.checked_sub(1).and_then(|i| trees.get(i)),
            Some(TokenTree::Punct(p)) if matches!(p.as_char(), '@' | '$'))
        {
            return false;
        }
        let candidate: TokenStream2 = [trees[before].clone(), trees[index].clone()]
            .into_iter()
            .collect();
        matches!(syn::parse2::<Expr>(candidate), Ok(Expr::Index(_)))
    }

    fn visit_opaque_macro_tokens(&mut self, tokens: TokenStream2) -> TokenStream2 {
        let trees: Vec<_> = tokens.into_iter().collect();
        let mut result = TokenStream2::new();
        for (i, tree) in trees.iter().enumerate() {
            match tree {
                TokenTree::Group(group) => {
                    // Nested macros must use their normal traversal rules;
                    // otherwise `vec![value; count]` loses its protected count.
                    // Unselected nested macros remain completely opaque.
                    let contents = if let Some(path) = Self::preceding_macro_path(&trees, i) {
                        if let Ok(Expr::Macro(mut nested)) =
                            syn::parse2::<Expr>(quote!(#path ! #group))
                        {
                            self.visit_macro_mut(&mut nested.mac);
                            nested.mac.tokens
                        } else {
                            group.stream()
                        }
                    } else if group.delimiter() == Delimiter::Bracket
                        && Self::is_index_group(&trees, i)
                    {
                        // Index expressions require their original integer type.
                        group.stream()
                    } else {
                        self.visit_opaque_macro_tokens(group.stream())
                    };
                    let mut updated = Group::new(group.delimiter(), contents);
                    updated.set_span(group.span());
                    result.extend([TokenTree::Group(updated)]);
                }
                TokenTree::Ident(ident) => {
                    // Leave paths, field names, macro tags, and assignment
                    // targets alone. In particular, `PI = value` in an
                    // opaque macro must keep its identifier binding.
                    let before = i.checked_sub(1).and_then(|j| trees.get(j));
                    let after = trees.get(i + 1);
                    let protected_before = matches!(before, Some(TokenTree::Punct(p))
                        if matches!(p.as_char(), ':' | '.' | '@' | '$'));
                    let protected_after = matches!(after, Some(TokenTree::Punct(p))
                        if matches!(p.as_char(), ':' | '!')
                            || (p.as_char() == '=' && p.spacing() == proc_macro2::Spacing::Alone));
                    if !protected_before
                        && !protected_after
                        && self.options.include.constants
                        && !self.options.ignore.contains(&ident.to_string())
                    {
                        if let Some(replacement) = self.constant(&ident.to_string()) {
                            self.changed = true;
                            result.extend(replacement.into_token_stream());
                            continue;
                        }
                    }
                    result.extend([tree.clone()]);
                }
                TokenTree::Literal(_) => {
                    if let Ok(mut expression) = syn::parse2::<Expr>(tree.to_token_stream()) {
                        // Raw integers may be indices, counts, tuple fields, or
                        // const arguments. Only expression-shaped macro bodies
                        // provide enough syntax to rewrite them safely.
                        if matches!(&expression, Expr::Lit(value)
                            if matches!(&value.lit, Lit::Int(int)
                                if !matches!(int.suffix(), "f32" | "f64")))
                            || matches!(i.checked_sub(1).and_then(|j| trees.get(j)),
                                Some(TokenTree::Punct(p)) if p.as_char() == '.')
                        {
                            result.extend([tree.clone()]);
                        } else {
                            self.visit_expr_mut(&mut expression);
                            result.extend(expression.into_token_stream());
                        }
                    } else {
                        result.extend([tree.clone()]);
                    }
                }
                _ => result.extend([tree.clone()]),
            }
        }
        result
    }

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
        // Generated expressions are emitted as tokens; reparsing them is unnecessary.
        Expr::Verbatim(quote!(#target::from(#expression).unwrap()))
    }

    fn constant(&self, name: &str) -> Option<Expr> {
        let target = &self.options.target;
        let base = match Constant::from_name(name)? {
            Constant::Special(method) => {
                let method = Ident::new(method, Span::call_site());
                return Some(Expr::Verbatim(quote!(#target::#method())));
            }
            Constant::Math(name) => {
                let constant = Ident::new(name, Span::call_site());
                Expr::Verbatim(quote!(core::f64::consts::#constant))
            }
            Constant::Experimental(_) if !self.options.constants.contains(name) => return None,
            Constant::Value(value) | Constant::Experimental(value) => {
                // Restrict the allowance to the generated fallback declaration.
                Expr::Verbatim(quote!({
                    #[allow(clippy::approx_constant)]
                    const VALUE: f64 = #value;
                    VALUE
                }))
            }
        };
        Some(self.convert(base))
    }
}

impl VisitMut for ReplaceNumbers {
    fn visit_macro_mut(&mut self, mac: &mut Macro) {
        let macro_name = mac
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string());
        if macro_name
            .as_ref()
            .is_some_and(|name| self.options.skip_macros.contains(name))
        {
            return;
        }
        let permitted = match &self.options.visit_macros {
            MacroTraversal::None => false,
            MacroTraversal::Auto => is_auto_macro(&mac.path),
            MacroTraversal::All => true,
            MacroTraversal::Selected(names) => {
                is_auto_macro(&mac.path)
                    || mac
                        .path
                        .segments
                        .last()
                        .is_some_and(|segment| names.contains(&segment.ident.to_string()))
            }
        };
        if !permitted {
            return;
        }
        let explicitly_selected = match &self.options.visit_macros {
            MacroTraversal::All => true,
            MacroTraversal::Selected(names) => {
                macro_name.as_ref().is_some_and(|name| names.contains(name))
            }
            _ => false,
        };
        let automatic = is_auto_macro(&mac.path) && !explicitly_selected;
        // An isolated literal in `dbg!` does not have a generic numeric
        // context. Keeping its original tokens also preserves custom macros
        // named `dbg` that accept `$value:literal`.
        if automatic
            && macro_name.as_deref() == Some("dbg")
            && mac
                .parse_body::<Expr>()
                .is_ok_and(|expr| is_literal_expression(&expr))
        {
            return;
        }
        // Explicit suffixes indicate a concrete float type. Outside `vec!`,
        // changing them in automatically visited macros can break comparisons
        // against f64 values and introduce unexpected trait bounds on T.
        let previous = self.preserve_explicit_float_types;
        self.preserve_explicit_float_types = automatic && macro_name.as_deref() != Some("vec");
        let parsed = visit_macro_arguments(
            self,
            mac,
            mac.path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "vec"),
        );
        if !parsed && !matches!(self.options.visit_macros, MacroTraversal::Auto) {
            mac.tokens = self.visit_opaque_macro_tokens(mac.tokens.clone());
        }
        self.preserve_explicit_float_types = previous;
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
                    && matches!(unary.expr.as_ref(), Expr::Lit(lit)
                        if matches!(&lit.lit, Lit::Int(value) if !matches!(value.suffix(), "f32" | "f64"))) =>
            {
                *node = self.convert(node.clone());
                self.changed = true;
            }
            Expr::Path(path) if self.options.include.constants && path.qself.is_none() => {
                if let Some(ident) = path.path.get_ident() {
                    let name = ident.to_string();
                    if !self.options.ignore.contains(&name) {
                        if let Some(replacement) = self.constant(&name) {
                            *node = replacement;
                            self.changed = true;
                        }
                    }
                }
            }
            Expr::Lit(expr) => {
                let should_replace = match expr.lit {
                    Lit::Float(ref value) => {
                        self.options.include.float
                            && !(self.preserve_explicit_float_types
                                && matches!(value.suffix(), "f32" | "f64"))
                    }
                    Lit::Int(ref value) if matches!(value.suffix(), "f32" | "f64") => {
                        self.options.include.float && !self.preserve_explicit_float_types
                    }
                    Lit::Int(_) => self.options.include.integer,
                    _ => false,
                };
                if should_replace {
                    *node = self.convert(Expr::Lit(expr.clone()));
                    self.changed = true;
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
/// Stable constants and existing aliases are automatic; `constants = [SQRT_3]`
/// additionally enables named experimental constants. An empty list keeps the
/// defaults. `ignore` takes precedence, and `include` must enable `constants`.
///
/// By default, included literals are converted using `T::from(literal).unwrap()`.
/// The optional `convert` argument accepts any Rust expression containing the
/// `literal` placeholder, including expression-shaped macro arguments. Special
/// constants such as `EPSILON` use methods on the target type.
///
/// `visit_macros = auto` (the default) rewrites expression arguments in `vec!`,
/// `assert!`, `assert_eq!`, `assert_ne!`, `debug_assert!`, `debug_assert_eq!`,
/// `debug_assert_ne!`, and `dbg!`. Use `visit_macros = all` to rewrite
/// other macro arguments, including comma- or semicolon-separated expressions
/// and floating-point atoms inside custom token syntax. Alternatively use
/// `visit_macros = [case, declare]` to rewrite named macros in addition to
/// those included in `auto`.
/// Opaque macro bodies are scanned token by token, preserving punctuation and
/// ambiguous integer tokens. In auto mode, explicit float suffixes stay typed
/// outside `vec!`; use unsuffixed literals for generic comparisons.
/// Use `skip_macros = [dbg]` to exclude macros with incompatible syntax.
/// These settings can alter macros
/// that inspect token syntax or expect literal, identifier, or pattern arguments.
/// Use `visit_macros = none` to leave all macro arguments unchanged.
#[proc_macro_attribute]
pub fn generic_float(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    expand_generic_float(args.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_generic_float(args: TokenStream2, item: TokenStream2) -> Result<TokenStream2> {
    let options = syn::parse2::<Options>(args)?;
    let mut function = syn::parse2::<ItemFn>(item.clone())?;
    let mut replacer = ReplaceNumbers {
        options,
        preserve_explicit_float_types: false,
        changed: false,
    };
    replacer.visit_block_mut(&mut function.block);
    if !replacer.changed {
        // Preserve the original tokens when the visitor made no substitutions.
        return Ok(item);
    }
    Ok(quote!(#function))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_op_expansion_preserves_original_tokens_and_attributes() {
        let item = quote!(
            #[doc = "preserve attribute spans and token ordering"]
            #[allow(non_snake_case)]
            pub fn example<T: Clone>(value: T) -> T {
                let _ = vec![value.clone(); 2];
                value
            }
        );
        let expanded = expand_generic_float(quote!(T), item.clone()).unwrap();
        assert_eq!(expanded.to_string(), item.to_string());
    }

    #[test]
    fn invalid_no_op_function_body_still_reports_parse_error() {
        assert!(expand_generic_float(quote!(T), quote!(fn invalid<T>() { let = value; })).is_err());
    }

    #[test]
    fn expansion_uses_proc_macro2_and_reports_parse_errors() {
        let expanded = expand_generic_float(
            quote!(T),
            quote!(
                fn example<T: num_traits::Float>(value: T) -> T {
                    PI + 2.0 * value
                }
            ),
        )
        .unwrap();
        let expanded = expanded.to_string();
        assert!(expanded.contains("core :: f64 :: consts :: PI"));
        assert!(expanded.contains("T :: from"));
        assert!(expand_generic_float(
            quote!(T, invalid = true),
            quote!(
                fn example() {}
            )
        )
        .is_err());
        assert!(expand_generic_float(
            quote!(T),
            quote!(
                struct Example;
            )
        )
        .is_err());
    }

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
            (
                "T, visit_macros = []",
                MacroTraversal::Selected(HashSet::new()),
            ),
        ] {
            let parsed: Options = syn::parse_str(attribute).unwrap();
            assert!(parsed.visit_macros == mode);
        }
    }

    #[test]
    fn selected_macro_names_are_parsed() {
        let parsed: Options =
            syn::parse_str("T, visit_macros = [case, declare, debug_assert, auto, all]").unwrap();
        let MacroTraversal::Selected(names) = parsed.visit_macros else {
            panic!("expected selected macros");
        };
        assert_eq!(names.len(), 5);
        assert!(names.contains("case"));
        assert!(names.contains("declare"));
        assert!(names.contains("debug_assert"));
        assert!(names.contains("auto"));
        assert!(names.contains("all"));
    }

    #[test]
    fn skipped_macro_names_are_parsed() {
        let parsed: Options = syn::parse_str("T, skip_macros = [dbg, assert_eq]").unwrap();
        assert!(parsed.skip_macros.contains("dbg"));
        assert!(parsed.skip_macros.contains("assert_eq"));
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
            "T, constants = [SQRT_3, SQRT_3]",
            "T, constants = [SQRT_3], constants = []",
            "T, constants = [UNKNOWN]",
            "T, constants = [MIN]",
            "T, constants = [MAX]",
            "T, constants = all",
            "T, include = [float, float]",
            "T, include = [floats]",
            "T, include = [integer], include = [constants]",
            "T, literals = \"all\"",
            "T, ignore = [PI], ignore = [E]",
            "T, convert = literal, convert = literal",
            "T, convert = f32::from(1.0)",
            "T, convert = custom!(value => literal)",
            "T, visit_macros = all, visit_macros = none",
            "T, visit_macros = [case, case]",
            "T, skip_macros = [dbg, dbg]",
            "T, skip_macros = [dbg], skip_macros = [assert_eq]",
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
