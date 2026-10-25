//! Ограничения обмена с удалёнными репозиториями в процессе сервера (фича 038,
//! research.md §3, ревью безопасности).
//!
//! На сервере «это устройство» — сам сервер: его сеть, его файлы, его ключи ssh и
//! конфигурация git администратора. Проверка адреса при подключении этого не
//! закрывает: имя может разрешиться во внутренний адрес (в том числе позже —
//! DNS rebinding), а адрес уже проверенный может переписать конфигурация git.
//! Поэтому сервер один раз при запуске включает здесь режим, в котором:
//! - транспорт HTTP ходит только по `https` и только на внешние адреса — после
//!   разрешения имени, на каждом запросе;
//! - транспорт ssh не запускается;
//! - libgit2 не читает конфигурацию git пользователя процесса и системы;
//! - получение ограничено по объёму и по времени.
//!
//! Режим — свойство процесса, а не вызова: в процессе сервера других обменов нет.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

const DEVICE: u8 = 0;
const SERVER: u8 = 1;
const SERVER_PRIVATE: u8 = 2;

static MODE: AtomicU8 = AtomicU8::new(DEVICE);

/// Сколько байт может прийти одним обменом на сервере: диск общий для всех
/// аккаунтов, и огромный репозиторий одного не должен занять его весь.
pub const SERVER_FETCH_LIMIT_BYTES: usize = 2 * 1024 * 1024 * 1024;

/// Сколько может идти один обмен на сервере. Предел простоя (20 с на чтение)
/// не ловит сервер, который присылает по байту чуть чаще.
pub const SERVER_EXCHANGE_LIMIT: Duration = Duration::from_secs(30 * 60);

/// Включает режим сервера. `allow_private` — администратор разрешил
/// репозитории во внутренней сети (`sync_private_remotes`).
pub fn restrict_to_server(allow_private: bool) {
    MODE.store(
        if allow_private {
            SERVER_PRIVATE
        } else {
            SERVER
        },
        Ordering::SeqCst,
    );
    // `insteadOf`, `pushInsteadOf`, `core.sshCommand` и автор администратора не
    // должны действовать на обмены пользователей.
    for level in [
        git2::ConfigLevel::System,
        git2::ConfigLevel::XDG,
        git2::ConfigLevel::Global,
        git2::ConfigLevel::ProgramData,
    ] {
        // SAFETY: настройка процесса libgit2; меняется при запуске сервера, до
        // первой операции с репозиторием.
        unsafe {
            let _ = git2::opts::set_search_path(level, "");
        }
    }
}

/// Работает ли процесс сервером.
pub fn on_server() -> bool {
    MODE.load(Ordering::SeqCst) != DEVICE
}

/// Отказывать ли в соединении с этим адресом.
pub fn refuses_ip(ip: IpAddr) -> bool {
    MODE.load(Ordering::SeqCst) == SERVER && is_internal_ip(ip)
}

/// Внутренний ли адрес: loopback, частные сети, link-local, CGNAT, служебные и
/// групповые диапазоны; IPv6 — ULA, link- и site-local, а также IPv6, внутри
/// которых записан внутренний IPv4 (mapped, compatible, NAT64, 6to4).
pub fn is_internal_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => internal_v4(ip),
        IpAddr::V6(ip) => internal_v6(ip),
    }
}

fn internal_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || a == 0
        // 100.64.0.0/10 — общий адрес провайдера (CGNAT).
        || (a == 100 && (b & 0xc0) == 64)
        // 192.0.0.0/24 — служебные адреса IETF.
        || (a == 192 && b == 0 && c == 0)
        // 198.18.0.0/15 — сети для проверок.
        || (a == 198 && (b & 0xfe) == 18)
        // 224.0.0.0/4 и 240.0.0.0/4 — групповые и зарезервированные.
        || a >= 224
}

fn internal_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    let embedded = |high: usize| {
        Ipv4Addr::new(
            (segments[high] >> 8) as u8,
            segments[high] as u8,
            (segments[high + 1] >> 8) as u8,
            segments[high + 1] as u8,
        )
    };
    if ip.is_loopback() || ip.is_unspecified() {
        return true;
    }
    if let Some(v4) = ip.to_ipv4_mapped() {
        return internal_v4(v4);
    }
    // `::a.b.c.d` — IPv4-compatible.
    if segments[..6].iter().all(|s| *s == 0) {
        return internal_v4(embedded(6));
    }
    // 64:ff9b::/96 — NAT64.
    if segments[0] == 0x64 && segments[1] == 0xff9b && segments[2..6].iter().all(|s| *s == 0) {
        return internal_v4(embedded(6));
    }
    // 2002::/16 — 6to4: IPv4 во втором и третьем слове.
    if segments[0] == 0x2002 {
        return internal_v4(embedded(1));
    }
    (segments[0] & 0xfe00) == 0xfc00 // ULA
        || (segments[0] & 0xffc0) == 0xfe80 // link-local
        || (segments[0] & 0xffc0) == 0xfec0 // site-local
        || (segments[0] & 0xff00) == 0xff00 // multicast
}

/// Разрешение имён для HTTP-клиента на сервере: внутренние адреса отбрасываются
/// после разрешения, на каждом соединении, — переразрешение имени (DNS
/// rebinding) не уводит обмен во внутреннюю сеть.
pub(super) struct ServerResolver;

impl reqwest::dns::Resolve for ServerResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let found = tokio::task::spawn_blocking(move || {
                (host.as_str(), 0)
                    .to_socket_addrs()
                    .map(|addrs| addrs.collect::<Vec<SocketAddr>>())
            })
            .await
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>)??;
            let allowed: Vec<SocketAddr> = found
                .into_iter()
                .filter(|addr| !refuses_ip(addr.ip()))
                .collect();
            if allowed.is_empty() {
                return Err("адрес ведёт во внутреннюю сеть сервера".into());
            }
            Ok(Box::new(allowed.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_ranges() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.0.5",
            "169.254.169.254",
            "100.100.1.1",
            "0.1.2.3",
            "192.0.0.8",
            "198.18.0.1",
            "224.0.0.1",
            "255.255.255.255",
            "::1",
            "::",
            "fd00::1",
            "fe80::1",
            "fec0::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "::7f00:1",
            "64:ff9b::a00:1",
            "2002:c0a8:1::1",
        ] {
            assert!(is_internal_ip(ip.parse().unwrap()), "{ip}");
        }
        for ip in [
            "8.8.8.8",
            "140.82.112.3",
            "2001:db8::1",
            "2606:4700::1111",
            "2002:808:808::1",
        ] {
            assert!(!is_internal_ip(ip.parse().unwrap()), "{ip}");
        }
    }
}
