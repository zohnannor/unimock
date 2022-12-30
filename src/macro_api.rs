use crate::call_pattern::InputIndex;
use crate::debug::{self, filter_questionmark};
use crate::mismatch::{Mismatch, MismatchKind};
use crate::output::Output;
use crate::{call_pattern::MatchingFn, call_pattern::MatchingFnDebug, *};

/// The evaluation of a [MockFn].
///
/// Used to tell trait implementations whether to do perform their own evaluation of a call.
///
/// The output is generic, because both owned and referenced output are supported.
pub enum Evaluation<'u, 'i, F: MockFn> {
    /// Function evaluated to its output.
    Evaluated(<F::Output<'u> as Output<'u, F::Response>>::Type),
    /// Function not yet evaluated.
    Skipped(F::Inputs<'i>),
}

impl<'u, 'i, F: MockFn> Evaluation<'u, 'i, F> {
    /// Unwrap the `Evaluated` variant, or panic.
    /// The unimock instance must be passed in order to register that an eventual panic happened.
    pub fn unwrap(self, unimock: &Unimock) -> <F::Output<'u> as Output<'u, F::Response>>::Type {
        match self {
            Self::Evaluated(output) => output,
            Self::Skipped(_) => panic!(
                "{}",
                unimock
                    .shared_state
                    .prepare_panic(error::MockError::CannotUnmock { name: F::NAME })
            ),
        }
    }
}

/// A builder for argument matchers.
pub struct Matching<F: MockFn> {
    pub(crate) mock_fn: std::marker::PhantomData<F>,
    pub(crate) matching_fn: Option<MatchingFn<F>>,
    pub(crate) matching_fn_debug: Option<MatchingFnDebug<F>>,
    pub(crate) matcher_debug: Option<debug::InputMatcherDebug>,
}

impl<F> Matching<F>
where
    F: MockFn,
{
    pub(crate) fn new() -> Self {
        Self {
            mock_fn: std::marker::PhantomData,
            matching_fn: None,
            matching_fn_debug: None,
            matcher_debug: None,
        }
    }

    /// Set the matching function, with debug capabilities.
    ///
    /// The function should accept a reference to inputs as argument, and return a boolean answer representing match or no match.
    ///
    /// The function also receives a [MismatchReporter]
    #[inline]
    pub fn debug_func<M>(&mut self, matching_fn: M)
    where
        M: (for<'i> Fn(&F::Inputs<'i>, &mut MismatchReporter) -> bool) + Send + Sync + 'static,
    {
        self.matching_fn_debug = Some(MatchingFnDebug(Box::new(matching_fn)));
    }

    /// Set the matching function.
    ///
    /// The function should accept a reference to inputs as argument, and return a boolean answer representing match or no match.
    #[inline]
    pub fn func<M>(&mut self, matching_fn: M)
    where
        M: (for<'i> Fn(&F::Inputs<'i>) -> bool) + Send + Sync + 'static,
    {
        self.matching_fn = Some(MatchingFn(Box::new(matching_fn)));
    }

    /// Register debug info on the matching builder.
    ///
    /// This way, a mismatch may be easier to debug, as the debug info can be printed as part of panic messages.
    pub fn pat_debug(&mut self, pat_debug: &'static str, file: &'static str, line: u32) {
        self.matcher_debug = Some(debug::InputMatcherDebug {
            pat_debug,
            file,
            line,
        });
    }
}

/// A reporter used in call pattern matchers in case of mismatched inputs.
///
/// This is a diagnostics tool leading to higher quality error messages.
///
/// Used by the [matching] macro.
pub struct MismatchReporter {
    enabled: bool,
    pub(crate) mismatches: Vec<(InputIndex, Mismatch)>,
}

impl MismatchReporter {
    pub(crate) fn new_enabled() -> Self {
        Self {
            enabled: true,
            mismatches: vec![],
        }
    }

    pub(crate) fn new_disabled() -> Self {
        Self {
            enabled: false,
            mismatches: vec![],
        }
    }

    /// Whether debugging is enabled
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Register failure to match a pattern
    #[deprecated]
    pub fn pat_fail(
        &mut self,
        input_index: usize,
        actual: impl Into<String>,
        expected: impl Into<String>,
    ) {
        self.pat_fail_opt_debug(input_index, filter_questionmark(actual.into()), expected);
    }

    /// Register failure to match a pattern
    pub fn pat_fail_opt_debug(
        &mut self,
        input_index: usize,
        actual: Option<impl Into<String>>,
        expected: impl Into<String>,
    ) {
        self.mismatches.push((
            InputIndex(input_index),
            Mismatch {
                kind: MismatchKind::Pattern,
                actual: actual.map(|dbg| dbg.into()),
                expected: expected.into(),
            },
        ));
    }

    /// Register failure for an eq check
    #[deprecated]
    pub fn eq_fail(
        &mut self,
        input_index: usize,
        actual: impl Into<String>,
        expected: impl Into<String>,
    ) {
        self.eq_fail_opt_debug(input_index, filter_questionmark(actual.into()), expected);
    }

    /// Register failure to match a pattern
    pub fn eq_fail_opt_debug(
        &mut self,
        input_index: usize,
        actual: Option<impl Into<String>>,
        expected: impl Into<String>,
    ) {
        self.mismatches.push((
            InputIndex(input_index),
            Mismatch {
                kind: MismatchKind::Eq,
                actual: actual.map(|dbg| dbg.into()),
                expected: expected.into(),
            },
        ));
    }

    /// Register failure for an ne check
    #[deprecated]
    pub fn ne_fail(
        &mut self,
        input_index: usize,
        actual: impl Into<String>,
        expected: impl Into<String>,
    ) {
        self.ne_fail_opt_debug(input_index, filter_questionmark(actual.into()), expected);
    }

    /// Register failure for an ne check
    pub fn ne_fail_opt_debug(
        &mut self,
        input_index: usize,
        actual: Option<impl Into<String>>,
        expected: impl Into<String>,
    ) {
        self.mismatches.push((
            InputIndex(input_index),
            Mismatch {
                kind: MismatchKind::Ne,
                actual: actual.map(|dbg| dbg.into()),
                expected: expected.into(),
            },
        ));
    }
}

/// Evaluate a [MockFn] given some inputs, to produce its output.
#[track_caller]
pub fn eval<'u, 'i, F>(unimock: &'u Unimock, inputs: F::Inputs<'i>) -> Evaluation<'u, 'i, F>
where
    F: MockFn + 'static,
{
    unimock.handle_error(eval::eval(&unimock.shared_state, inputs))
}

/// Trait for computing the proper [std::fmt::Debug] representation of a value.
pub trait ProperDebug {
    /// Format a debug representation.
    fn unimock_try_debug(&self) -> String;

    /// Optionally format a debug representation.
    fn unimock_try_debug_opt(&self) -> Option<String>;
}

/// Fallback trait (using autoref specialization) for returning `"?"` when the implementing value does not implement [std::fmt::Debug].
pub trait NoDebug {
    /// Format a debug representation.
    fn unimock_try_debug(&self) -> String;

    /// Optionally format a debug representation.
    fn unimock_try_debug_opt(&self) -> Option<String>;
}

// Autoref specialization:
// https://github.com/dtolnay/case-studies/blob/master/autoref-specialization/README.md

impl<T: std::fmt::Debug> ProperDebug for T {
    fn unimock_try_debug(&self) -> String {
        format!("{self:?}")
    }

    fn unimock_try_debug_opt(&self) -> Option<String> {
        Some(format!("{self:?}"))
    }
}

impl<T> NoDebug for &T {
    fn unimock_try_debug(&self) -> String {
        "?".to_string()
    }

    fn unimock_try_debug_opt(&self) -> Option<String> {
        None
    }
}

/// Take a vector of strings, comma separate and put within parentheses.
pub fn format_inputs(inputs: &[String]) -> String {
    let joined = inputs.join(", ");
    format!("({joined})")
}

/// Convert any type implementing `AsRef<str>` to a `&str`.
pub fn as_str_ref<T>(input: &T) -> &str
where
    T: AsRef<str>,
{
    input.as_ref()
}

/// Convert any type implementing `AsRef<[I]>` to a `&[I]`.
pub fn as_slice<T, I>(input: &T) -> &[I]
where
    T: AsRef<[I]>,
{
    input.as_ref()
}
