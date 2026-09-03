//! Senkronizasyon kümeleri ve kurtarma algoritması (error-recovery.md §2-§3).

use volt_diagnostics::Diagnostic;

use crate::token::TokenKind;
use crate::token::TokenKind::*;

use super::Parser;

pub(crate) type TokenSet = &'static [TokenKind];

/// Öğe seviyesi — en dış sınır.
pub(crate) const ITEM_START: TokenSet = &[
    KwModule, KwDomain, KwFn, KwStruct, KwEnum, KwConst, KwType, KwExtern, KwPub, KwUse, KwPackage,
    At, DocComment,
];

/// Port seviyesi (error-recovery.md §2; kurtarmada PORT_RECOVERY kullanılır).
#[allow(dead_code)]
pub(crate) const PORT_START: TokenSet = &[KwIn, KwOut, KwInout, At, DocComment];

/// Port hatasından sonra deyimlere de senkronize olunabilmeli
/// ("in b :" ardından "reg ..." atlanmamalı).
pub(crate) const PORT_RECOVERY: TokenSet = &[
    KwIn, KwOut, KwInout, At, DocComment, KwReg, KwLet, KwWire, KwOn, KwComb, KwFor, KwIf, KwMatch,
];

/// Deyim seviyesi.
pub(crate) const STMT_START: TokenSet =
    &[KwReg, KwLet, KwWire, KwOn, KwComb, KwFor, KwIf, KwMatch, At];

/// Blok içi deyim.
pub(crate) const BLOCK_STMT_START: TokenSet = &[KwIf, KwMatch, KwLet, KwFor, Ident];

/// Evrensel durak noktaları — her seviyede geçerli (EOF ayrıca kontrol edilir).
pub(crate) const UNIVERSAL_STOP: TokenSet = &[RBrace, Semi];

/// Payload'lı varyantlar için discriminant karşılaştırması.
pub(crate) fn same_kind(a: TokenKind, b: TokenKind) -> bool {
    std::mem::discriminant(&a) == std::mem::discriminant(&b)
}

pub(crate) fn set_contains(set: TokenSet, kind: TokenKind) -> bool {
    set.iter().any(|&k| same_kind(k, kind))
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RecoveryResult {
    AtSyncPoint,
    Recovered { skipped: usize },
    ReachedEof,
}

impl Parser<'_> {
    /// Ana kurtarma rutini (error-recovery.md §3).
    /// İlerleme garantisi: senkronizasyon noktasında değilsek en az
    /// bir token tüketilir.
    pub(crate) fn recover(&mut self, sync: TokenSet, err: Diagnostic) -> RecoveryResult {
        self.push_error(err);
        self.recover_silent(sync)
    }

    /// Hata zaten raporlandıysa yalnızca senkronize ol.
    pub(crate) fn recover_silent(&mut self, sync: TokenSet) -> RecoveryResult {
        let at_sync = self
            .current()
            .map(|k| set_contains(sync, k) || set_contains(UNIVERSAL_STOP, k))
            .unwrap_or(true);
        if at_sync {
            return RecoveryResult::AtSyncPoint;
        }

        // İlerleme garantisi: en az bir token tüket
        let start_pos = self.pos;
        self.bump_any();

        // Parantez derinliği takibi: iç bloktaki '}' dış sınır sanılmamalı
        let mut depth = 0i32;
        while let Some(kind) = self.current() {
            match kind {
                LBrace | LParen | LBracket => depth += 1,
                RBrace | RParen | RBracket => {
                    if depth == 0 {
                        return RecoveryResult::Recovered {
                            skipped: self.pos - start_pos,
                        };
                    }
                    depth -= 1;
                }
                k if depth == 0 && set_contains(sync, k) => {
                    return RecoveryResult::Recovered {
                        skipped: self.pos - start_pos,
                    };
                }
                Semi if depth == 0 => {
                    // ';' bozuk yapının sonlandırıcısıdır — tüketilir ki
                    // ardında hayalet Error öğesi kalmasın
                    self.bump_any();
                    return RecoveryResult::Recovered {
                        skipped: self.pos - start_pos,
                    };
                }
                _ => {}
            }
            self.bump_any();
        }

        RecoveryResult::ReachedEof
    }
}
