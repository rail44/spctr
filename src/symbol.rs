use lasso::{Spur, ThreadedRodeo};
use std::sync::OnceLock;

pub type Symbol = Spur;

static INTERNER: OnceLock<ThreadedRodeo> = OnceLock::new();

fn interner() -> &'static ThreadedRodeo {
    INTERNER.get_or_init(ThreadedRodeo::default)
}

pub fn intern(s: &str) -> Symbol {
    interner().get_or_intern(s)
}

pub fn display(s: Symbol) -> &'static str {
    interner().resolve(&s)
}
