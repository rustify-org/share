use std::marker::PhantomData;

use super::{BaseInfo, BaseTransformer, BaseVNode, ConvertInfo, CoreTransformer, Js, C};
use crate::Name;
use rustc_hash::FxHashMap;

macro_rules! impl_enter {
    ($impl: ident) => {
        $impl!(enter_root, IRRoot);
        $impl!(enter_text, TextIR);
        $impl!(enter_if, IfNodeIR);
        $impl!(enter_for, ForNodeIR);
        $impl!(enter_vnode, VNodeIR);
        $impl!(enter_slot_outlet, RenderSlotIR);
        $impl!(enter_v_slot, VSlotIR);
        $impl!(enter_slot_fn, Slot);
        $impl!(enter_cache, CacheIR);
        $impl!(enter_js_expr, JsExpression);
        $impl!(enter_fn_param, JsExpression);
        $impl!(enter_comment, CommentType);
        $impl!(enter_hoisted, HoistedIndex);
    };
}
macro_rules! impl_exit {
    ($impl: ident) => {
        $impl!(exit_root, IRRoot);
        $impl!(exit_text, TextIR);
        $impl!(exit_if, IfNodeIR);
        $impl!(exit_for, ForNodeIR);
        $impl!(exit_vnode, VNodeIR);
        $impl!(exit_slot_outlet, RenderSlotIR);
        $impl!(exit_v_slot, VSlotIR);
        $impl!(exit_slot_fn, Slot);
        $impl!(exit_cache, CacheIR);
        $impl!(exit_js_expr, JsExpression);
        $impl!(exit_fn_param, JsExpression);
        $impl!(exit_comment, CommentType);
        $impl!(exit_hoisted, HoistedIndex);
    };
}
macro_rules! noop_pass {
    ($method: ident, $ty: ident) => {
        #[inline]
        fn $method(&mut self, _r: &mut C::$ty<T>) {}
    };
}

pub trait CorePass<T: ConvertInfo> {
    impl_enter!(noop_pass);
    impl_exit!(noop_pass);
    // macro output example:
    // fn enter_root(&mut self, _: &mut IRRoot<T>) {}
    // fn exit_root(&mut self, _: &mut IRRoot<T>) {}
}

macro_rules! chain_enter {
    ($method: ident, $ty: ident) => {
        #[inline]
        fn $method(&mut self, r: &mut C::$ty<T>) {
            self.first.$method(r);
            self.second.$method(r);
        }
    };
}
macro_rules! chain_exit {
    ($method: ident, $ty: ident) => {
        #[inline]
        fn $method(&mut self, r: &mut C::$ty<T>) {
            self.second.$method(r);
            self.first.$method(r);
        }
    };
}

pub struct Chain<A, B> {
    pub first: A,
    pub second: B,
}
impl<T, A, B> CorePass<T> for Chain<A, B>
where
    T: ConvertInfo,
    A: CorePass<T>,
    B: CorePass<T>,
{
    impl_enter!(chain_enter);
    impl_exit!(chain_exit);
    // macro output example:
    // #[inline]
    // fn enter_root(&mut self, r: &mut IRRoot<T>) {
    //     self.first.enter_root(r);
    //     self.second.enter_root(r);
    // }
    // #[inline]
    // fn exit_root(&mut self, r: &mut IRRoot<T>) {
    //     self.second.exit_root(r);
    //     self.first.exit_root(r);
    // }
}

/// Chains multiple transform pass.
#[macro_export]
macro_rules! chain {
    ($a:expr, $b:expr) => {{
        use $crate::Chain;

        Chain {
            first: $a,
            second: $b,
        }
    }};

    ($a:expr, $b:expr,) => {
        chain!($a, $b)
    };

    ($a:expr, $b:expr,  $($rest:tt)+) => {{
        use $crate::Chain;

        Chain {
            first: $a,
            second: chain!($b, $($rest)*),
        }
    }};
}

type Identifiers<'a> = FxHashMap<Name<'a>, usize>;
#[derive(Default)]
pub struct Scope<'a> {
    pub identifiers: Identifiers<'a>,
}

/// Check if an IR contains expressions that reference current context scope ids
/// e.g. identifiers referenced in the scope can skip prefixing
// TODO: has_ref will repeatedly call on vnode regardless if new ids are introduced.
// So it's a O(d^2) complexity where d is the depth of nested v-slot component.
// we can optimize it by tracking how many IDs are introduced and skip unnecessary call
// in practice it isn't a problem because stack overflow happens way faster :/
impl<'a> Scope<'a> {
    pub fn has_identifier(&self, id: Name<'a>) -> bool {
        self.identifiers.contains_key(id)
    }
    pub fn add_identifier(&mut self, id: Name<'a>) {
        *self.identifiers.entry(id).or_default() += 1;
    }
    pub fn remove_identifier(&mut self, id: Name<'a>) {
        *self.identifiers.entry(id).or_default() -= 1;
    }
    pub fn has_ref_in_vnode(&self, node: &mut BaseVNode<'a>) -> bool {
        if self.identifiers.is_empty() {
            return false;
        }
        let mut ref_finder = RefFinder(&self.identifiers, false);
        BaseTransformer::transform_vnode(node, &mut ref_finder);
        ref_finder.1
    }
    pub fn has_ref_in_expr(&self, exp: &mut Js<'a>) -> bool {
        if self.identifiers.is_empty() {
            return false;
        }
        let mut ref_finder = RefFinder(&self.identifiers, false);
        BaseTransformer::transform_js_expr(exp, &mut ref_finder);
        ref_finder.1
    }
}
struct RefFinder<'a, 'b>(&'b Identifiers<'a>, bool);
// TODO: implement interruptible transformer for early return
// TODO: current implementation has false alarms in code like below
// <comp v-for="a in source">
//  <p v-for="a in s">{{a}}</p> <- expect stable, got dynamic
// </comp>
// but it is fine since ref_usage is only for optimization
impl<'a, 'b> CorePass<BaseInfo<'a>> for RefFinder<'a, 'b> {
    fn enter_js_expr(&mut self, e: &mut Js<'a>) {
        if let Js::Simple(e, _) = e {
            if self.0.contains_key(e.raw) {
                self.1 = true;
            }
        }
    }
}
macro_rules! noop_pass_ext {
    ($method: ident, $ty: ident) => {
        #[inline]
        fn $method(&mut self, _: &mut C::$ty<T>, _: &mut Shared) {}
    };
}

pub trait CorePassExt<T: ConvertInfo, Shared> {
    impl_enter!(noop_pass_ext);
    impl_exit!(noop_pass_ext);
    // example expand
    // fn enter_js_expr(&mut self, _: &mut T::JsExpression, _: &mut Shared) {}
    // fn exit_js_expr(&mut self, _: &mut T::JsExpression, _: &mut Shared) {}
}

macro_rules! chain_enter_ext {
    ($method: ident, $ty: ident) => {
        #[inline]
        fn $method(&mut self, r: &mut C::$ty<T>, s: &mut Shared) {
            self.first.$method(r, s);
            self.second.$method(r, s);
        }
    };
}
macro_rules! chain_exit_ext {
    ($method: ident, $ty: ident) => {
        #[inline]
        fn $method(&mut self, r: &mut C::$ty<T>, s: &mut Shared) {
            self.second.$method(r, s);
            self.first.$method(r, s);
        }
    };
}
impl<T, A, B, Shared> CorePassExt<T, Shared> for Chain<A, B>
where
    T: ConvertInfo,
    A: CorePassExt<T, Shared>,
    B: CorePassExt<T, Shared>,
{
    impl_enter!(chain_enter_ext);
    impl_exit!(chain_exit_ext);
}

pub struct SharedInfoPasses<T, Passes, Shared>
where
    T: ConvertInfo,
    Passes: CorePassExt<T, Shared>,
{
    pub passes: Passes,
    pub shared_info: Shared,
    pub pd: PhantomData<T>,
}

macro_rules! shared_pass_impl {
    ($method: ident, $ty: ident) => {
        #[inline]
        fn $method(&mut self, e: &mut C::$ty<T>) {
            let shared = &mut self.shared_info;
            self.passes.$method(e, shared);
        }
    };
}

impl<T, P, Shared> CorePass<T> for SharedInfoPasses<T, P, Shared>
where
    T: ConvertInfo,
    P: CorePassExt<T, Shared>,
{
    impl_enter!(shared_pass_impl);
    impl_exit!(shared_pass_impl);
}
