use compiler::compiler::CompileOption;
use compiler::util::prop_finder;
use compiler::{
    SourceLocation, BindingMetadata,
    scanner::{Scanner, TextMode},
    parser::{Parser, AstNode, AstRoot, Element, ElemProp},
    error::{VecErrorHandler, CompilationError, RcErrHandle, ErrorKind},
};
use smallvec::{smallvec, SmallVec};
use std::path::PathBuf;
use std::rc::Rc;
use rustc_hash::FxHashMap;

pub enum PadOption {
    Line,
    Space,
    NoPad,
}

pub struct SfcParseOptions {
    pub filename: String,
    pub source_map: bool,
    pub source_root: PathBuf,
    pub pad: PadOption,
    pub ignore_empty: bool,
}

impl Default for SfcParseOptions {
    fn default() -> Self {
        Self {
            filename: "anonymous.vue".into(),
            source_map: true,
            source_root: "".into(),
            pad: PadOption::NoPad,
            ignore_empty: true,
        }
    }
}

#[derive(Clone)]
pub struct SfcBlock<'a> {
    pub source: &'a str,
    pub attrs: FxHashMap<&'a str, Option<&'a str>>,
    pub loc: SourceLocation,
    pub compiled_content: String,
    // pub map: Option<RawSourceMap>,
}
impl<'a> SfcBlock<'a> {
    fn new(element: Element<'a>, src: &'a str) -> Self {
        let source = Self::compute_content(&element, src);
        let attrs = element
            .properties
            .into_iter()
            .filter_map(|p| match p {
                ElemProp::Attr(attr) => {
                    let val = attr.value.map(|v| v.content.raw);
                    Some((attr.name, val))
                }
                _ => None,
            })
            .collect::<FxHashMap<_, _>>();
        Self {
            source,
            attrs,
            compiled_content: source.into(),
            loc: element.location,
        }
    }
    pub fn get_attr(&self, name: &'a str) -> Option<&'a str> {
        self.attrs.get(name).copied().flatten()
    }

    fn compute_content(element: &Element<'a>, src: &'a str) -> &'a str {
        if element.children.is_empty() {
            ""
        } else {
            let start = element
                .children
                .first()
                .unwrap()
                .get_location()
                .start
                .offset;
            let end = element.children.last().unwrap().get_location().end.offset;
            &src[start..end]
        }
    }
}

pub enum SfcError {
    DeprecatedFunctionalTemplate,
    DeprecatedStyleVars,
    SrcOnScriptSetup,
    ScrtipSrcWithScriptSetup,
    DuplicateBlock,
}

impl ErrorKind for SfcError {
    fn msg(&self) -> &'static str {
        use SfcError::*;
        match self {
            DeprecatedFunctionalTemplate => "<template functional> is no longer supported.",
            DeprecatedStyleVars => "<style vars> has been replaced by a new proposal.",
            SrcOnScriptSetup => "<script setup> cannot use the 'src' attribute because its syntax will be ambiguous outside of the component.",
            ScrtipSrcWithScriptSetup => "<script> cannot use the 'src' attribute when <script setup> is also present because they must be processed together.",
            DuplicateBlock => "Single file component can contain only one element: ",
        }
    }
}

// TODO
pub type Ast = String;

pub struct SfcTemplateBlock<'a> {
    // pub ast: Ast,
    pub block: SfcBlock<'a>,
}

#[derive(Clone)]
pub struct SfcScriptBlock<'a> {
    // pub ast: Option<Ast>,
    // pub setup_ast: Option<Ast>,
    pub setup: Option<&'a str>,
    pub bindings: Option<BindingMetadata<'a>>,
    pub block: SfcBlock<'a>,
}

impl<'a> SfcScriptBlock<'a> {
    pub fn is_setup(&self) -> bool {
        self.block.get_attr("setup").is_some()
    }
    pub fn get_lang(&self) -> &str {
        self.block.get_attr("lang").unwrap_or("jsx")
    }
}

pub struct SfcStyleBlock<'a> {
    // pub scoped: bool,
    // pub module: Option<&'a str>,
    pub block: SfcBlock<'a>,
}
pub struct SfcCustomBlock<'a> {
    pub custom_type: &'a str,
    pub block: SfcBlock<'a>,
}

pub struct SfcDescriptor<'a> {
    pub filename: String,
    pub template: Option<SfcTemplateBlock<'a>>,
    pub scripts: SmallVec<[SfcScriptBlock<'a>; 1]>,
    pub styles: SmallVec<[SfcStyleBlock<'a>; 1]>,
    pub custom_blocks: Vec<SfcCustomBlock<'a>>,
    pub css_vars: Vec<&'a str>,
    /// whether the SFC uses :slotted() modifier.
    /// this is used as a compiler optimization hint.
    pub slotted: bool,
}

impl<'a> SfcDescriptor<'a> {
    fn new(filename: String) -> Self {
        Self {
            filename,
            template: None,
            scripts: smallvec![],
            styles: smallvec![],
            custom_blocks: vec![],
            css_vars: vec![],
            slotted: false,
        }
    }
}

pub struct SfcParseResult<'a> {
    pub descriptor: SfcDescriptor<'a>,
    pub errors: Vec<CompilationError>,
}

pub fn parse_sfc(source: &str, option: SfcParseOptions) -> SfcParseResult<'_> {
    let err_handle = Rc::new(VecErrorHandler::default());
    let ast = parse_ast(source, err_handle.clone());
    let mut descriptor = SfcDescriptor::new(option.filename);
    let mut errors = get_errors(err_handle);
    for node in ast.children {
        let elem = match node {
            AstNode::Element(elem) => elem,
            _ => continue,
        };
        let ignore_empty = option.ignore_empty;
        if ignore_empty && elem.tag_name != "template" && is_empty(&elem) && !has_src(&elem) {
            continue;
        }
        let maybe_errror = assemble_descriptor(elem, source, &mut descriptor);
        if let Some(error) = maybe_errror {
            errors.push(error);
        }
    }
    SfcParseResult { descriptor, errors }
}

fn parse_ast(source: &str, err_handle: RcErrHandle) -> AstRoot {
    let compile_opt = CompileOption {
        is_pre_tag: |_| true,
        is_native_tag: |_| true,
        get_text_mode: |tag| {
            if tag == "template" {
                TextMode::Data
            } else {
                TextMode::RawText
            }
        },
        error_handler: err_handle.clone(),
        ..Default::default()
    };
    let scanner = Scanner::new(compile_opt.scanning());
    let parser = Parser::new(compile_opt.parsing());
    let tokens = scanner.scan(source, err_handle.clone());
    parser.parse(tokens, err_handle.clone())
}

fn get_errors(err_handle: Rc<VecErrorHandler>) -> Vec<CompilationError> {
    err_handle.error_mut().drain(..).collect()
}

fn assemble_descriptor<'a>(
    element: Element<'a>,
    src: &'a str,
    descriptor: &mut SfcDescriptor<'a>,
) -> Option<CompilationError> {
    let tag_name = element.tag_name;
    if tag_name == "template" {
        let has_functional = prop_finder(&element, "functional")
            .attr_only()
            .find()
            .map(|func| func.get_ref().get_location().clone());
        if descriptor.template.is_some() {
            let error = CompilationError::extended(SfcError::DuplicateBlock)
                .with_additional_message("<template>")
                .with_location(element.location);
            return Some(error);
        }
        let block = SfcTemplateBlock {
            block: SfcBlock::new(element, src),
        };
        descriptor.template = Some(block);
        has_functional.map(|loc| {
            CompilationError::extended(SfcError::DeprecatedFunctionalTemplate).with_location(loc)
        })
    } else if tag_name == "script" {
        let location = element.location.clone();
        let block = SfcBlock::new(element, src);
        let block = SfcScriptBlock {
            bindings: None, // TODO
            setup: block.get_attr("setup"),
            block,
        };
        let scripts = &descriptor.scripts;
        let is_setup = block.is_setup();
        if scripts.len() >= 2 || !scripts.is_empty() && scripts[0].is_setup() == is_setup {
            let ty = if is_setup { "<script setup>" } else { "script" };
            let error = CompilationError::extended(SfcError::DuplicateBlock)
                .with_additional_message(ty)
                .with_location(location);
            return Some(error);
        }
        descriptor.scripts.push(block);
        None
    } else if tag_name == "style" {
        let has_vars = prop_finder(&element, "vars")
            .attr_only()
            .find()
            .map(|vars| vars.get_ref().get_location().clone());
        let block = SfcStyleBlock {
            block: SfcBlock::new(element, src),
        };
        descriptor.styles.push(block);
        has_vars
            .map(|loc| CompilationError::extended(SfcError::DeprecatedStyleVars).with_location(loc))
    } else {
        let ty = element.tag_name;
        let block = SfcBlock::new(element, src);
        let block = SfcCustomBlock {
            custom_type: ty,
            block,
        };
        descriptor.custom_blocks.push(block);
        None
    }
}

fn is_empty(elem: &Element) -> bool {
    !elem.children.iter().any(|n| match n {
        AstNode::Text(t) => !t.is_all_whitespace(),
        _ => true,
    })
}

fn has_src(elem: &Element) -> bool {
    prop_finder(elem, "src").attr_only().find().is_some()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_sfc() {
        let src = "<template>abc</template><script>export default {}</script>";
        let parsed = parse_sfc(src, Default::default());
        let descriptor = parsed.descriptor;
        assert!(descriptor.template.is_some());
        assert_eq!(descriptor.scripts.len(), 1);
        let script = &descriptor.scripts[0];
        assert_eq!(script.block.source, "export default {}");
    }
}
