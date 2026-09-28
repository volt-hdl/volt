//! Ctrl-C ile temiz çıkış (ADR-0095 §4).
//!
//! `volt test --watch` kesmeyi yakalar: Docker'da koşan araç
//! konteynerleri adlandırılmıştır ve `docker rm -f` ile kaldırılır
//! (ADR-0094 sınırı: Windows'ta öldürülen `docker.exe` konteyneri geride
//! bırakabiliyordu). Kesme yakalayıcısı yalnız `--watch`'ta kurulur;
//! diğer komutlarda davranış ve Docker argümanları değişmez (adsız).
//!
//! Adlar `docker run` dönünce kayıttan DÜŞMEZ: istemci zorla
//! öldürüldüyse `docker run` hemen döner ama konteyner koşmaya devam eder
//! (ölçüm ADR-0095 §6). Kayıt her izleme koşusunun sonunda ve kesmede
//! tek bir `docker rm -f <adlar…>` çağrısıyla süpürülür; biten
//! konteynerin adı zaten yoktur (`--rm`), çağrı onu sessizce geçer.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// Ctrl-C'nin konvansiyonel çıkış kodu (128 + SIGINT).
pub(crate) const INTERRUPTED: i32 = 130;

/// Konteyner, ilk `rm -f` anında henüz oluşmamış olabilir (istemci
/// isteği göndermiş, daemon kurmamış): kısa bir beklemeyle bir kez daha.
const REMOVE_RETRY: Duration = Duration::from_millis(500);

static ENABLED: AtomicBool = AtomicBool::new(false);
static STOPPING: AtomicBool = AtomicBool::new(false);
static SEQ: AtomicU64 = AtomicU64::new(0);
/// Süpürülmemiş konteyner adları (docker yürütülebiliri, ad).
static NAMED: Mutex<Vec<(PathBuf, String)>> = Mutex::new(Vec::new());

/// Kesme yakalayıcısını kurar; `on_stop` çıkıştan önce basılacak iletiyi
/// yazar. Yakalayıcı kurulamazsa (başka biri kurmuş) hata metni döner.
pub(crate) fn install(on_stop: fn()) -> Result<(), String> {
    ENABLED.store(true, Ordering::SeqCst);
    ctrlc::set_handler(move || {
        STOPPING.store(true, Ordering::SeqCst);
        // Süren bir süpürmenin `rm`'i de kesmeyi alıp ölmüş olabilir:
        // kayıttaki her ad yeniden kaldırılır.
        let named = named_snapshot();
        remove(&named);
        if !named.is_empty() {
            std::thread::sleep(REMOVE_RETRY);
            remove(&named);
        }
        on_stop();
        std::process::exit(INTERRUPTED);
    })
    .map_err(|e| e.to_string())
}

/// Koşu sonu süpürmesi (`--watch`): bu koşunun adlı konteynerlerinden
/// geride kalan varsa kaldırılır. Adlı konteyner yoksa Docker çağrılmaz.
/// Adlar kayıttan ancak `rm` bittikten sonra düşer.
pub(crate) fn sweep() {
    let named = named_snapshot();
    remove(&named);
    if let Ok(mut n) = NAMED.lock() {
        n.retain(|e| !named.contains(e));
    }
}

fn named_snapshot() -> Vec<(PathBuf, String)> {
    NAMED.lock().map(|n| n.clone()).unwrap_or_default()
}

/// `docker rm -f <adlar…>`, docker yürütülebilirine göre gruplanmış.
fn remove(named: &[(PathBuf, String)]) {
    let mut dockers: Vec<&PathBuf> = named.iter().map(|(d, _)| d).collect();
    dockers.dedup();
    for docker in dockers {
        let names = named.iter().filter(|(d, _)| d == docker).map(|(_, n)| n);
        let _ = Command::new(docker)
            .args(["rm", "-f"])
            .args(names)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// Kesme sürüyorsa (yakalayıcı konteynerleri kaldırıyor) çağıran iş
/// parçacığını süreç çıkana dek bekletir: öldürülen konteynerin çıkış
/// kodu (137) yanlışlıkla "bellek bitti" diye raporlanmasın.
pub(crate) fn park_if_stopping() {
    while STOPPING.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Yeni `docker run`'ın adı: yakalayıcı kurulu değilse `None` (adsız,
/// eski argümanlar). Ad süpürülene dek kayıtta kalır.
pub(crate) fn container_name_for(docker: &Path) -> Option<String> {
    if !ENABLED.load(Ordering::SeqCst) {
        return None;
    }
    // Kesme sürerken yeni konteyner başlatılmaz: süreç birazdan çıkar.
    park_if_stopping();
    let name = container_name(std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst));
    if let Ok(mut n) = NAMED.lock() {
        n.push((docker.to_path_buf(), name.clone()));
    }
    Some(name)
}

/// `volt-watch-<pid>-<sıra>`: aynı makinedeki iki `--watch` çakışmaz.
fn container_name(pid: u32, seq: u64) -> String {
    format!("volt-watch-{pid}-{seq}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_per_process_and_sequence() {
        assert_eq!(container_name(42, 0), "volt-watch-42-0");
        assert_ne!(container_name(42, 0), container_name(42, 1));
        assert_ne!(container_name(42, 0), container_name(43, 0));
    }

    #[test]
    fn without_the_handler_containers_stay_unnamed() {
        // Yakalayıcı bu test sürecinde kurulmaz: eski davranış (adsız).
        assert_eq!(container_name_for(Path::new("docker")), None);
        assert!(named_snapshot().is_empty());
    }
}
