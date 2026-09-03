//! Kaynak konum ve aralık (span) temel tipleri + SourceMap.

use std::ops::Range;
use std::path::{Path, PathBuf};

/// Bir kaynak dosyayı temsil eden opak kimlik.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub u32);

/// Kaynak metinde yarı açık bayt aralığı `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: u32,
    pub end: u32,
    pub file: FileId,
}

impl Span {
    pub fn new(file: FileId, start: u32, end: u32) -> Self {
        Self { start, end, file }
    }

    pub fn len(&self) -> u32 {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Derlemedeki tek bir kaynak dosya.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: FileId,
    pub path: PathBuf,
    pub text: String,
    /// Satır başlarının bayt offsetleri (ilk eleman her zaman 0).
    line_starts: Vec<u32>,
}

/// Derlemedeki tüm kaynak dosyaların kaydı; span → satır/sütun çevirisi yapar.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dosyayı kaydeder ve kimliğini döndürür.
    pub fn add_file(&mut self, path: impl Into<PathBuf>, text: impl Into<String>) -> FileId {
        let id = FileId(self.files.len() as u32);
        let text = text.into();
        let line_starts = compute_line_starts(&text);
        self.files.push(SourceFile {
            id,
            path: path.into(),
            text,
            line_starts,
        });
        id
    }

    pub fn file(&self, id: FileId) -> &SourceFile {
        &self.files[id.0 as usize]
    }

    pub fn source(&self, id: FileId) -> &str {
        &self.file(id).text
    }

    pub fn path(&self, id: FileId) -> &Path {
        &self.file(id).path
    }

    /// Span başlangıcının 1-tabanlı (satır, sütun) konumu.
    pub fn line_col(&self, span: Span) -> (u32, u32) {
        self.line_col_at(span.file, span.start)
    }

    /// Verilen bayt offsetinin 1-tabanlı (satır, sütun) konumu.
    /// Sütun, satır içindeki bayt offsetidir (UTF-8 çok baytlı
    /// karakterlerde görsel sütundan sapabilir).
    pub fn line_col_at(&self, id: FileId, byte: u32) -> (u32, u32) {
        let line0 = self.line_index(id, byte);
        let line_start = self.file(id).line_starts[line0];
        (line0 as u32 + 1, byte - line_start + 1)
    }

    /// Span başlangıcının 1-tabanlı (satır, sütun) konumu — sütun
    /// KARAKTER sayısıdır (UTF-8 çok baytlı karakterler 1 sayılır).
    /// İnsan-okunabilir çıktı bunu kullanmalı; JSON `byte` alanı için
    /// bayt tabanlı `line_col` geçerlidir.
    pub fn line_col_utf8(&self, span: Span) -> (u32, u32) {
        let line0 = self.line_index(span.file, span.start);
        let line_start = self.file(span.file).line_starts[line0] as usize;
        let text = &self.file(span.file).text[line_start..span.start as usize];
        (line0 as u32 + 1, text.chars().count() as u32 + 1)
    }

    /// Baytın bulunduğu 0-tabanlı satır indeksi.
    pub fn line_index(&self, id: FileId, byte: u32) -> usize {
        let starts = &self.file(id).line_starts;
        starts.partition_point(|&s| s <= byte).saturating_sub(1)
    }

    /// 0-tabanlı satırın bayt aralığı (satır sonu dahil).
    pub fn line_range(&self, id: FileId, line0: usize) -> Range<usize> {
        let file = self.file(id);
        let start = file.line_starts[line0] as usize;
        let end = file
            .line_starts
            .get(line0 + 1)
            .map(|&s| s as usize)
            .unwrap_or(file.text.len());
        start..end
    }

    /// 1-tabanlı satırın metni (satır sonu karakterleri olmadan).
    pub fn line_text(&self, id: FileId, line1: u32) -> &str {
        let range = self.line_range(id, (line1 - 1) as usize);
        self.file(id).text[range].trim_end_matches(['\n', '\r'])
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

fn compute_line_starts(text: &str) -> Vec<u32> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| (i + 1) as u32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_with(text: &str) -> (SourceMap, FileId) {
        let mut map = SourceMap::new();
        let id = map.add_file("test.volt", text);
        (map, id)
    }

    #[test]
    fn line_col_first_line() {
        let (map, id) = map_with("let x = 1\nlet y = 2\n");
        assert_eq!(map.line_col(Span::new(id, 4, 5)), (1, 5));
    }

    #[test]
    fn line_col_second_line() {
        let (map, id) = map_with("let x = 1\nlet y = 2\n");
        assert_eq!(map.line_col(Span::new(id, 14, 15)), (2, 5));
    }

    #[test]
    fn line_text_strips_newline() {
        let (map, id) = map_with("birinci\nikinci\r\n");
        assert_eq!(map.line_text(id, 1), "birinci");
        assert_eq!(map.line_text(id, 2), "ikinci");
    }

    #[test]
    fn source_returns_full_text() {
        let (map, id) = map_with("module M {}");
        assert_eq!(map.source(id), "module M {}");
    }

    #[test]
    fn multiple_files_distinct_ids() {
        let mut map = SourceMap::new();
        let a = map.add_file("a.volt", "aaa");
        let b = map.add_file("b.volt", "bbb");
        assert_ne!(a, b);
        assert_eq!(map.source(a), "aaa");
        assert_eq!(map.source(b), "bbb");
        assert_eq!(map.path(b).to_str(), Some("b.volt"));
    }

    #[test]
    fn line_col_utf8_counts_chars_not_bytes() {
        // "module Sayaç { in veri : u8 }" — 'ç' 2 bayt kaplar
        let src = "module Sayaç { in veri : u8 }";
        let (map, id) = map_with(src);
        // "Sayaç" 7. bayt / 8. karakter DEĞİL — 7. bayt = 8. sütun (1-tabanlı)
        let sayac = Span::new(id, 7, 13); // "Sayaç" (ç = 2 bayt: 11-12)
        assert_eq!(map.line_col_utf8(sayac), (1, 8));
        assert_eq!(map.line_col(sayac), (1, 8)); // 'ç'ten önce fark yok
                                                 // "veri" 'ç'ten SONRA: bayt 18, karakter sütunu 18 (1 bayt geride)
        let veri_start = src.find("veri").unwrap() as u32;
        let veri = Span::new(id, veri_start, veri_start + 4);
        assert_eq!(map.line_col(veri), (1, veri_start + 1)); // bayt sütunu: 19
        assert_eq!(map.line_col_utf8(veri), (1, veri_start)); // karakter: 18
                                                              // "u8" için de aynı kayma
        let u8_start = src.find("u8").unwrap() as u32;
        let u8_span = Span::new(id, u8_start, u8_start + 2);
        assert_eq!(map.line_col_utf8(u8_span).1, map.line_col(u8_span).1 - 1);
    }

    #[test]
    fn line_col_utf8_multiline_turkish() {
        let (map, id) = map_with("// yukarı sayaç\nmodule Sayaç {}\n");
        let module_start = map.source(id).find("module").unwrap() as u32;
        assert_eq!(
            map.line_col_utf8(Span::new(id, module_start, module_start + 6)),
            (2, 1)
        );
    }

    #[test]
    fn line_col_at_line_start_is_col_one() {
        let (map, id) = map_with("a\nb\n");
        assert_eq!(map.line_col_at(id, 2), (2, 1));
    }
}
