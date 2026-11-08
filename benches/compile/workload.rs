use num_lazy_proc::generic_float;
use num_traits::Float;

// The runner repeats this function with distinct names.
#[generic_float(T)]
pub fn sample<T: Float>(x: T) -> T {
    (PI + TAU + E + LN_2 + LN_10 + SQRT_2 + FRAC_PI_2 + FRAC_1_PI + x) * 0.5
}
