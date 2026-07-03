use std::{fmt, marker::PhantomData};

pub trait Hook<I> {
    fn trace(&mut self, item: I);

    #[inline]
    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }

    #[inline]
    fn adapt<F, T>(&mut self, f: F) -> Adapter<&mut Self, F>
    where
        Self: Sized,
        F: FnMut(T) -> I,
    {
        Adapter { hook: self, f }
    }

    #[inline]
    // TODO: documentation
    /// …
    ///
    /// This actually returns an [`AutoAdapter`] but we use `impl Hook<T>` for
    /// better type inference. If you need a nameable type or an adapter to
    /// multiple types, use [`AutoAdapter`]'s [`From`] implementation, where
    /// `I` is the original hook's item (`T: Into<I>`).
    /// ``` rust
    /// # use lcrt::trace::{AutoAdapter, Hook};
    /// # type I = ();
    /// # let hook = ();
    /// AutoAdapter::<_, I>::from(hook);
    /// ```
    fn auto_adapt<T>(&mut self) -> impl Hook<T>
    where
        Self: Sized,
        T: Into<I>,
    {
        AutoAdapter::from(self)
    }
}
impl<H, I> Hook<I> for &mut H
where
    H: Hook<I> + ?Sized,
{
    fn trace(&mut self, item: I) {
        (**self).trace(item);
    }
}
// Assert that `Hook` is dyn-compatible
const _: &dyn Hook<()> = &Disabled;

pub type Disabled = ();
#[expect(
    non_upper_case_globals,
    reason = "allow the type name to be used as the unit struct value"
)]
pub const Disabled: Disabled = ();
impl<I> Hook<I> for Disabled {
    #[inline(always)]
    fn trace(&mut self, _item: I) {}
}

pub struct Debug<const STDERR: bool = false>;
impl<I: fmt::Debug, const STDERR: bool> Hook<I> for Debug<STDERR> {
    fn trace(&mut self, item: I) {
        if STDERR {
            eprintln!("{item:?}");
        } else {
            println!("{item:?}");
        }
    }
}

pub struct Adapter<H, F> {
    hook: H,
    f: F,
}
impl<H, F, T, I> Hook<T> for Adapter<H, F>
where
    H: Hook<I>,
    F: FnMut(T) -> I,
{
    #[inline]
    fn trace(&mut self, item: T) {
        self.hook.trace((self.f)(item));
    }
}

pub struct AutoAdapter<H, I> {
    hook: H,
    _item: PhantomData<I>,
}
impl<H, T, I> Hook<T> for AutoAdapter<H, I>
where
    H: Hook<I>,
    T: Into<I>,
{
    #[inline]
    fn trace(&mut self, item: T) {
        self.hook.trace(item.into());
    }
}
impl<H, I> From<H> for AutoAdapter<H, I>
where
    H: Hook<I>,
{
    #[inline]
    fn from(hook: H) -> Self {
        Self {
            hook,
            _item: PhantomData,
        }
    }
}
