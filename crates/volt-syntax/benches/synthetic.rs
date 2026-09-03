//! Bench'ler için sentetik Volt kaynağı üretici.
//!
//! Yalnızca ui/pass korpusunda doğrulanmış yapıları kullanır (port,
//! reg, on bloğu, kombinasyonel atama) — üretilen dosya hatasız
//! ayrıştırılmalıdır; bench kurulumunda bu doğrulanır.

/// Yaklaşık `target_lines` satırlık, sözdizimsel olarak geçerli bir
/// Volt kaynağı üretir. Her modül sabit 16 satırdır.
pub fn generate_large_module(target_lines: usize) -> String {
    const LINES_PER_MODULE: usize = 16;
    let module_count = target_lines.div_ceil(LINES_PER_MODULE);
    let mut src = String::with_capacity(target_lines * 24);
    for i in 0..module_count {
        src.push_str(&format!(
            "// Sentetik modül {i}\n\
             module Synth{i} {{\n\
             \x20   in  clk : clock\n\
             \x20   in  a : u8\n\
             \x20   in  b : u8\n\
             \x20   out sum : u8\n\
             \x20   out acc_out : u8\n\
             \n\
             \x20   reg acc : u8 = 0\n\
             \n\
             \x20   on clk {{\n\
             \x20       acc <= acc + a\n\
             \x20   }}\n\
             \n\
             \x20   sum = a + b\n\
             \x20   acc_out = acc\n\
             }}\n"
        ));
    }
    src
}
