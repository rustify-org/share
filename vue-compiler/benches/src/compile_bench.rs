mod bench_util;

use compiler::compiler::BaseCompiler;
use compiler::compiler::{CompileOption, TemplateCompiler, get_base_passes};

use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::{criterion_group, criterion_main};

fn base_compile(source: &str) {
    let option = CompileOption {
        is_native_tag: |t| t != "draggable-header-view" && t != "tree-item",
        is_dev: false,
        ..Default::default()
    };
    let sfc_info = Default::default();
    let dest = || vec![];
    let compiler = BaseCompiler::new(dest, get_base_passes, option);
    compiler.compile(source, &sfc_info).unwrap();
}

fn test_enum_eq(c: &mut Criterion) {
    for (name, content) in bench_util::get_fixtures() {
        c.bench_with_input(BenchmarkId::new("compile", name), &content, |b, c| {
            b.iter(|| base_compile(c));
        });
    }
}

criterion_group!(benches, test_enum_eq);
criterion_main!(benches);
