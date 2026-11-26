//! Sürücünün yolu, `emit_full` imzasıyla: AST tip denetiminden geçer,
//! üretici tip denetiminin tablosunu okur (`TypeckResult::checked_types`,
//! ADR-0141). `prev()`'in yardımcı register'ının ve değerinin genişliği ve
//! işareti tip denetiminden gelir (#413); tablosuz `emit_full` kontratında
//! `prev()` olan modüle E2005 verir.

use volt_ast::SourceFile;
use volt_span::FileId;
use volt_sv_emit::{emit_unit, ConstArrayStyle, EmitOutput, SourceText, SvaMode};

pub fn emit_full(ast: &SourceFile, source_name: &str, source: &str, mode: SvaMode) -> EmitOutput {
    let analysis = volt_hir::analyze(ast);
    let checked = analysis.typeck.checked_types(ast);
    let sources = [SourceText {
        file: FileId(0),
        name: source_name,
        text: source,
    }];
    emit_unit(
        ast,
        source_name,
        &sources,
        mode,
        ConstArrayStyle::default(),
        &checked,
    )
}
