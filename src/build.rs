use crate::call_pattern::*;
use crate::clause::{self, ClauseSealed, TerminalClause};
use crate::fn_mocker::PatternMatchMode;
use crate::output::{IntoResponseClone, IntoResponseOnce, Respond, StaticRef};
use crate::property::*;
use crate::*;

use std::marker::PhantomData;
use std::panic;

pub(crate) struct DynCallPatternBuilder {
    pub pattern_match_mode: PatternMatchMode,
    pub input_matcher: DynInputMatcher,
    pub responders: Vec<DynCallOrderResponder>,
    pub count_expectation: counter::CallCountExpectation,
    pub current_response_index: usize,
}

impl DynCallPatternBuilder {
    pub fn new(pattern_match_mode: PatternMatchMode, input_matcher: DynInputMatcher) -> Self {
        Self {
            pattern_match_mode,
            input_matcher,
            responders: vec![],
            count_expectation: Default::default(),
            current_response_index: 0,
        }
    }
}

pub(crate) enum BuilderWrapper<'p> {
    Borrowed(&'p mut DynCallPatternBuilder),
    Owned(DynCallPatternBuilder),
    Stolen,
}

impl<'p> BuilderWrapper<'p> {
    fn steal(&mut self) -> BuilderWrapper<'p> {
        let mut stolen = BuilderWrapper::Stolen;
        std::mem::swap(self, &mut stolen);
        stolen
    }

    pub fn inner(&self) -> &DynCallPatternBuilder {
        match self {
            Self::Borrowed(builder) => builder,
            Self::Owned(builder) => builder,
            Self::Stolen => panic!("builder stolen"),
        }
    }

    fn inner_mut(&mut self) -> &mut DynCallPatternBuilder {
        match self {
            Self::Borrowed(builder) => builder,
            Self::Owned(builder) => builder,
            Self::Stolen => panic!("builder stolen"),
        }
    }

    fn push_responder(&mut self, responder: DynResponder) {
        let dyn_builder = self.inner_mut();
        dyn_builder.responders.push(DynCallOrderResponder {
            response_index: dyn_builder.current_response_index,
            responder,
        })
    }

    /// Note: must be called after `push_responder`
    fn quantify(&mut self, times: usize, exactness: counter::Exactness) {
        let mut builder = self.inner_mut();

        builder.count_expectation.add_to_minimum(times, exactness);
        builder.current_response_index += times;
    }

    fn into_owned(self) -> DynCallPatternBuilder {
        match self {
            Self::Owned(owned) => owned,
            _ => panic!("Tried to turn a non-owned pattern builder into owned"),
        }
    }
}

/// Builder for defining a series of cascading call patterns on a specific [MockFn].
pub struct Each<F: MockFn> {
    patterns: Vec<DynCallPatternBuilder>,
    mock_fn: PhantomData<F>,
}

impl<F> Each<F>
where
    F: MockFn + 'static,
{
    /// Define the next call pattern, given some input matcher.
    ///
    /// The new call pattern will be matched after any previously defined call patterns on the same [Each] instance.
    ///
    /// The method returns a [DefineMultipleResponses], which is used to define how unimock responds to the matched call.
    pub fn call<'e>(
        &'e mut self,
        matching_fn: &dyn Fn(&mut Matching<F>),
    ) -> DefineMultipleResponses<'e, F, InAnyOrder> {
        self.patterns.push(DynCallPatternBuilder::new(
            PatternMatchMode::InAnyOrder,
            DynInputMatcher::from_matching_fn(matching_fn),
        ));

        DefineMultipleResponses {
            builder: BuilderWrapper::Borrowed(self.patterns.last_mut().unwrap()),
            mock_fn: PhantomData,
            ordering: InAnyOrder,
        }
    }

    pub(crate) fn new() -> Self {
        Self {
            patterns: vec![],
            mock_fn: PhantomData,
        }
    }
}

impl<F> ClauseSealed for Each<F>
where
    F: MockFn + 'static,
{
    fn deconstruct(self, sink: &mut dyn clause::TerminalSink) -> Result<(), String> {
        if self.patterns.is_empty() {
            return Err("Stub contained no call patterns".to_string());
        }

        for builder in self.patterns.into_iter() {
            sink.put_terminal(TerminalClause {
                dyn_mock_fn: DynMockFn::new::<F>(),
                builder,
            })?;
        }

        Ok(())
    }
}

/// A matched call pattern, ready for defining a single response.
pub struct DefineResponse<'p, F: MockFn, O: Ordering> {
    builder: BuilderWrapper<'p>,
    mock_fn: PhantomData<F>,
    ordering: O,
}

impl<'p, F: MockFn, O: Ordering> DefineResponse<'p, F, O>
where
    <F::Response as Respond>::Type: Send + Sync,
{
    /// Specify the output of the call pattern by providing a value.
    /// The output type cannot contain non-static references.
    /// It must also be [Send] and [Sync] because unimock needs to store it.
    ///
    /// Unless explicitly configured on the returned [QuantifyReturnValue], the return value specified here
    ///     can be returned only once, because this method does not require a [Clone] bound.
    /// To be able to return this value multiple times, quantify it explicitly.
    pub fn returns<T>(self, value: T) -> QuantifyReturnValue<'p, F, T, O>
    where
        T: IntoResponseOnce<F::Response>,
    {
        QuantifyReturnValue {
            builder: self.builder,
            return_value: Some(value),
            mock_fn: self.mock_fn,
            ordering: self.ordering,
        }
    }
}

/// A matched call pattern, ready for defining multiple response, requiring return values to implement [Clone].
pub struct DefineMultipleResponses<'p, F: MockFn, O: Ordering> {
    builder: BuilderWrapper<'p>,
    mock_fn: PhantomData<F>,
    ordering: O,
}

impl<'p, F, O> DefineMultipleResponses<'p, F, O>
where
    F: MockFn + 'static,
    O: Ordering,
{
    /// Specify the output of the call pattern by providing a value.
    /// The output type cannot contain non-static references.
    /// It must also be [Send] and [Sync] because unimock needs to store it, and [Clone] because it should be able to be returned multiple times.
    pub fn returns<V: IntoResponseClone<F::Response>>(mut self, value: V) -> Quantify<'p, F, O> {
        self.builder
            .push_responder(value.into_clone_responder::<F>().0);
        self.quantify()
    }
}

macro_rules! define_response_common_impl {
    ($typename:ident) => {
        impl<'p, F, O> $typename<'p, F, O>
        where
            F: MockFn + 'static,
            O: Ordering,
        {
            /// Create a new owned call pattern match.
            pub(crate) fn with_owned_builder(
                input_matcher: DynInputMatcher,
                pattern_match_mode: PatternMatchMode,
                ordering: O,
            ) -> Self {
                Self {
                    builder: BuilderWrapper::Owned(DynCallPatternBuilder::new(
                        pattern_match_mode,
                        input_matcher,
                    )),
                    mock_fn: PhantomData,
                    ordering,
                }
            }

            /// Specify the response of the call pattern by calling `Default::default()`.
            pub fn returns_default(mut self) -> Quantify<'p, F, O>
            where
                <F::Response as Respond>::Type: Default,
            {
                self.builder.push_responder(
                    FunctionResponder::<F> {
                        func: Box::new(|_| Default::default()),
                    }
                    .into_dyn_responder(),
                );
                self.quantify()
            }

            /// Specify the response of the call pattern by invoking the given closure that can then compute it based on input parameters.
            pub fn answers<C, R>(mut self, func: C) -> Quantify<'p, F, O>
            where
                C: (for<'i> Fn(F::Inputs<'i>) -> R) + Send + Sync + 'static,
                R: IntoResponseOnce<F::Response>,
            {
                self.builder.push_responder(
                    FunctionResponder::<F> {
                        func: Box::new(move |inputs| func(inputs).into_response()),
                    }
                    .into_dyn_responder(),
                );
                self.quantify()
            }

            /// Specify the response of the call pattern to be a static reference to leaked memory.
            ///
            /// The value may be based on the value of input parameters.
            ///
            /// This version will produce a new memory leak for _every invocation_ of the answer function.
            ///
            /// This method should only be used when computing a reference based
            /// on input parameters is necessary, which should not be a common use case.
            pub fn answers_leaked_ref<C, R, T>(mut self, func: C) -> Quantify<'p, F, O>
            where
                F: MockFn<Response = StaticRef<T>>,
                C: (for<'i> Fn(F::Inputs<'i>) -> R) + Send + Sync + 'static,
                R: std::borrow::Borrow<T> + 'static,
                T: 'static,
            {
                self.builder.push_responder(
                    FunctionResponder::<F> {
                        func: Box::new(move |inputs| {
                            let value = func(inputs);
                            let leaked_ref = Box::leak(Box::new(value));

                            <R as std::borrow::Borrow<T>>::borrow(leaked_ref)
                        }),
                    }
                    .into_dyn_responder(),
                );
                self.quantify()
            }

            /// Prevent this call pattern from succeeding by explicitly panicking with a custom message.
            pub fn panics(mut self, message: impl Into<String>) -> Quantify<'p, F, O> {
                let message = message.into();
                self.builder.push_responder(DynResponder::Panic(message));
                self.quantify()
            }

            /// Instruct this call pattern to invoke its corresponding `unmocked` function.
            ///
            /// For this to work, the mocked trait must be configured with an `unmock_with=[..]` parameter.
            /// If unimock doesn't find a way to unmock the function, this will panic when the function is called.
            pub fn unmocked(mut self) -> Quantify<'p, F, O> {
                self.builder.push_responder(DynResponder::Unmock);
                self.quantify()
            }

            fn quantify(self) -> Quantify<'p, F, O> {
                Quantify {
                    builder: self.builder,
                    mock_fn: PhantomData,
                    ordering: self.ordering,
                }
            }
        }
    };
}

define_response_common_impl!(DefineResponse);
define_response_common_impl!(DefineMultipleResponses);

/// Builder for defining how a call pattern with an explicit return value gets verified with regards to quantification/counting.
pub struct QuantifyReturnValue<'p, F, T, O>
where
    F: MockFn,
    T: IntoResponseOnce<F::Response>,
{
    pub(crate) builder: BuilderWrapper<'p>,
    return_value: Option<T>,
    mock_fn: PhantomData<F>,
    ordering: O,
}

impl<'p, F, T, O> QuantifyReturnValue<'p, F, T, O>
where
    F: MockFn,
    T: IntoResponseOnce<F::Response>,
    O: Copy,
{
    /// Expect this call pattern to be matched exactly once.
    ///
    /// This is the only quantifier that works together with return values that don't implement [Clone].
    pub fn once(mut self) -> QuantifiedResponse<'p, F, O, Exact> {
        self.builder.push_responder(
            self.return_value
                .take()
                .unwrap()
                .into_once_responder::<F>()
                .0,
        );
        self.builder.quantify(1, counter::Exactness::Exact);
        QuantifiedResponse {
            builder: self.builder.steal(),
            mock_fn: PhantomData,
            ordering: self.ordering,
            _repetition: Exact,
        }
    }

    /// Expect this call pattern to be matched exactly the specified number of times.
    pub fn n_times(mut self, times: usize) -> QuantifiedResponse<'p, F, O, Exact>
    where
        T: IntoResponseClone<F::Response>,
    {
        self.builder.push_responder(
            self.return_value
                .take()
                .unwrap()
                .into_clone_responder::<F>()
                .0,
        );
        self.builder.quantify(times, counter::Exactness::Exact);
        QuantifiedResponse {
            builder: self.builder.steal(),
            mock_fn: PhantomData,
            ordering: self.ordering,
            _repetition: Exact,
        }
    }

    /// Expect this call pattern to be matched at least the specified number of times.
    ///
    /// This only works for call patterns matched in any ordered.
    /// Strictly ordered call patterns must have exact quantification.
    pub fn at_least_times(mut self, times: usize) -> QuantifiedResponse<'p, F, O, AtLeast>
    where
        T: IntoResponseClone<F::Response>,
        O: Ordering<Kind = InAnyOrder>,
    {
        self.builder.push_responder(
            self.return_value
                .take()
                .unwrap()
                .into_clone_responder::<F>()
                .0,
        );
        self.builder.quantify(times, counter::Exactness::AtLeast);
        QuantifiedResponse {
            builder: self.builder.steal(),
            mock_fn: PhantomData,
            ordering: self.ordering,
            _repetition: AtLeast,
        }
    }
}

impl<'p, F, T, O> ClauseSealed for QuantifyReturnValue<'p, F, T, O>
where
    F: MockFn,
    T: IntoResponseOnce<F::Response>,
    O: Copy + Ordering,
{
    fn deconstruct(self, sink: &mut dyn clause::TerminalSink) -> Result<(), String> {
        self.once().deconstruct(sink)
    }
}

/// The drop implementation of this is intended to run when used in a stubbing clause,
/// when the call pattern is left unquantified by the user.
///
/// In that case, it is only able to return once, because no [Clone] bound has been
/// part of any construction step.
impl<'p, F, T, O> Drop for QuantifyReturnValue<'p, F, T, O>
where
    F: MockFn,
    T: IntoResponseOnce<F::Response>,
{
    fn drop(&mut self) {
        if let Some(return_value) = self.return_value.take() {
            self.builder
                .push_responder(return_value.into_once_responder::<F>().0);
        }
    }
}

/// Builder for defining how a call pattern gets verified with regards to quantification/counting.
pub struct Quantify<'p, F: MockFn, O> {
    pub(crate) builder: BuilderWrapper<'p>,
    mock_fn: PhantomData<F>,
    ordering: O,
}

impl<'p, F, O> Quantify<'p, F, O>
where
    F: MockFn + 'static,
    O: Ordering,
{
    /// Expect this call pattern to be matched exactly once.
    pub fn once(mut self) -> QuantifiedResponse<'p, F, O, Exact> {
        self.builder.quantify(1, counter::Exactness::Exact);
        self.into_exact()
    }

    /// Expect this call pattern to be matched exactly the specified number of times.
    pub fn n_times(mut self, times: usize) -> QuantifiedResponse<'p, F, O, Exact> {
        self.builder.quantify(times, counter::Exactness::Exact);
        self.into_exact()
    }

    /// Expect this call pattern to be matched at least the specified number of times.
    pub fn at_least_times(mut self, times: usize) -> QuantifiedResponse<'p, F, O, AtLeast> {
        self.builder.quantify(times, counter::Exactness::AtLeast);
        QuantifiedResponse {
            builder: self.builder,
            mock_fn: PhantomData,
            ordering: self.ordering,
            _repetition: AtLeast,
        }
    }

    fn into_exact(self) -> QuantifiedResponse<'p, F, O, Exact> {
        QuantifiedResponse {
            builder: self.builder,
            mock_fn: PhantomData,
            ordering: self.ordering,
            _repetition: Exact,
        }
    }
}

impl<'p, F, O> ClauseSealed for Quantify<'p, F, O>
where
    F: MockFn + 'static,
    O: Ordering,
{
    fn deconstruct(mut self, sink: &mut dyn clause::TerminalSink) -> Result<(), String> {
        if self.builder.inner().pattern_match_mode == PatternMatchMode::InOrder {
            self.builder.quantify(1, counter::Exactness::Exact);
        }

        sink.put_terminal(TerminalClause {
            dyn_mock_fn: DynMockFn::new::<F>(),
            builder: self.builder.into_owned(),
        })
    }
}

/// An exactly quantified response, i.e. the number of times it is expected to respond is an exact number.
pub struct QuantifiedResponse<'p, F: MockFn, O, R> {
    builder: BuilderWrapper<'p>,
    mock_fn: PhantomData<F>,
    ordering: O,
    _repetition: R,
}

impl<'p, F, O, R> QuantifiedResponse<'p, F, O, R>
where
    F: MockFn + 'static,
    O: Ordering,
    R: Repetition,
{
    /// Prepare to set up a new response, which will take effect after the current response has been yielded.
    /// In order to make an output sequence, the preceding output must be exactly quantified.
    pub fn then(mut self) -> DefineMultipleResponses<'p, F, O>
    where
        R: Repetition<Kind = Exact>,
    {
        // Opening for a new response, which will be non-exactly quantified unless otherwise specified, set the exactness to AtLeastPlusOne now.
        // The reason it is AtLeastPlusOne is the additive nature.
        // We do not want to add anything to the number now, because it could be added to later in Quantify.
        // We just want to express that when using `then`, it should be matched at least one time, if not `then` would be unnecessary.
        self.builder
            .inner_mut()
            .count_expectation
            .add_to_minimum(0, counter::Exactness::AtLeastPlusOne);

        DefineMultipleResponses {
            builder: self.builder,
            mock_fn: PhantomData,
            ordering: self.ordering,
        }
    }
}

impl<'p, F, O, R> ClauseSealed for QuantifiedResponse<'p, F, O, R>
where
    F: MockFn + 'static,
    O: Ordering,
    R: Repetition,
{
    fn deconstruct(self, sink: &mut dyn clause::TerminalSink) -> Result<(), String> {
        sink.put_terminal(TerminalClause {
            dyn_mock_fn: DynMockFn::new::<F>(),
            builder: self.builder.into_owned(),
        })
    }
}
