use std::fmt;

pub trait Hook<I> {
    fn trace(&mut self, item: I);
    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
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
