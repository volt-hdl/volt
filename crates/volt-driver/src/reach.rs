//! Çıktı kümesi (ADR-0042 ek): `build` ve `verify` yalnız ana dosyadan
//! erişilebilen modülleri yazar/doğrular.
//!
//! `use` ile yüklenen dosya kütüphanedir: `use riscv_core::imm_i_of` bir
//! fn ister, `RiscvCore`/`UartTx` modüllerini değil. Eskiden birimdeki her
//! modül `build/rtl/`'ye yazılıyor, kontratları da `verify`'da koşuyordu;
//! yalnız fn içeren bir dosya ise modülsüz, boş bir `.sv` üretiyordu
//! (Verilator `--top-module` reddeder — ADR-0081 Aşama 3 bulgusu).
//!
//! Süzgeç tanılara dokunmaz: kütüphane dosyasının tamamı yine denetlenir
//! (ADR-0070 paritesi — `check` ile `build` aynı tanıları görür). Yalnız
//! ÇIKTI kümesi daralır: RTL, SVA, formal görevleri, kısıtlar (SDC/XDC) ve
//! `@mmio` sürücüleri aynı kümeye bağlıdır. `run`/`test` süzülmez: test
//! dosyası modül tanımlamaz, test ettiği modülleri `use` ile alır.

use std::collections::HashSet;

use volt_span::FileId;

use crate::Compiled;

impl Compiled {
    /// Çıktıları ana dosyanın modülleri + örnekleme kapanışına indirir.
    /// Hata nedeniyle çıktı yoksa (`sv == None`) dokunmaz.
    pub(crate) fn retain_reachable(&mut self) {
        if self.sv.is_none() {
            return;
        }
        let library: HashSet<FileId> = self.library_files.iter().copied().collect();
        let keep = volt_sv_emit::reachable_modules(&self.ast, |f| !library.contains(&f));
        self.modules.retain(|m| keep.contains(&m.name));
        self.sva_files.retain(|f| keep.contains(&f.module_name));
        self.sva_props.retain(|p| keep.contains(&p.module_name));
        self.unclocked_contracts
            .retain(|u| keep.contains(&u.module));
        self.regmaps.retain(|r| keep.contains(&r.module));
        self.constraints
            .modules
            .retain(|m| keep.contains(&m.module));
        self.sv = Some(volt_sv_emit::unit_sv(&self.source_name, &self.modules));
    }
}
