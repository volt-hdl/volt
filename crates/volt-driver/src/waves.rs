//! Dalga formu oturumu dosyaları (ADR-0092): VCD'nin yanına GTKWave
//! oturumu (`<ad>.gtkw`) ve enum başına çeviri tablosu
//! (`<ad>.filters/<Enum>.txt`). Metinler `volt_sv_emit::waves`'te
//! üretilir; burada yalnız yollar ve dosya yazımı.

use std::path::{Path, PathBuf};

use volt_diagnostics::lstr;
use volt_sv_emit::{filter_text, gtkw_text, WaveInfo, GTKW_MARKER};

/// VCD'yi üreten araç: hiyerarşi önekini belirler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WaveScope {
    /// Verilator `--trace`: `TOP.<Modül>.…`
    Simulation,
    /// sby karşı örneği / tümevarım izi: `<Modül>.…`
    Formal,
}

/// Oturum yazımının sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Session {
    /// Tasarımda enum/Trit sinyali yok: dosya üretilmedi.
    None,
    /// Oturum yazıldı.
    Written(PathBuf),
    /// Aynı adlı, Volt'un yazmadığı bir `.gtkw` var: dokunulmadı.
    Kept(PathBuf),
}

/// `<ad>.vcd` → `<ad>.gtkw`.
pub(crate) fn session_path(vcd: &Path) -> PathBuf {
    vcd.with_extension("gtkw")
}

/// `<ad>.vcd` → `<ad>.filters/`.
fn filters_dir(vcd: &Path) -> PathBuf {
    let stem = vcd
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    vcd.with_file_name(format!("{stem}.filters"))
}

/// `top` modülünden kurulan oturumu VCD'nin yanına yazar. Enum/Trit
/// sinyali yoksa hiçbir dosya üretilmez; daha önce Volt'un yazdığı eski
/// oturum varsa silinir (başka tasarımın adlarını göstermesin).
pub(crate) fn write_session(
    vcd: &Path,
    info: &WaveInfo,
    top: &str,
    scope: WaveScope,
) -> std::io::Result<Session> {
    let gtkw = session_path(vcd);
    let ours = is_volt_session(&gtkw);
    let prefix = match scope {
        WaveScope::Simulation => format!("TOP.{top}"),
        WaveScope::Formal => top.to_string(),
    };
    // Önce oturum var mı: enum'suz tasarımın çıktısı kullanıcının kendi
    // `.gtkw`'sinden etkilenmez.
    let Some(session) = info.session(top, &prefix) else {
        if ours {
            remove_session(&gtkw, &filters_dir(vcd))?;
        }
        return Ok(Session::None);
    };
    // Tablolar yalnız Volt'undur: `.gtkw` kullanıcınınsa da tazelenir
    // (GTKWave'de yeniden kaydedilen oturum işareti kaybeder ama `^N`
    // satırları bu dosyaları göstermeye devam eder).
    let dir = filters_dir(vcd);
    std::fs::create_dir_all(&dir)?;
    let names = filter_file_names(&session.tables);
    let mut filters = Vec::with_capacity(names.len());
    for (table, name) in session.tables.iter().zip(&names) {
        let path = dir.join(name);
        std::fs::write(&path, filter_text(table))?;
        filters.push(viewer_path(&path));
    }
    if gtkw.exists() && !ours {
        return Ok(Session::Kept(gtkw));
    }
    std::fs::write(&gtkw, gtkw_text(&session, &viewer_path(vcd), &filters))?;
    Ok(Session::Written(gtkw))
}

/// Tablo dosya adları `<Enum>.txt`; büyük/küçük harf duyarsız dosya
/// sisteminde çakışan ad (`State`/`STATE`) `<Enum>_<k>.txt` olur.
fn filter_file_names(tables: &[&volt_sv_emit::WaveTable]) -> Vec<String> {
    let mut taken = std::collections::HashSet::new();
    tables
        .iter()
        .map(|t| {
            let mut name = format!("{}.txt", t.name);
            let mut k = 2;
            while !taken.insert(name.to_ascii_lowercase()) {
                name = format!("{}_{k}.txt", t.name);
                k += 1;
            }
            name
        })
        .collect()
}

/// Dosya Volt'un yazdığı bir oturum mu (ilk satır işareti).
fn is_volt_session(gtkw: &Path) -> bool {
    std::fs::read_to_string(gtkw).is_ok_and(|t| t.lines().next() == Some(GTKW_MARKER))
}

/// Volt'un eski oturumunu ve tablolarını siler (yalnız `.txt`'ler; dizin
/// boş kalırsa o da).
fn remove_session(gtkw: &Path, dir: &Path) -> std::io::Result<()> {
    std::fs::remove_file(gtkw)?;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "txt") {
                std::fs::remove_file(&p)?;
            }
        }
        let _ = std::fs::remove_dir(dir);
    }
    Ok(())
}

/// Oturuma yazılan yol: Volt'a verildiği gibi (göreli ise göreli), `/`
/// ayraçlı. GTKWave göreli yolu `.gtkw`'nin dizinine değil çalışma
/// dizinine göre açar (ölçüm ADR-0092): basılan `gtkwave <vcd> <gtkw>`
/// komutu Volt'un koştuğu dizinden çalışır — Docker'da koşulduğunda da
/// (mutlak `/work/...` yolu ana makinede bulunmazdı).
fn viewer_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Kullanıcıya gösterilen tek satırlık açma komutu (etiketsiz).
pub(crate) fn open_hint(vcd: &Path, session: &Session) -> String {
    match session {
        Session::Written(gtkw) => format!("gtkwave {} {}", vcd.display(), gtkw.display()),
        Session::Kept(gtkw) => lstr!(
            en: "gtkwave {} (enum names: {} was not written by Volt, left unchanged)",
                vcd.display(), gtkw.display();
            tr: "gtkwave {} (enum adları: {} Volt'un değil, dokunulmadı)",
                vcd.display(), gtkw.display()
        ),
        // Oturum yoksa satır eskisi gibi: yalnız VCD yolu.
        Session::None => vcd.display().to_string(),
    }
}

/// Yazım hatası dalga formunu geçersiz kılmaz: uyarı basılır, oturumsuz
/// devam edilir.
pub(crate) fn write_or_warn(vcd: &Path, info: &WaveInfo, top: &str, scope: WaveScope) -> Session {
    match write_session(vcd, info, top, scope) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "{}",
                lstr!(
                    en: "warning: could not write the waveform session for {}: {e}", vcd.display();
                    tr: "uyarı: {} için dalga formu oturumu yazılamadı: {e}", vcd.display()
                )
            );
            Session::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use volt_sv_emit::{WaveModule, WaveSignal, WaveTable};

    fn info(with_enum: bool) -> WaveInfo {
        let signals = if with_enum {
            vec![WaveSignal {
                name: "state_r".into(),
                width: 2,
                table: "TxState".into(),
            }]
        } else {
            Vec::new()
        };
        WaveInfo {
            modules: vec![WaveModule {
                name: "UartTx".into(),
                signals,
                instances: vec![],
            }],
            tables: vec![WaveTable {
                name: "TxState".into(),
                width: 2,
                entries: vec![
                    (0, "Idle".into()),
                    (1, "Start".into()),
                    (2, "Data".into()),
                    (3, "Stop".into()),
                ],
            }],
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("volt-waves-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dizin");
        dir
    }

    #[test]
    fn writes_session_and_filters_next_to_the_vcd() {
        let dir = scratch("write");
        let vcd = dir.join("waves.vcd");
        let s = write_session(&vcd, &info(true), "UartTx", WaveScope::Simulation).expect("yazım");
        assert_eq!(s, Session::Written(dir.join("waves.gtkw")));
        let gtkw = std::fs::read_to_string(dir.join("waves.gtkw")).expect("gtkw");
        assert!(gtkw.starts_with(GTKW_MARKER), "{gtkw}");
        assert!(gtkw.contains("\nTOP.UartTx.state_r[1:0]\n"), "{gtkw}");
        assert!(gtkw.contains("waves.filters/TxState.txt"), "{gtkw}");
        let filter =
            std::fs::read_to_string(dir.join("waves.filters").join("TxState.txt")).expect("tablo");
        assert!(
            filter.ends_with("00 Idle\n01 Start\n10 Data\n11 Stop\n"),
            "{filter}"
        );
        let formal = dir.join("m_cex.vcd");
        write_session(&formal, &info(true), "UartTx", WaveScope::Formal).expect("yazım");
        let text = std::fs::read_to_string(dir.join("m_cex.gtkw")).expect("gtkw");
        assert!(text.contains("\nUartTx.state_r[1:0]\n"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn design_without_enums_writes_nothing_and_clears_a_stale_session() {
        let dir = scratch("none");
        let vcd = dir.join("waves.vcd");
        let s = write_session(&vcd, &info(false), "UartTx", WaveScope::Simulation).expect("ok");
        assert_eq!(s, Session::None);
        assert_eq!(std::fs::read_dir(&dir).expect("dizin").count(), 0);
        // Önceki koşunun Volt oturumu silinir.
        write_session(&vcd, &info(true), "UartTx", WaveScope::Simulation).expect("yazım");
        write_session(&vcd, &info(false), "UartTx", WaveScope::Simulation).expect("ok");
        assert_eq!(std::fs::read_dir(&dir).expect("dizin").count(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_users_own_session_file_is_never_overwritten() {
        let dir = scratch("keep");
        let vcd = dir.join("waves.vcd");
        let mine = "[timestart] 0\nTOP.x\n";
        std::fs::write(dir.join("waves.gtkw"), mine).expect("yazım");
        let s = write_session(&vcd, &info(true), "UartTx", WaveScope::Simulation).expect("ok");
        assert_eq!(s, Session::Kept(dir.join("waves.gtkw")));
        assert_eq!(
            std::fs::read_to_string(dir.join("waves.gtkw")).expect("oku"),
            mine
        );
        // Tablolar yine tazelenir (kullanıcı oturumu onları gösterebilir).
        assert!(dir.join("waves.filters").join("TxState.txt").is_file());
        // Enum'suz tasarımda kullanıcının dosyası çıktıyı değiştirmez.
        let s = write_session(&vcd, &info(false), "UartTx", WaveScope::Simulation).expect("ok");
        assert_eq!(s, Session::None);
        assert_eq!(
            std::fs::read_to_string(dir.join("waves.gtkw")).expect("oku"),
            mine
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Göreli yol göreli kalır (GTKWave çalışma dizinine göre çözer; Docker
    /// koşusunun `/work` yolu ana makinede yoktur).
    #[test]
    fn relative_paths_stay_relative() {
        let dir = PathBuf::from("target").join(format!("volt-waves-rel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dizin");
        let vcd = dir.join("w.vcd");
        write_session(&vcd, &info(true), "UartTx", WaveScope::Simulation).expect("yazım");
        let text = std::fs::read_to_string(dir.join("w.gtkw")).expect("gtkw");
        let rel = format!(
            "{}/w.filters/TxState.txt",
            dir.to_string_lossy().replace('\\', "/")
        );
        assert!(text.contains(&format!("^1 {rel}\n")), "{text}");
        assert!(!text.contains(":/") && !text.contains('\\'), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn case_colliding_enum_names_get_separate_files() {
        let t = |n: &str| WaveTable {
            name: n.into(),
            width: 1,
            entries: vec![],
        };
        let (a, b, c) = (t("State"), t("STATE"), t("Trit"));
        assert_eq!(
            filter_file_names(&[&a, &b, &c]),
            ["State.txt", "STATE_2.txt", "Trit.txt"]
        );
    }

    #[test]
    fn open_hint_names_both_files() {
        let vcd = Path::new("w.vcd");
        let s = Session::Written(PathBuf::from("w.gtkw"));
        assert_eq!(open_hint(vcd, &s), "gtkwave w.vcd w.gtkw");
        assert_eq!(open_hint(vcd, &Session::None), "w.vcd");
    }
}
