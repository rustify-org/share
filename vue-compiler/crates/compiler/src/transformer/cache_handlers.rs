/// cache handlers so that it's always the same handler being passed down.
/// this avoids unnecessary re-renders when users use inline handlers on
/// components. NB. requires prefix_identifiers
use super::{BaseInfo, BaseVNode, CorePassExt, BaseCache, Scope};
use crate::{
    converter::v_on::get_handler_type,
    flags::StaticLevel,
    ir::{JsExpr as Js, CacheKind, HandlerType},
};

pub struct CacheHandlers {
    in_v_once: bool,
    is_component: bool,
    cache_handlers: bool,
}
impl CacheHandlers {
    pub fn new(cache_handlers: bool) -> Self {
        Self {
            in_v_once: false,
            is_component: false,
            cache_handlers,
        }
    }
}

impl<'a> CorePassExt<BaseInfo<'a>, Scope<'a>> for CacheHandlers {
    fn enter_cache(&mut self, cn: &mut BaseCache<'a>, _: &mut Scope<'a>) {
        if matches!(cn.kind, CacheKind::Once) {
            self.in_v_once = true;
        }
    }
    fn exit_cache(&mut self, cn: &mut BaseCache<'a>, _: &mut Scope<'a>) {
        if matches!(cn.kind, CacheKind::Once) {
            self.in_v_once = false;
        }
    }
    fn enter_vnode(&mut self, vn: &mut BaseVNode<'a>, _: &mut Scope<'a>) {
        self.is_component = vn.is_component;
    }
    #[allow(clippy::nonminimal_bool)]
    fn enter_js_expr(&mut self, exp: &mut Js<'a>, scope: &mut Scope<'a>) {
        // unnecessary to cache inside v-once
        if !self.cache_handlers || self.in_v_once {
            return;
        }
        let ty = match exp {
            Js::FuncSimple { src, .. } => get_handler_type(*src),
            Js::FuncCompound { ty, .. } => ty.clone(),
            _ => return,
        };
        let is_member_exp = matches!(ty, HandlerType::MemberExpr);
        let should_cache =
            // #1541 bail if this is a member exp handler passed to a component -
            // we need to use the original function to preserve arity,
            // e.g. <transition> relies on checking cb.length to determine
            // transition end handling. Inline function is ok since its arity
            // is preserved even when cached.
            !(is_member_exp && self.is_component) &&
            // bail if the function references closure variables (v-for, v-slot)
            // it must be passed fresh to avoid stale values.
            !scope.has_ref_in_expr(exp) &&
            // runtime constants don't need to be cached
            // (this is analyzed by compileScript in SFC <script setup>)
            exp.static_level() > StaticLevel::NotStatic;
        let cache = match exp {
            Js::FuncSimple { cache, .. } | Js::FuncCompound { cache, .. } => cache,
            _ => return,
        };
        *cache = should_cache;
    }
}
