use compiler::{
    Namespace, codegen::ScriptMode, compiler::CompileOption, converter::RcErrHandle,
    flags::RuntimeHelper, parser::Element, scanner::TextMode,
};
use crate::{converter::DOM_DIR_CONVERTERS, extension::dom_helper};
use phf::{phf_set, Set};

const NATIVE_TAGS: Set<&str> = phf_set! {
    // HTML_TAGS
    "html","body","base","head","link","meta","style","title","address","article","aside",
    "footer","header","h1","h2","h3","h4","h5","h6","nav","section","div","dd","dl","dt",
    "figcaption", "figure","picture","hr","img","li","main","ol","p","pre","ul","a","b",
    "abbr","bdi","bdo","br","cite","code","data","dfn","em","i","kbd","mark","q","rp","rt",
    "ruby","s","samp","small","span","strong","sub","sup","time","u","var","wbr","area",
    "audio","map","track","video","embed","object","param","source","canvas","script",
    "noscript","del","ins","caption","col","colgroup","table","thead","tbody","td","th",
    "tr","button","datalist","fieldset","form","input","label","legend","meter","optgroup",
    "option","output","progress","select","textarea","details","dialog","menu","summary",
    "template","blockquote","iframe","tfoot",
    // SVG_TAGS
    "svg","animate","animateMotion","animateTransform","circle","clipPath","color-profile",
    "defs","desc","discard","ellipse","feBlend","feColorMatrix","feComponentTransfer",
    "feComposite","feConvolveMatrix","feDiffuseLighting","feDisplacementMap",
    "feDistanceLight","feDropShadow","feFlood","feFuncA","feFuncB","feFuncG","feFuncR",
    "feGaussianBlur","feImage","feMerge","feMergeNode","feMorphology","feOffset",
    "fePointLight","feSpecularLighting","feSpotLight","feTile","feTurbulence","filter",
    "foreignObject","g","hatch","hatchpath","image","line","linearGradient","marker","mask",
    "mesh","meshgradient","meshpatch","meshrow","metadata","mpath","path","pattern",
    "polygon","polyline","radialGradient","rect","set","solidcolor","stop","switch","symbol",
    "text","textPath","tspan","unknown","use","view", // "title"
    // MATH ML
    "annotation-xml", "annotation", "maction", "maligngroup", "malignmark", "math", "menclose",
    "merror", "mfenced", "mfrac", "mi", "mlongdiv", "mmultiscripts", "mo", "mover", "mpadded",
    "mphantom", "mprescripts", "mroot", "mrow", "ms", "mscarries", "mscarry", "msgroup", "msline",
    "mspace", "msqrt", "msrow", "mstack", "mstyle", "msub", "msubsup", "msup", "mtable", "mtd",
    "mtext", "mtr", "munder", "munderover", "none", "semantics",
};

pub fn is_native_tag(tag: &str) -> bool {
    NATIVE_TAGS.contains(tag)
}
fn is_pre_tag(tag: &str) -> bool {
    tag.eq_ignore_ascii_case("pre")
}

const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];
fn is_void_tag(tag: &str) -> bool {
    VOID_TAGS.contains(&tag)
}

fn get_builtin_component(tag: &str) -> Option<RuntimeHelper> {
    match tag {
        "transition" | "Transition" => Some(dom_helper::TRANSITION),
        "TransitionGroup" | "transition-group" => Some(dom_helper::TRANSITION_GROUP),
        _ => None,
    }
}

fn get_text_mode(tag: &str) -> TextMode {
    match tag {
        "style" | "script" | "iframe" | "noscript" => TextMode::RawText,
        "textarea" | "title" => TextMode::RcData,
        _ => TextMode::Data,
    }
}

// https://html.spec.whatwg.org/multipage/parsing.html#tree-construction-dispatcher
fn get_namespace(tag: &str, parent: Option<&Element>) -> Namespace {
    if let Some(p) = parent {
        if p.namespace == Namespace::MathMl {
            if p.tag_name == "annotaion-xml" {
                if tag == "svg" {
                    return Namespace::Svg;
                } else {
                    return Namespace::Html;
                }
            } else if ["mi", "mo", "mn", "ms", "mtext"].contains(&p.tag_name) {
                if tag == "mglyph" || tag == "malignmark" {
                    return Namespace::MathMl;
                } else {
                    return Namespace::Html;
                }
            } else {
                return Namespace::MathMl;
            }
        }
        if p.namespace == Namespace::Svg {
            if ["foreignObject", "desc", "title"].contains(&p.tag_name) {
                return Namespace::Html;
            } else {
                return Namespace::Svg;
            }
        }
    }
    if tag == "svg" {
        Namespace::Svg
    } else if tag == "math" {
        Namespace::MathMl
    } else {
        Namespace::Html
    }
}

pub fn compile_option(error_handler: RcErrHandle) -> CompileOption {
    CompileOption {
        is_native_tag,
        get_text_mode,
        is_pre_tag,
        is_void_tag,
        get_builtin_component,
        get_namespace,
        delimiters: ("{{".to_string(), "}}".to_string()),
        directive_converters: DOM_DIR_CONVERTERS.iter().copied().collect(),
        helper_strs: dom_helper::DOM_HELPER_MAP,
        error_handler,
        mode: ScriptMode::Function {
            prefix_identifier: true,
            runtime_global_name: "Vue".into(),
        },
        ..Default::default()
    }
}
