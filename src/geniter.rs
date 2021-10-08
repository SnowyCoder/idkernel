use core::{
    ops::{Generator, GeneratorState},
    pin::Pin,
};

use alloc::boxed::Box;

/// a iterator that holds an internal generator representing
/// the iteration state
#[derive(Clone, Debug)]
pub struct GenIter<T>(pub Pin<Box<T>>)
where
    T: Generator<Return = ()>;

impl<T> Iterator for GenIter<T>
where
    T: Generator<Return = ()>,
{
    type Item = T::Yield;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.0.as_mut().resume(()) {
            GeneratorState::Yielded(n) => Some(n),
            GeneratorState::Complete(()) => None,
        }
    }
}

impl<G> From<G> for GenIter<G>
where
    G: Generator<Return = ()>,
{
    #[inline]
    fn from(gen: G) -> Self {
        GenIter(Box::pin(gen))
    }
}
