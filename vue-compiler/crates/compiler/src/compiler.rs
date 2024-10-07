use super::{
    SFCInfo,
    codegen::{CodeGenerateOption, CodeGenerator, CodeGen, ScriptMode, CodeGenInfo},
    converter::{
        no_op_directive_convert, BaseConvertInfo as BaseInfo, BaseConverter, BaseRoot,
        ConvertOption, Converter, DirConvertFn, V_BIND, V_MODEL,
    },
    error::{NoopErrorHandler, RcErrHandle},
    flags::RuntimeHelper,
    parser::{Element, ParseOption, Parser, WhitespaceStrategy, AstRoot},
    scanner::{ScanOption, Scanner, TextMode, Tokens},
    transformer::{BaseTransformer, CorePass, TransformOption, Transformer},
    util::{no, yes},
    Namespace,
    transformer::{
        collect_entities::EntityCollector,
        mark_patch_flag::PatchFlagMarker,
        mark_slot_flag::SlotFlagMarker,
        optimize_text::TextOptimizer,
        pass::{Scope, SharedInfoPasses},
        process_expression::ExpressionProcessor,
        hoist_static::HoistStatic,
    },
};

use rustc_hash::FxHashMap;
use std::{io, rc::Rc, marker::PhantomData};

pub struct CompileOption {
    /// e.g. platform native elements, e.g. `<div>` for browsers
    pub is_native_tag: fn(&str) -> bool,

    /// e.g. native elements that can self-close, e.g. `<img>`, `<br>`, `<hr>`
    pub is_void_tag: fn(&str) -> bool,

    /// e.g. elements that should preserve whitespace inside, e.g. `<pre>`
    pub is_pre_tag: fn(&str) -> bool,

    /// Platform-specific built-in components e.g. `<Transition>`
    /// The pairing runtime provides additional built-in elements,
    /// Platform developer can use this to mark them as built-in
    /// so the compiler will generate component vnodes for them.
    pub get_builtin_component: fn(&str) -> Option<RuntimeHelper>,

    /// Separate option for end users to extend the native elements list
    pub is_custom_element: fn(&str) -> bool,

    /// Get tag namespace
    pub get_namespace: fn(&str, Option<&Element<'_>>) -> Namespace,

    /// Get text parsing mode for this element
    pub get_text_mode: fn(&str) -> TextMode,

    /// @default ['{{', '}}']
    pub delimiters: (String, String),

    /// Whitespace handling strategy
    pub whitespace: WhitespaceStrategy,

    /// platform speicific helper
    pub helper_strs: &'static [&'static str],

    /// Whether to keep comments in the templates AST.
    /// This defaults to `true` in development and `false` in production builds.
    pub preserve_comments: Option<bool>,
    /// Whether the output is dev build which includes v-if comment and dev patch flags.
    pub is_dev: bool,

    /// An object of { name: transform } to be applied to every directive attribute
    /// node found on element nodes.
    pub directive_converters: FxHashMap<&'static str, DirConvertFn>,
    /// Hoist static VNodes and props objects to `_hoisted_x` constants
    /// @default false
    pub hoist_static: bool,
    /// Cache v-on handlers to avoid creating new inline functions on each render,
    /// also avoids the need for dynamically patching the handlers by wrapping it.
    /// e.g `@click="foo"` by default is compiled to `{ onClick: foo }`. With this
    /// option it's compiled to:
    /// ```js
    /// { onClick: _cache[0] || (_cache[0] = e => _ctx.foo(e)) }
    /// ```
    /// - Requires "prefixIdentifiers" to be enabled because it relies on scope
    /// analysis to determine if a handler is safe to cache.
    /// @default false
    pub cache_handlers: bool,

    /// - `module` mode will generate ES module import statements for helpers
    /// and export the render function as the default export.
    /// - `function` mode will generate a single `const { helpers... } = Vue`
    /// statement and return the render function. It expects `Vue` to be globally
    /// available (or passed by wrapping the code with an IIFE). It is meant to be
    /// used with `new Function(code)()` to generate a render function at runtime.
    /// @default 'function'
    pub mode: ScriptMode,
    /// Generate source map?
    /// @default false
    pub source_map: bool,
    /// Whether the output JS needs re-rendering when Vue runtime data change.
    /// e.g. SSR can set it to false since SSR is executed only once per request.
    /// @default true
    pub need_reactivity: bool,
    /// Custom error reporter. Default is noop.
    pub error_handler: RcErrHandle,
    // deleted options
    // nodeTransforms?: NodeTransform[]
    // transformHoist?: HoistTransform | null
    // expressionPlugins?: ParserPlugin[]
    // prefix_identifiers: bool,
    // optimizeImports?: boolean // farewell, webpack optimization

    // moved to SFCInfo
    // bindingMetadata?: BindingMetadata
    // inline?: boolean
    // filename?: string
    // scopeId?: string | null
    // slotted?: boolean

    // moved to SSR or need_reactivity
    // ssr: bool // will be false in fallback node
    // inSSR?: bool // always true in ssr build
    // ssrCssVars?: string
    // ssrRuntimeModuleName?: string
}

impl Default for CompileOption {
    fn default() -> Self {
        let mut directive_converters = FxHashMap::default();
        directive_converters.insert(V_BIND.0, V_BIND.1);
        directive_converters.insert(V_MODEL.0, V_MODEL.1);
        directive_converters.insert("on", no_op_directive_convert);
        Self {
            is_native_tag: yes,
            is_void_tag: no,
            is_pre_tag: no,
            get_builtin_component: |_| None,
            is_custom_element: no,
            get_namespace: |_, _| Namespace::Html,
            get_text_mode: |_| TextMode::Data,
            delimiters: ("{{".into(), "}}".into()),
            whitespace: WhitespaceStrategy::Preserve,
            helper_strs: &[],
            preserve_comments: None,
            is_dev: true,
            directive_converters,
            hoist_static: false,
            cache_handlers: false,
            mode: ScriptMode::Function {
                prefix_identifier: false,
                runtime_global_name: "Vue".into(),
            },
            source_map: false,
            need_reactivity: true,
            error_handler: Rc::new(NoopErrorHandler),
        }
    }
}

impl CompileOption {
    pub fn scanning(&self) -> ScanOption {
        ScanOption {
            delimiters: self.delimiters.clone(),
            get_text_mode: self.get_text_mode,
        }
    }
    pub fn parsing(&self) -> ParseOption {
        ParseOption {
            whitespace: self.whitespace.clone(),
            preserve_comment: self.preserve_comments.unwrap_or(self.is_dev),
            get_namespace: self.get_namespace,
            get_text_mode: self.get_text_mode,
            is_native_element: self.is_native_tag,
            is_void_tag: self.is_void_tag,
            is_pre_tag: self.is_pre_tag,
            get_builtin_component: self.get_builtin_component,
            is_custom_element: self.is_custom_element,
        }
    }
    pub fn converting(&self) -> ConvertOption {
        ConvertOption {
            get_builtin_component: self.get_builtin_component,
            is_dev: self.is_dev,
            directive_converters: self.directive_converters.clone(),
            need_reactivity: self.need_reactivity,
        }
    }
    pub fn transforming(&self) -> TransformOption {
        let prefix = match self.mode {
            ScriptMode::Function {
                prefix_identifier, ..
            } => prefix_identifier,
            ScriptMode::Module { .. } => true,
        };
        TransformOption {
            prefix_identifier: prefix,
            is_dev: self.is_dev,
        }
    }
    pub fn codegen(&self) -> CodeGenerateOption {
        CodeGenerateOption {
            is_dev: self.is_dev,
            mode: self.mode.clone(),
            source_map: self.source_map,
            helper_strs: self.helper_strs,
        }
    }
}

// TODO: refactor this ownership usage
pub trait TemplateCompiler<'a> {
    type IR;
    type Info: Copy;
    type Output;

    fn scan(&self, source: &'a str) -> Tokens<'a>;
    fn parse(&self, tokens: Tokens<'a>) -> AstRoot<'a>;
    fn convert(&self, ast: AstRoot<'a>, info: Self::Info) -> Self::IR;
    fn transform(&self, ir: &mut Self::IR, info: Self::Info);
    fn generate(&self, ir: Self::IR, info: Self::Info) -> Self::Output;
    fn get_error_handler(&self) -> RcErrHandle;

    fn compile(&self, source: &'a str, info: Self::Info) -> Self::Output {
        let tokens = self.scan(source);
        let ast = self.parse(tokens);
        let mut ir = self.convert(ast, info);
        self.transform(&mut ir, info);
        self.generate(ir, info)
    }
}

pub struct BaseCompiler<'a, P, W>
where
    W: io::Write,
    P: CorePass<BaseInfo<'a>>,
{
    writer: fn() -> W,
    passes: fn(&'a SFCInfo<'a>, &CompileOption) -> P,
    option: CompileOption,
    scanner: Scanner,
    parser: Parser,
    pd: PhantomData<&'a ()>,
}

impl<'a, P, W> BaseCompiler<'a, P, W>
where
    W: io::Write,
    P: CorePass<BaseInfo<'a>>,
{
    pub fn new(
        writer: fn() -> W,
        passes: fn(&'a SFCInfo<'a>, &CompileOption) -> P,
        option: CompileOption,
    ) -> Self {
        Self {
            writer,
            passes,
            scanner: Scanner::new(option.scanning()),
            parser: Parser::new(option.parsing()),
            option,
            pd: PhantomData,
        }
    }
    fn get_converter(&self) -> BaseConverter {
        let eh = self.get_error_handler();
        let option = self.option.converting();
        BaseConverter::new(eh, option)
    }
}

impl<'a, P, W> TemplateCompiler<'a> for BaseCompiler<'a, P, W>
where
    W: io::Write,
    P: CorePass<BaseInfo<'a>>,
{
    type IR = BaseRoot<'a>;
    type Info = &'a SFCInfo<'a>;
    type Output = io::Result<W>;

    fn scan(&self, source: &'a str) -> Tokens<'a> {
        self.scanner.scan(source, self.get_error_handler())
    }

    fn parse(&self, tokens: Tokens<'a>) -> AstRoot<'a> {
        self.parser.parse(tokens, self.get_error_handler())
    }
    fn convert(&self, ast: AstRoot<'a>, info: Self::Info) -> Self::IR {
        self.get_converter().convert_ir(ast, info)
    }
    fn transform(&self, ir: &mut Self::IR, info: Self::Info) {
        let pass = (self.passes)(info, &self.option);
        BaseTransformer::transform(ir, pass)
    }
    fn generate(&self, ir: Self::IR, sfc_info: Self::Info) -> Self::Output {
        let mut writer = (self.writer)();
        let option = self.option.codegen();
        let generator = CodeGen::new(option);
        let gen_info = CodeGenInfo {
            writer: &mut writer,
            sfc_info,
        };
        generator.generate(ir, gen_info)?;
        Ok(writer)
    }
    fn get_error_handler(&self) -> RcErrHandle {
        self.option.error_handler.clone()
    }
}

pub fn get_base_passes<'a>(
    sfc_info: &'a SFCInfo<'a>,
    opt: &CompileOption,
) -> impl CorePass<BaseInfo<'a>> {
    use crate::chain;
    let prefix_identifier = opt.transforming().prefix_identifier;
    let shared = chain![
        SlotFlagMarker,
        ExpressionProcessor {
            prefix_identifier,
            sfc_info,
            err_handle: opt.error_handler.clone(),
        },
    ];
    chain![
        TextOptimizer,
        EntityCollector::default(),
        PatchFlagMarker,
        SharedInfoPasses {
            passes: shared,
            shared_info: Scope::default(),
            pd: PhantomData,
        },
        HoistStatic::default(),
    ]
}
