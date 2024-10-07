//! There is still a lot we can optimize VStr
//! * instead of using &str, we can use intern to cache static attr name.
//! * we can also cache camelize/capitalize result.
//! * if VStr raw already satisfy StrOps, setting the ops flag is noop.
//! * interning/cache can be optional, e.g. Text Token can skip it at all.
use super::{
    is_event_prop, non_whitespace, not_js_identifier, json::write_json_string,
    decode_html::decode_entities,
};
use bitflags::bitflags;
use std::{
    fmt::{self, Write},
    ops::Deref,
};

bitflags! {
    /// Represents string manipulation. It has two categories:
    /// 1. IDEMPOTENT_OPS and 2. AFFINE_OPS,
    /// depending on whether the manipulation is idempotent or not
    /// NB strops is order sensitive when it is cast to string.
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct StrOps: u16 {
        const HANDLER_KEY         = 1 << 0;
        const MODEL_HANDLER       = 1 << 1;
        const VALID_DIR           = 1 << 2;
        const VALID_COMP          = 1 << 3;
        const V_DIR_PREFIX        = 1 << 4;
        const COMPRESS_WHITESPACE = 1 << 5;
        const DECODE_ENTITY       = 1 << 6;
        const CAMEL_CASE          = 1 << 7;
        const CAPITALIZED         = 1 << 8;
        const JS_STRING           = 1 << 9;
        const CTX_PREFIX          = 1 << 11;
        const MOD_SUFFIX          = 1 << 12;
        const ASSIGN_EVT          = 1 << 13;
        // marker op is placed at the end
        const SELF_SUFFIX         = 1 << 14;
        const DECODE_ATTR         = 1 << 15;
        /// Ops that can be safely carried out multiple times
        const IDEMPOTENT_OPS =
            Self::COMPRESS_WHITESPACE.bits() | Self::DECODE_ENTITY.bits() |
            Self::CAMEL_CASE.bits() | Self::CAPITALIZED.bits() | Self::DECODE_ATTR.bits();
        /// Ops that can only be performed at most once. Name comes from
        /// https://en.wikipedia.org/wiki/Substructural_type_system
        const AFFINE_OPS =
            Self::HANDLER_KEY.bits() | Self::MODEL_HANDLER.bits() | Self::VALID_DIR.bits() |
            Self::VALID_COMP.bits() | Self::SELF_SUFFIX.bits() | Self::V_DIR_PREFIX.bits() |
            Self::JS_STRING.bits() | Self::CTX_PREFIX.bits();
        /// Ops that mark the string is an hoisted asset
        const ASSET_OPS = Self::VALID_DIR.bits() | Self::VALID_COMP.bits() |
            Self::SELF_SUFFIX.bits();
    }
}

// NB: JS word boundary is `\w`: `[a-zA-Z0-9-]`.
fn write_camelized<W: Write>(s: &str, mut w: W) -> fmt::Result {
    // str.replace(/-(\w)/g, (_, c) => c.toUpperCase())
    let mut is_minus = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() && is_minus {
            write!(w, "{}", c.to_ascii_uppercase())?;
            is_minus = false;
            continue;
        }
        // write pending -
        if is_minus {
            write!(w, "-")?;
        }
        is_minus = c == '-';
        if !is_minus {
            write!(w, "{}", c)?;
        }
    }
    if is_minus {
        write!(w, "-")
    } else {
        Ok(())
    }
}
fn write_capitalized<W: Write>(s: &str, mut w: W) -> fmt::Result {
    if s.is_empty() {
        return Ok(());
    }
    let c = s.chars().next().unwrap();
    write!(w, "{}", c.to_uppercase())?;
    let s = &s[c.len_utf8()..];
    w.write_str(s)
}

fn write_hyphenated<W: Write>(s: &str, mut w: W) -> fmt::Result {
    // https://javascript.info/regexp-boundary
    // str.replace(/\B([A-Z])/g, '-$1').toLowerCase()
    let mut is_boundary = true;
    for c in s.chars() {
        if !is_boundary && c.is_ascii_uppercase() {
            write!(w, "-{}", c.to_ascii_lowercase())?;
            is_boundary = false;
        } else {
            write!(w, "{}", c)?;
            is_boundary = !c.is_ascii_alphanumeric() && c != '_';
        }
    }
    Ok(())
}

/// compress consecutive whitespaces into one.
fn write_compressed<W: Write>(mut s: &str, mut w: W) -> fmt::Result {
    while let Some(p) = s.find(|c: char| c.is_ascii_whitespace()) {
        let (prev, after) = s.split_at(p);
        w.write_str(prev)?;
        w.write_str(" ")?;
        if let Some(p) = after.find(non_whitespace) {
            s = after.split_at(p).1;
        } else {
            s = "";
        }
    }
    w.write_str(s)
}

/// decode html entity before writing.
fn write_decoded<W: Write>(s: &str, mut w: W) -> fmt::Result {
    if !s.contains('&') {
        return w.write_str(s);
    }
    decode_entities(s, w, false)
}
fn write_attr_decoded<W: Write>(s: &str, mut w: W) -> fmt::Result {
    if !s.contains('&') {
        return w.write_str(s);
    }
    decode_entities(s, w, true)
}

fn write_valid_asset<W: Write>(mut s: &str, mut w: W, asset: &str) -> fmt::Result {
    write!(w, "_{}_", asset)?;
    while let Some(n) = s.find(not_js_identifier) {
        let (prev, next) = s.split_at(n);
        write!(w, "{}", prev)?;
        let c = next.chars().next().unwrap();
        if c == '-' {
            write!(w, "_")?;
        } else {
            write!(w, "{}", c as u32)?;
        }
        s = &next[c.len_utf8()..];
    }
    write!(w, "{}", s)?;
    Ok(())
}

impl StrOps {
    // ideally it should be str.satisfy(op) but adding a trait
    // to str is too much. Use passive voice.
    fn is_satisfied_by(&self, _s: &str) -> bool {
        todo!()
    }
    fn write_ops<W: Write>(&self, s: &str, mut w: W) -> fmt::Result {
        let flag_count = self.bits().count_ones();
        if flag_count == 0 {
            return w.write_str(s);
        }
        if flag_count == 1 {
            return Self::write_one_op(*self, s, w);
        }
        let mut src = s;
        let mut temp = String::new();
        let mut dest = String::new();
        for op in self.iter() {
            Self::write_one_op(op, src, &mut dest)?;
            std::mem::swap(&mut temp, &mut dest);
            dest.clear();
            src = &temp;
        }
        w.write_str(src)
    }
    fn write_one_op<W: Write>(op: Self, s: &str, mut w: W) -> fmt::Result {
        debug_assert!(op.bits().count_ones() == 1);
        match op {
            StrOps::COMPRESS_WHITESPACE => write_compressed(s, w),
            StrOps::DECODE_ENTITY => write_decoded(s, w),
            StrOps::DECODE_ATTR => write_attr_decoded(s, w),
            StrOps::JS_STRING => write_json_string(s, w),
            StrOps::CAMEL_CASE => write_camelized(s, w),
            StrOps::CAPITALIZED => write_capitalized(s, w),
            StrOps::VALID_DIR => write_valid_asset(s, w, "directive"),
            StrOps::VALID_COMP => write_valid_asset(s, w, "component"),
            StrOps::SELF_SUFFIX => {
                // noop, just a marker
                w.write_str(s)
            }
            StrOps::V_DIR_PREFIX => {
                w.write_str("v-")?;
                w.write_str(s)
            }
            StrOps::HANDLER_KEY => {
                w.write_str("on")?;
                let len = s.chars().next().unwrap().len_utf8();
                write_capitalized(&s[0..len], &mut w)?;
                w.write_str(&s[len..])
            }
            StrOps::MODEL_HANDLER => {
                w.write_str("onUpdate:")?;
                w.write_str(s)
            }
            StrOps::CTX_PREFIX => {
                w.write_str("_ctx.")?;
                w.write_str(s)
            }
            StrOps::MOD_SUFFIX => {
                w.write_str(s)?;
                w.write_str("Modifiers")
            }
            StrOps::ASSIGN_EVT => {
                w.write_str("(")?;
                w.write_str(s)?;
                w.write_str(") = $event")
            }
            _ => panic!("strop {:?} is not expected", op),
        }
    }
}

/// A str for Vue compiler's internal modification.
/// Instead of returning a Cow<str>, StrOp is recorded in the VStr
/// and will be processed later in codegen phase.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct VStr<'a> {
    pub raw: &'a str,
    pub ops: StrOps,
}

impl<'a> VStr<'a> {
    // adjective and is_xx for static method
    pub fn raw(raw: &'a str) -> Self {
        Self {
            raw,
            ops: StrOps::empty(),
        }
    }
    pub fn is_handler(s: &VStr) -> bool {
        if s.ops
            .intersects(StrOps::HANDLER_KEY | StrOps::MODEL_HANDLER)
        {
            return true;
        }
        is_event_prop(s.raw)
    }
    pub fn is_self_suffixed(s: &VStr) -> bool {
        s.ops.contains(StrOps::SELF_SUFFIX)
    }
    pub fn is_asset(s: &VStr) -> bool {
        s.ops.intersects(StrOps::ASSET_OPS)
    }
    pub fn is_ctx_prefixed(s: &VStr) -> bool {
        s.ops.contains(StrOps::CTX_PREFIX)
    }
    pub fn is_event_assign(s: &VStr) -> bool {
        s.ops.contains(StrOps::ASSIGN_EVT)
    }
    pub fn has_affix(s: &VStr) -> bool {
        s.ops.intersects(
            StrOps::MODEL_HANDLER
                | StrOps::VALID_DIR
                | StrOps::VALID_COMP
                | StrOps::V_DIR_PREFIX
                | StrOps::CTX_PREFIX
                | StrOps::MOD_SUFFIX
                | StrOps::ASSIGN_EVT,
        )
    }
}
impl<'a> VStr<'a> {
    // verb is instance method
    pub fn decode(&mut self, is_attr: bool) -> &mut Self {
        let ops = if is_attr {
            StrOps::DECODE_ATTR
        } else {
            StrOps::DECODE_ENTITY
        };
        self.ops |= ops;
        self
    }
    pub fn camelize(&mut self) -> &mut Self {
        self.ops |= StrOps::CAMEL_CASE;
        self
    }
    pub fn capitalize(&mut self) -> &mut Self {
        self.ops |= StrOps::CAPITALIZED;
        self
    }
    pub fn pascalize(&mut self) -> &mut Self {
        self.camelize().capitalize()
    }
    pub fn compress_whitespace(&mut self) -> &mut Self {
        self.ops |= StrOps::COMPRESS_WHITESPACE;
        self
    }
    /// convert v-on arg to handler key: click -> onClick
    pub fn be_handler(&mut self) -> &mut Self {
        self.ops |= StrOps::HANDLER_KEY;
        self
    }
    /// convert v-model:arg to onModelUpdate:arg
    pub fn be_vmodel(&mut self) -> &mut Self {
        self.ops |= StrOps::MODEL_HANDLER;
        self
    }
    /// add __self suffix for self referring component
    pub fn suffix_self(&mut self) -> &mut Self {
        self.ops |= StrOps::SELF_SUFFIX;
        self
    }
    /// convert into a valid asset id
    pub fn be_component(&mut self) -> &mut Self {
        self.ops |= StrOps::VALID_COMP;
        self
    }
    pub fn unbe_component(&mut self) -> &mut Self {
        self.ops.remove(StrOps::VALID_COMP);
        self
    }
    pub fn be_directive(&mut self) -> &mut Self {
        self.ops |= StrOps::VALID_DIR;
        self
    }
    pub fn unbe_directive(&mut self) -> &mut Self {
        self.ops.remove(StrOps::VALID_DIR);
        self
    }
    /// convert into a valid asset id
    pub fn prefix_v_dir(&mut self) -> &mut Self {
        self.ops |= StrOps::V_DIR_PREFIX;
        self
    }
    pub fn be_js_str(&mut self) -> &mut Self {
        self.ops |= StrOps::JS_STRING;
        self
    }
    pub fn prefix_ctx(&mut self) -> &mut Self {
        self.ops |= StrOps::CTX_PREFIX;
        self
    }
    pub fn suffix_mod(&mut self) -> &mut Self {
        self.ops |= StrOps::MOD_SUFFIX;
        self
    }
    pub fn assign_event(&mut self) -> &mut Self {
        self.ops |= StrOps::ASSIGN_EVT;
        self
    }
    pub fn into_string(self) -> String {
        let mut ret = String::new();
        self.write_to(&mut ret).expect("string should never fail");
        ret
    }

    pub fn write_to<W: Write>(&self, w: W) -> fmt::Result {
        self.ops.write_ops(self.raw, w)
    }
}

impl<'a> Deref for VStr<'a> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.raw
    }
}

impl<'a> From<&'a str> for VStr<'a> {
    fn from(s: &'a str) -> Self {
        VStr::raw(s)
    }
}

#[cfg(feature = "serde")]
impl<'a> serde::Serialize for VStr<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = self.into_string();
        serializer.serialize_str(&s)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_v_str_size() {
        assert_eq!(std::mem::size_of::<VStr>(), 24);
    }

    // TODO: proptest can test invariant
    #[test]
    fn test_str_ops_iter() {
        let a = StrOps::all();
        let v: Vec<_> = a.iter().collect();
        assert_eq!(v.len() as u32, a.bits().count_ones());
        assert!(v.iter().all(|op| op.bits().count_ones() == 1));
        let a = StrOps::empty();
        let v = a.iter().count();
        assert_eq!(v, 0);
        let a = StrOps::V_DIR_PREFIX | StrOps::VALID_COMP;
        let v: Vec<_> = a.iter().collect();
        assert_eq!(v[0], StrOps::VALID_COMP);
        assert_eq!(v[1], StrOps::V_DIR_PREFIX);
        assert_eq!(v.len(), 2);
    }

    fn write_string(ops: StrOps, s: &str) -> String {
        let mut w = String::new();
        ops.write_ops(s, &mut w).unwrap();
        w
    }

    #[test]
    fn test_str_ops_write() {
        let src = "test";
        let cases = [
            (StrOps::empty(), "test"),
            (StrOps::V_DIR_PREFIX, "v-test"),
            (StrOps::V_DIR_PREFIX, "v-test"),
            (StrOps::SELF_SUFFIX, "test"),
            (StrOps::JS_STRING, stringify!("test")),
            (StrOps::CAMEL_CASE | StrOps::V_DIR_PREFIX, "vTest"),
        ];
        for (ops, expect) in cases {
            let origin = ops;
            assert_eq!(write_string(ops, src), expect);
            assert_eq!(ops, origin);
        }
    }

    #[test]
    fn test_str_ops_write_edge() {
        let cases = [
            ("å—åŒ–ã‘", StrOps::empty(), "å—åŒ–ã‘"),
            ("å—åŒ–ã‘", StrOps::JS_STRING, stringify!("å—åŒ–ã‘")),
            ("foo-bar", StrOps::CAMEL_CASE, "fooBar"),
            ("foo-bar", StrOps::CAPITALIZED, "Foo-bar"),
            ("", StrOps::CAPITALIZED, ""),
            ("ālaya-vijñāna", StrOps::CAMEL_CASE, "ālayaVijñāna"),
            ("आलयविज्ञान", StrOps::CAMEL_CASE, "आलयविज्ञान"),
            ("ω", StrOps::CAPITALIZED, "Ω"),
            (
                "foo-bar",
                StrOps::CAPITALIZED | StrOps::CAMEL_CASE,
                "FooBar",
            ),
            ("-a-b-c", StrOps::CAMEL_CASE, "ABC"),
            ("a-a-b-c", StrOps::CAMEL_CASE, "aABC"),
            ("a--b", StrOps::CAMEL_CASE, "a-B"),
            ("a--b", StrOps::VALID_COMP, "_component_a__b"),
            ("aいろは", StrOps::VALID_COMP, "_component_aいろは"),
            ("a^_^", StrOps::VALID_COMP, "_component_a94_94"),
            ("a--b", StrOps::VALID_DIR, "_directive_a__b"),
            ("a--", StrOps::VALID_DIR, "_directive_a__"),
        ];
        for (src, ops, expect) in cases {
            let origin = ops;
            assert_eq!(write_string(ops, src), expect);
            assert_eq!(ops, origin);
        }
    }
}
