use num_lazy_proc::generic_float;
use num_traits::Float;
use std::ops::{Add, Mul};

trait CustomScalar: Copy + Add<Output = Self> + Mul<Output = Self> {
    fn from_f64(value: f64) -> Self;
}

impl CustomScalar for f32 {
    fn from_f64(value: f64) -> Self {
        value as Self
    }
}

impl CustomScalar for f64 {
    fn from_f64(value: f64) -> Self {
        value
    }
}

#[generic_float(T, convert = <T as CustomScalar>::from_f64(literal))]
fn custom_scalar<T: CustomScalar>(radius: T) -> T {
    2.0 * PI * radius
}

#[allow(non_snake_case)]
#[generic_float(T, ignore = [E], convert = <T as CustomScalar>::from_f64(literal))]
fn custom_scalar_ignoring_e<T: CustomScalar>(E: T) -> T {
    E + PI
}

#[generic_float(T, convert = T::from_f64(f64::from(literal)), include = [integer])]
fn custom_scalar_integer<T: CustomScalar>(value: T) -> T {
    2 * value
}

macro_rules! cast_scalar {
    ($ty:ty, $value:expr) => {
        <$ty as CustomScalar>::from_f64($value)
    };
}

macro_rules! cast_scalar_semicolon {
    ($ty:ty; $value:expr) => {
        <$ty as CustomScalar>::from_f64($value)
    };
}

macro_rules! identity_expr {
    ($value:expr) => {
        $value
    };
}

#[generic_float(T, convert = cast_scalar!(T, literal))]
fn macro_based_conversion<T: CustomScalar>(value: T) -> T {
    PI + 2.0 * value
}

#[generic_float(T, convert = cast_scalar_semicolon!(T; identity_expr!(literal)))]
fn nested_macro_conversion<T: CustomScalar>(value: T) -> T {
    PI + 2.0 * value
}

#[generic_float(T)]
fn nested_vectors<T: Float>() -> Vec<Vec<T>> {
    vec![vec![PI, 0.0], vec![TAU, 1.0]]
}

#[generic_float(T)]
fn repeated_vectors<T: Float>(length: usize) -> Vec<Vec<T>> {
    vec![vec![PI, 1.0]; length]
}

#[generic_float(T)]
fn repeated_values<T: Float>(length: usize) -> Vec<T> {
    vec![0.0; length]
}

#[allow(non_snake_case)]
#[generic_float(T, ignore = [E])]
fn macro_with_ignored_identifier<T: Float>(E: T) -> Vec<T> {
    vec![E, PI]
}

#[generic_float(T, visit_macros = none)]
fn untouched_macro<T: Float>(_value: T) -> Vec<f64> {
    use core::f64::consts::PI;
    vec![PI, 0.0]
}

macro_rules! append {
    ($values:expr, $value:expr) => {
        $values.push($value)
    };
}

#[generic_float(T, visit_macros = all)]
fn statement_macro<T: Float>() -> Vec<T> {
    let mut values = Vec::new();
    append!(values, PI);
    values
}

macro_rules! case {
    ($condition:expr, $result:expr) => {
        if $condition {
            return $result;
        }
    };
    (@any [$($value:expr),+] == $target:expr, $result:expr) => {
        $(if $value == $target {
            return $result;
        })+
    };
}

macro_rules! declare {
    (mut [$($name:ident $(= $value:expr)?),+ $(,)?]) => {
        $(let mut $name $(= $value)?;)+
    };
    (mut [$($name:ident),+ $(,)?] = $value:expr) => {
        $(let mut $name = $value;)+
    };
    ([$($name:ident $(= $value:expr)?),+ $(,)?]) => {
        $(let $name $(= $value)?;)+
    };
}

#[generic_float(T, visit_macros = [case, debug_assert])]
fn selected_case_macros<T: Float>(x: T, y: T) -> T {
    debug_assert!(x <= INF);
    case!(@any [x.abs(), y.abs()] == INF, PI_2);
    case!(x < 0.5, EPS);
    PI
}

#[allow(unused_mut)]
#[generic_float(T, visit_macros = [declare])]
fn selected_declare_macros<T: Float>() -> (T, T, T, T) {
    declare!(mut [first = PI, second = NAN]);
    declare!(mut [third, fourth] = INF);
    declare!([fifth = EPS]);
    (first + fifth, second, third, fourth)
}

macro_rules! bind_name {
    ($name:ident = $value:expr) => {{
        let $name = $value;
        $name
    }};
}

#[allow(non_snake_case)]
#[generic_float(T, visit_macros = [bind_name])]
fn selected_single_assignment<T: Float>() -> T {
    bind_name!(PI = 2.0)
}

macro_rules! take {
    (@value $value:expr) => {
        $value
    };
}

macro_rules! tagged_list {
    ($tag:ident [$($value:expr),+]) => {
        vec![$($value),+]
    };
}

#[generic_float(T, visit_macros = [tagged_list])]
fn opaque_macro_tagged_list<T: Float>() -> Vec<T> {
    tagged_list!(value [PI, 1.5])
}

#[generic_float(T, include = [constants, float, integer], visit_macros = [take])]
fn opaque_macro_protects_integer_contexts<T: Float>() -> (T, Vec<T>) {
    let values = [1.5, 2.5];
    let selected = take!(@value values[0]);
    let repeated = take!(@value vec![PI; 2]);
    (selected, repeated)
}

#[generic_float(T, include = [float, integer])]
fn qualified_auto_vec<T: Float>() -> Vec<T> {
    std::vec![1.5; 2]
}

#[generic_float(T, include = [float, integer], visit_macros = [vec])]
fn qualified_selected_vec<T: Float>() -> Vec<T> {
    std::vec![1.5; 2]
}

mod shadowed_auto_macros {
    use super::*;

    macro_rules! dbg {
        ($value:literal) => {
            $value
        };
        ($value:expr) => {
            $value
        };
    }

    macro_rules! assert_eq {
        ($left:literal, $right:literal) => {
            $left == $right
        };
    }

    #[generic_float(T)]
    pub(super) fn simple_literal<T: Float>() -> f64 {
        let _ = core::mem::size_of::<T>();
        dbg!(1.5)
    }

    #[generic_float(T)]
    pub(super) fn wrapped_literals<T: Float>() -> (f64, f64, f64) {
        let _ = core::mem::size_of::<T>();
        (dbg!(1.5), dbg!((1.5)), dbg!(-1.5))
    }

    #[generic_float(T, skip_macros = [assert_eq])]
    pub(super) fn skipped_macro<T: Float>() -> bool {
        let _ = core::mem::size_of::<T>();
        assert_eq!(1.5, 1.5)
    }
}

#[generic_float(T)]
fn auto_preserves_explicit_float_types<T: Float>(value: T) -> T {
    assert_eq!(f64::from(2_u8), 2.0_f64);
    std::assert_ne!(core::f64::consts::PI, 3.125_f64);
    value
}

macro_rules! literal_sensitive {
    ($name:ident => $value:literal) => {
        $value
    };
}

#[generic_float(T, visit_macros = [case])]
fn selected_macro_does_not_change_others<T: Float>(x: T) -> (T, f64) {
    case!(x == INF, (PI, literal_sensitive!(name => 2.5)));
    (x, literal_sensitive!(name => 2.5))
}

#[generic_float(T, visit_macros = [case])]
fn selected_macro_preserves_nested_sensitive<T: Float>(x: T) -> T {
    case!(@any [x] == INF, PI + T::from(literal_sensitive!(name => 2.5)).unwrap());
    x
}

// Names that are traversal modes when unbracketed are ordinary macro names
// inside an explicit selection list.
macro_rules! auto {
    ($value:expr) => {
        $value
    };
}

macro_rules! all {
    ($value:expr) => {
        $value
    };
}

#[generic_float(T, visit_macros = [auto, all])]
fn selected_macros_named_like_modes<T: Float>() -> T {
    auto!(PI) + all!(0.5)
}

#[generic_float(T)]
fn standard_macros_in_auto_mode<T: Float + core::fmt::Debug + core::fmt::Display>(value: T) -> T {
    assert!(value > 0.5, "value={value}");
    assert_eq!(value, 1.0, "expected one, got {value}");
    assert_ne!(value, 2.0);
    debug_assert!(value < INF);
    debug_assert_eq!(value, 1.0);
    debug_assert_ne!(value, 2.0);

    // Format argument names are part of the format macro's syntax, even when
    // they happen to coincide with recognized mathematical constants.
    let formatted = format!("pi={PI}", PI = PI);
    assert!(formatted.starts_with("pi=3.14"));

    use std::fmt::Write as _;
    let mut output = String::new();
    write!(&mut output, "value={EPS}", EPS = EPS).unwrap();
    assert!(output.starts_with("value="));

    dbg!(value + 0.25)
}

#[generic_float(T)]
fn integer_shaped_float_suffixes<T: Float>() -> T {
    1f32 + 2f64
}

#[generic_float(T, include = [integer])]
fn integer_only_leaves_float_suffixes<T: Float>() -> f64 {
    let _ = core::mem::size_of::<T>();
    1f64
}

macro_rules! custom_syntax {
    ($name:ident => $value:literal) => {
        $value
    };
}

#[generic_float(T)]
fn unparseable_macro<T: Float>(_value: T) -> f64 {
    custom_syntax!(value => 7.0)
}

macro_rules! literal_token {
    ($value:literal) => {
        $value
    };
}

#[generic_float(T)]
fn literal_macro_argument<T: Float>(_value: T) -> f64 {
    literal_token!(1.5_f64)
}

#[generic_float(T)]
fn token_sensitive_macros<T: Float>(_value: T) -> (&'static str, bool) {
    (stringify!(PI), matches!(1.0_f64, 1.0_f64))
}

#[generic_float(T)]
fn nested_item_scopes<T: Float>(value: T) -> T {
    const SCALE: f64 = 2.5;
    fn adjustment() -> f64 {
        1.5
    }
    value + T::from(SCALE + adjustment()).unwrap() + 3.0
}

#[generic_float(T)]
fn inline_const_scope<T: Float>(value: T) -> T {
    value + T::from(const { 2.5_f64 }).unwrap() + 3.0
}

#[generic_float(T, include = [integer])]
fn negative_minimum_integer<T: Float>() -> T {
    -2147483648_i32
}

fn const_size<const N: usize>() -> usize {
    N
}

struct ConstMethod;

impl ConstMethod {
    fn size<const N: usize>(&self) -> usize {
        N
    }
}

#[generic_float(T, include = [constants, float, integer])]
fn integer_syntax_contexts<T: Float>() -> T {
    let fixed: [T; 2] = [PI, 1.0];
    let repeated = [2.0; 2];
    let mut vector = vec![3.0; 2];
    vector.push(5.0);
    let _ = const_size::<2>();
    let _ = ConstMethod.size::<2>();
    fixed[0] + repeated[1] + vector[0] + 4
}

#[generic_float(T)]
fn circumference<T: Float>(r: T) -> T {
    2.0 * PI * r
}

#[generic_float(T, ignore = [E])]
#[allow(non_snake_case)]
fn euler_buckling_load<T: Float>(E: T, second_moment: T, length: T) -> T {
    PI * PI * E * second_moment / (length * length)
}

#[generic_float(T, include = [constants, float, integer])]
fn convert_integer<T: Float>(r: T) -> T {
    2 * PI * r
}

#[generic_float(T)]
fn integer_context<T: Float>(values: &[T], r: T) -> T {
    values[0] + r.powi(2) + PI
}

#[generic_float(T, include = [constants])]
fn constants_only<T: Float>(r: T) -> T {
    r + E
}

#[allow(non_snake_case)]
#[generic_float(T, include = [float])]
fn float_only<T: Float>(PI: T) -> T {
    PI + 2.5
}

#[allow(non_snake_case)]
#[generic_float(T, include = [integer])]
fn integer_only<T: Float>(PI: T) -> T {
    PI + 3
}

#[allow(non_snake_case)]
#[generic_float(T, include = [float, integer])]
fn literals_only<T: Float>(PI: T) -> T {
    PI + 1.5 + 2
}

#[generic_float(T, include = [constants, integer])]
fn constants_and_integer<T: Float>(value: T) -> T {
    PI + 2 * value
}

#[allow(non_snake_case)]
#[generic_float(T, include = [])]
fn no_replacements<T: Float>(PI: T) -> f64 {
    let _ = PI;
    2.5
}

#[generic_float(T, include = [constants])]
fn special_constant_only<T: Float>() -> T {
    EPSILON
}

#[allow(non_snake_case)]
#[generic_float(T, include = [float])]
fn nested_float_only<T: Float>(PI: T) -> Vec<T> {
    vec![PI, 1.5]
}

#[generic_float(T)]
fn specials<T: Float>() -> (T, T, T, T) {
    (EPSILON, INF, NAN, NEG_ZERO)
}

#[generic_float(T, include = [constants])]
fn renamed_specials<T: Float>() -> (T, T, T, T) {
    (INF, NEG_INF, MIN_VAL, MAX_VAL)
}

#[generic_float(T, include = [constants])]
fn special_aliases<T: Float>() -> (T, T, T, T, T, T) {
    (EPSILON, EPS, INFINITY, INF, NEG_INFINITY, NEG_INF)
}

#[generic_float(T, include = [constants])]
fn pi_aliases<T: Float>() -> [T; 10] {
    [
        FRAC_PI_2, PI_2, FRAC_PI_3, PI_3, FRAC_PI_4, PI_4, FRAC_PI_6, PI_6, FRAC_PI_8, PI_8,
    ]
}

#[allow(non_snake_case)]
#[generic_float(T, include = [constants])]
fn min_max_are_not_constants<T: Float>(MIN: T, MAX: T) -> T {
    MIN + MAX
}

#[generic_float(T, include = [constants])]
fn limits<T: Float>() -> [T; 3] {
    [MIN_VAL, MAX_VAL, MIN_POSITIVE]
}

#[generic_float(T, include = [constants], convert = <T as CustomScalar>::from_f64(literal))]
fn custom_scalar_phi<T: CustomScalar>() -> [T; 3] {
    [PHI, GOLDEN_RATIO, EULER_GAMMA]
}

#[generic_float(T, include = [constants], constants = [
    SQRT_3, SQRT_5, FRAC_1_SQRT_3, FRAC_1_SQRT_5, FRAC_1_SQRT_PI, FRAC_1_SQRT_2PI,
])]
fn generic_constant_catalog<T: Float>() -> [T; 28] {
    [
        PI,
        FRAC_PI_2,
        FRAC_PI_3,
        FRAC_1_PI,
        FRAC_2_PI,
        FRAC_2_SQRT_PI,
        TAU,
        E,
        LN_2,
        LN_10,
        LOG2_10,
        LOG2_E,
        LOG10_2,
        LOG10_E,
        SQRT_2,
        FRAC_1_SQRT_2,
        PHI,
        FRAC_PI_4,
        FRAC_PI_6,
        FRAC_PI_8,
        GOLDEN_RATIO,
        EULER_GAMMA,
        SQRT_3,
        SQRT_5,
        FRAC_1_SQRT_3,
        FRAC_1_SQRT_5,
        FRAC_1_SQRT_PI,
        FRAC_1_SQRT_2PI,
    ]
}

#[allow(non_snake_case)]
#[generic_float(T, visit_macros = [take])]
fn experimental_identifiers<T: Float>(values: [T; 6]) -> ([T; 6], Vec<T>, T) {
    let [SQRT_3, SQRT_5, FRAC_1_SQRT_3, FRAC_1_SQRT_5, FRAC_1_SQRT_PI, FRAC_1_SQRT_2PI] = values;
    (
        [
            SQRT_3,
            SQRT_5,
            FRAC_1_SQRT_3,
            FRAC_1_SQRT_5,
            FRAC_1_SQRT_PI,
            FRAC_1_SQRT_2PI,
        ],
        vec![
            SQRT_3,
            SQRT_5,
            FRAC_1_SQRT_3,
            FRAC_1_SQRT_5,
            FRAC_1_SQRT_PI,
            FRAC_1_SQRT_2PI,
        ],
        take!(@value SQRT_3),
    )
}

#[allow(non_snake_case)]
#[generic_float(T, constants = [SQRT_3, PI], visit_macros = [take])]
fn selected_experimental<T: Float>(SQRT_5: T) -> [T; 5] {
    [PI, SQRT_3, SQRT_5, vec![SQRT_3][0], take!(@value SQRT_3)]
}

#[allow(non_snake_case)]
#[generic_float(T, constants = [SQRT_3], ignore = [SQRT_3])]
fn ignored_experimental<T: Float>(SQRT_3: T) -> T {
    SQRT_3 + PI
}

#[allow(non_snake_case)]
#[generic_float(T, include = [float], constants = [SQRT_3])]
fn excluded_constants<T: Float>(SQRT_3: T) -> T {
    SQRT_3 + 1.0
}

#[allow(non_snake_case)]
#[generic_float(T, constants = [])]
fn empty_constant_selection<T: Float>(SQRT_3: T) -> T {
    SQRT_3 + PI
}

#[generic_float(T, constants = [SQRT_3], convert = <T as CustomScalar>::from_f64(literal))]
fn custom_experimental<T: CustomScalar>() -> T {
    SQRT_3
}

#[test]
fn experimental_constants_require_explicit_inclusion() {
    let values = [2.0_f64, 3.0, 4.0, 5.0, 6.0, 7.0];
    let (direct, automatic, opaque) = experimental_identifiers(values);
    assert_eq!(direct, values);
    assert_eq!(automatic, values);
    assert_eq!(opaque, values[0]);

    let root = f64::from_bits(0x3ffb_b67a_e858_4caa);
    assert_eq!(
        selected_experimental(42.0_f64),
        [core::f64::consts::PI, root, 42.0, root, root]
    );
    assert_eq!(ignored_experimental(42.0_f64), 42.0 + core::f64::consts::PI);
    assert_eq!(excluded_constants(42.0_f64), 43.0);
    assert_eq!(
        empty_constant_selection(42.0_f64),
        42.0 + core::f64::consts::PI
    );
    assert_eq!(custom_experimental::<f64>().to_bits(), root.to_bits());
    assert_eq!(custom_experimental::<f32>().to_bits(), 0x3fdd_b3d7);
}

#[allow(clippy::approx_constant)]
fn expected_f32_constant_catalog() -> [f32; 28] {
    [
        core::f32::consts::PI,
        core::f32::consts::FRAC_PI_2,
        core::f32::consts::FRAC_PI_3,
        core::f32::consts::FRAC_1_PI,
        core::f32::consts::FRAC_2_PI,
        core::f32::consts::FRAC_2_SQRT_PI,
        core::f32::consts::TAU,
        core::f32::consts::E,
        core::f32::consts::LN_2,
        core::f32::consts::LN_10,
        core::f32::consts::LOG2_10,
        core::f32::consts::LOG2_E,
        core::f32::consts::LOG10_2,
        core::f32::consts::LOG10_E,
        core::f32::consts::SQRT_2,
        core::f32::consts::FRAC_1_SQRT_2,
        1.618_034_f32,
        core::f32::consts::FRAC_PI_4,
        core::f32::consts::FRAC_PI_6,
        core::f32::consts::FRAC_PI_8,
        f32::from_bits(0x3fcf_1bbd),
        f32::from_bits(0x3f13_c468),
        f32::from_bits(0x3fdd_b3d7),
        f32::from_bits(0x400f_1bbd),
        f32::from_bits(0x3f13_cd3a),
        f32::from_bits(0x3ee4_f92e),
        f32::from_bits(0x3f10_6ebb),
        f32::from_bits(0x3ecc_422a),
    ]
}

#[test]
fn generic_constants_and_literals() {
    assert_eq!(circumference(1.0_f64), core::f64::consts::TAU);
    assert_eq!(
        circumference(1.0_f32),
        2.0_f32 * (core::f64::consts::PI as f32)
    );
    assert_eq!(convert_integer(1.0_f64), core::f64::consts::TAU);
    let modulus = 200e9_f64;
    let second_moment = 8.33e-6_f64;
    let length = 2.0_f64;
    assert_eq!(
        euler_buckling_load(modulus, second_moment, length),
        core::f64::consts::PI * core::f64::consts::PI * modulus * second_moment / (length * length)
    );
    assert_eq!(
        integer_context(&[1.0_f64], 2.0_f64),
        5.0 + core::f64::consts::PI
    );
    assert_eq!(constants_only(1.0_f64), 1.0 + core::f64::consts::E);
}

#[test]
fn include_categories_are_independent() {
    assert_eq!(float_only(3.0_f64), 5.5_f64);
    assert_eq!(integer_only(3.0_f32), 6.0_f32);
    assert_eq!(literals_only(3.0_f64), 6.5_f64);
    assert_eq!(
        constants_and_integer(3.0_f64),
        core::f64::consts::PI + 6.0_f64
    );
    assert_eq!(no_replacements(3.0_f32), 2.5_f64);
    assert_eq!(special_constant_only::<f32>(), f32::EPSILON);
    assert_eq!(nested_float_only(3.0_f32), vec![3.0_f32, 1.5_f32]);
}

#[test]
fn custom_conversion_without_num_traits_bound() {
    assert_eq!(custom_scalar(1.0_f32), 2.0_f32 * core::f32::consts::PI);
    assert_eq!(custom_scalar(1.0_f64), core::f64::consts::TAU);
    assert_eq!(
        custom_scalar_ignoring_e(1.0_f64),
        1.0 + core::f64::consts::PI
    );
    assert_eq!(custom_scalar_integer(3.0_f32), 6.0_f32);
    assert_eq!(
        custom_scalar_phi::<f32>().map(f32::to_bits),
        [0x3fcf_1bbd, 0x3fcf_1bbd, 0x3f13_c468]
    );
    assert_eq!(
        custom_scalar_phi::<f64>().map(f64::to_bits),
        [
            0x3ff9_e377_9b97_f4a8,
            0x3ff9_e377_9b97_f4a8,
            0x3fe2_788c_fc6f_b619
        ]
    );
    let expected = 2.0 + core::f64::consts::PI;
    assert_eq!(macro_based_conversion(1.0_f64), expected);
    assert_eq!(nested_macro_conversion(1.0_f64), expected);
}

#[test]
fn macros_visit_expression_arguments_recursively() {
    let nested = nested_vectors::<f32>();
    assert_eq!(nested[0], vec![core::f32::consts::PI, 0.0_f32]);
    assert_eq!(nested[1], vec![core::f32::consts::TAU, 1.0_f32]);

    let repeated = repeated_vectors::<f64>(3);
    assert_eq!(repeated.len(), 3);
    assert!(repeated
        .iter()
        .all(|row| row == &[core::f64::consts::PI, 1.0]));
    assert_eq!(repeated_values::<f32>(3), vec![0.0_f32; 3]);
    assert_eq!(
        macro_with_ignored_identifier(4.0_f64),
        vec![4.0_f64, core::f64::consts::PI]
    );
    assert_eq!(statement_macro::<f64>(), vec![core::f64::consts::PI]);
}

#[test]
fn macros_can_be_ignored_or_left_unparsed() {
    assert_eq!(untouched_macro(1.0_f32), vec![core::f64::consts::PI, 0.0]);
    assert_eq!(unparseable_macro(1.0_f32), 7.0_f64);
    assert_eq!(literal_macro_argument(0.0_f32), 1.5_f64);
    assert_eq!(token_sensitive_macros(0.0_f32), ("PI", true));
}

#[test]
fn selected_macros_rewrite_expression_and_custom_syntax() {
    assert_eq!(selected_case_macros(0.25_f32, 1.0), f32::EPSILON);
    assert_eq!(
        selected_case_macros(f32::INFINITY, 1.0),
        core::f32::consts::FRAC_PI_2
    );
    assert_eq!(selected_case_macros(1.0_f64, 1.0), core::f64::consts::PI);

    let (first, second, third, fourth) = selected_declare_macros::<f32>();
    assert_eq!(first, core::f32::consts::PI + f32::EPSILON);
    assert!(second.is_nan());
    assert_eq!(third, f32::INFINITY);
    assert_eq!(fourth, f32::INFINITY);

    assert_eq!(selected_macro_does_not_change_others(2.0_f64), (2.0, 2.5));
    assert_eq!(
        selected_macro_preserves_nested_sensitive(f32::INFINITY),
        core::f32::consts::PI + 2.5
    );
}

#[test]
fn selected_macros_preserve_assignments_indices_and_repetitions() {
    assert_eq!(selected_single_assignment::<f32>(), 2.0_f32);
    let (selected, repeated) = opaque_macro_protects_integer_contexts::<f32>();
    assert_eq!(selected, 1.5_f32);
    assert_eq!(repeated, vec![core::f32::consts::PI; 2]);
    assert_eq!(qualified_auto_vec::<f32>(), vec![1.5_f32; 2]);
    assert_eq!(qualified_selected_vec::<f64>(), vec![1.5_f64; 2]);
    assert_eq!(
        opaque_macro_tagged_list::<f32>(),
        vec![core::f32::consts::PI, 1.5]
    );
}

#[test]
fn automatic_macros_preserve_concrete_types_and_can_be_skipped() {
    assert_eq!(auto_preserves_explicit_float_types(0.5_f32), 0.5_f32);
    assert_eq!(shadowed_auto_macros::simple_literal::<f32>(), 1.5_f64);
    assert_eq!(
        shadowed_auto_macros::wrapped_literals::<f32>(),
        (1.5, 1.5, -1.5)
    );
    assert!(shadowed_auto_macros::skipped_macro::<f32>());
}

#[test]
fn macro_names_can_match_traversal_mode_names() {
    assert_eq!(
        selected_macros_named_like_modes::<f32>(),
        core::f32::consts::PI + 0.5
    );
    assert_eq!(
        selected_macros_named_like_modes::<f64>(),
        core::f64::consts::PI + 0.5
    );
}

#[test]
fn standard_macros_are_transformed_in_auto_mode() {
    assert_eq!(standard_macros_in_auto_mode(1.0_f32), 1.25_f32);
    assert_eq!(standard_macros_in_auto_mode(1.0_f64), 1.25_f64);
}

#[test]
fn integer_shaped_float_suffixes_obey_float_category() {
    assert_eq!(integer_shaped_float_suffixes::<f32>(), 3.0_f32);
    assert_eq!(integer_shaped_float_suffixes::<f64>(), 3.0_f64);
    assert_eq!(integer_only_leaves_float_suffixes::<f32>(), 1.0_f64);
}

#[test]
fn independent_scopes_and_integer_syntax_are_preserved() {
    assert_eq!(nested_item_scopes(1.0_f64), 8.0);
    assert_eq!(inline_const_scope(1.0_f64), 6.5);
    assert_eq!(negative_minimum_integer::<f64>(), -2147483648.0);
    assert_eq!(
        integer_syntax_contexts::<f64>(),
        core::f64::consts::PI + 2.0 + 3.0 + 4.0
    );
}

#[test]
fn special_constants() {
    let (epsilon, infinity, nan, negative_zero) = specials::<f32>();
    assert_eq!(epsilon, f32::EPSILON);
    assert_eq!(infinity, f32::INFINITY);
    assert!(nan.is_nan());
    assert_eq!(negative_zero.to_bits(), (-0.0_f32).to_bits());

    let (inf, neg_inf, min_val, max_val) = renamed_specials::<f64>();
    assert_eq!(inf, f64::INFINITY);
    assert_eq!(neg_inf, f64::NEG_INFINITY);
    assert_eq!(min_val, f64::MIN);
    assert_eq!(max_val, f64::MAX);
}

#[test]
fn standard_names_and_compatibility_aliases_agree() {
    for pair in pi_aliases::<f32>().as_chunks::<2>().0 {
        assert_eq!(pair[0].to_bits(), pair[1].to_bits());
    }

    let (epsilon, eps_alias, infinity, inf_alias, negative_infinity, negative_inf_alias) =
        special_aliases::<f64>();
    assert_eq!(epsilon, eps_alias);
    assert_eq!(epsilon, f64::EPSILON);
    assert_eq!(infinity, inf_alias);
    assert_eq!(negative_infinity, negative_inf_alias);
    assert_eq!(infinity, f64::INFINITY);
    assert_eq!(negative_infinity, f64::NEG_INFINITY);

    assert_eq!(limits::<f32>(), [f32::MIN, f32::MAX, f32::MIN_POSITIVE]);
    assert_eq!(limits::<f64>(), [f64::MIN, f64::MAX, f64::MIN_POSITIVE]);
    assert_eq!(min_max_are_not_constants(2.0_f64, 3.0_f64), 5.0_f64);
}

#[test]
fn added_f64_constants_match_rust_values() {
    // IEEE 754 encodings of Rust's constants, including those unavailable on 1.88.
    let expected = [
        0x3fe9_21fb_5444_2d18,
        0x3fe0_c152_382d_7366,
        0x3fd9_21fb_5444_2d18,
        0x3ff9_e377_9b97_f4a8,
        0x3fe2_788c_fc6f_b619,
        0x3ffb_b67a_e858_4caa,
        0x4001_e377_9b97_f4a8,
        0x3fe2_79a7_4590_331c,
        0x3fdc_9f25_c5bf_edd9,
        0x3fe2_0dd7_5042_9b6d,
        0x3fd9_8845_33d4_3651,
    ];
    for (value, bits) in generic_constant_catalog::<f64>()[17..].iter().zip(expected) {
        assert_eq!(value.to_bits(), bits);
    }
}

#[test]
fn generic_f32_constants_match_native_f32_values() {
    let generic = generic_constant_catalog::<f32>();
    let native = expected_f32_constant_catalog();
    for (generic, native) in generic.into_iter().zip(native) {
        assert_eq!(generic.to_bits(), native.to_bits());
    }
}

#[test]
fn phi_matches_correctly_rounded_floats() {
    let generic_f32 = generic_constant_catalog::<f32>()[16];
    let native_f32 = expected_f32_constant_catalog()[16];
    let generic_f64 = generic_constant_catalog::<f64>()[16];

    assert_eq!(native_f32.to_bits(), 0x3fcf_1bbd);
    assert_eq!(generic_f32.to_bits(), 0x3fcf_1bbd);
    assert_eq!(generic_f64.to_bits(), 0x3ff9_e377_9b97_f4a8);
}
