//! Renk kararı (cli-contract.md §3 "Genel Bayraklar" ve "Renk
//! Davranışı", #95): `--color auto|always|never` ve `--no-color` her
//! komutta geçerlidir; öncelik bayrak > `VOLT_COLOR` > auto. Auto'da
//! akış terminal değilse, `NO_COLOR` ya da `CI` ayarlıysa renk yok. Karar
//! akış başına verilir: tanılar stderr'e, `volt explain` stdout'a yazar.

use clap::ValueEnum;

/// cli-contract.md §3 `--color` değerleri; `VOLT_COLOR` aynı sözcükleri alır.
#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub(crate) enum ColorArg {
    Auto,
    Always,
    Never,
}

/// Kararı etkileyen ortam değişkenleri.
#[derive(Debug, Default)]
pub(crate) struct ColorEnv {
    /// `VOLT_COLOR` — tanınmayan değer yok sayılır (auto).
    pub(crate) volt_color: Option<ColorArg>,
    /// `NO_COLOR` tanımlı.
    pub(crate) no_color: bool,
    /// `CI` tanımlı.
    pub(crate) ci: bool,
}

impl ColorEnv {
    pub(crate) fn from_process() -> Self {
        ColorEnv {
            volt_color: std::env::var("VOLT_COLOR")
                .ok()
                .and_then(|v| ColorArg::from_str(v.trim(), true).ok()),
            no_color: std::env::var_os("NO_COLOR").is_some(),
            ci: std::env::var_os("CI").is_some(),
        }
    }
}

/// `--no-color`, `--color=never` kısayoludur.
pub(crate) fn flag(color: Option<ColorArg>, no_color: bool) -> Option<ColorArg> {
    if no_color {
        Some(ColorArg::Never)
    } else {
        color
    }
}

/// Terminal ANSI renk dizilerini işler mi. Windows konsolu bunları
/// yalnız sanal terminal kipinde işler (eski conhost); kip açılamazsa auto
/// renksiz kalır, ham kaçış kodları yazılmaz. Diğer sistemlerde evet.
pub(crate) fn terminal_takes_ansi() -> bool {
    anstyle_query::windows::enable_ansi_colors().unwrap_or(true)
}

/// Bu akışa renkli yazılsın mı.
pub(crate) fn enabled(flag: Option<ColorArg>, env: &ColorEnv, is_terminal: bool) -> bool {
    match flag.or(env.volt_color).unwrap_or(ColorArg::Auto) {
        ColorArg::Always => true,
        ColorArg::Never => false,
        ColorArg::Auto => is_terminal && !env.no_color && !env.ci,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(volt_color: Option<ColorArg>, no_color: bool, ci: bool) -> ColorEnv {
        ColorEnv {
            volt_color,
            no_color,
            ci,
        }
    }

    #[test]
    fn auto_colors_a_terminal_alone() {
        assert!(enabled(None, &env(None, false, false), true));
        assert!(!enabled(None, &env(None, false, false), false));
    }

    #[test]
    fn no_color_and_ci_turn_auto_off_on_a_terminal() {
        assert!(!enabled(None, &env(None, true, false), true));
        assert!(!enabled(None, &env(None, false, true), true));
        assert!(!enabled(
            Some(ColorArg::Auto),
            &env(None, true, false),
            true
        ));
    }

    #[test]
    fn the_flag_wins_over_volt_color_and_volt_color_over_auto() {
        let always = env(Some(ColorArg::Always), true, true);
        assert!(enabled(None, &always, false), "VOLT_COLOR=always");
        assert!(!enabled(Some(ColorArg::Never), &always, true));
        let never = env(Some(ColorArg::Never), false, false);
        assert!(!enabled(None, &never, true));
        assert!(enabled(Some(ColorArg::Always), &never, false));
    }

    #[test]
    fn no_color_flag_means_never() {
        assert_eq!(flag(Some(ColorArg::Always), true), Some(ColorArg::Never));
        assert_eq!(flag(None, false), None);
    }
}
