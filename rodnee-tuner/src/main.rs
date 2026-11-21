use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LOG_PATH: &str = "/data/adb/rodnee/tuner.log";
const BACKLIGHT_PATH: &str = "/sys/class/leds/lcd-backlight/brightness";

#[derive(Copy, Clone, PartialEq, Eq)]
enum ScreenState {
    Unknown,
    On,
    Off,
}

fn log_msg(msg: &str) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(meta) = fs::metadata(LOG_PATH) {
        if meta.len() > 64 * 1024 {
            let _ = fs::write(LOG_PATH, "");
        }
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(LOG_PATH) {
        let _ = writeln!(f, "[{ts}] {msg}");
    }
}

fn write_sysfs(path: &str, val: &str) -> bool {
    if !Path::new(path).exists() {
        return false;
    }
    fs::write(path, val).is_ok()
}

fn read_trimmed(path: &str) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn wait_for_boot_completed() {
    for _ in 0..90 {
        if let Ok(out) = Command::new("/system/bin/getprop")
            .arg("sys.boot_completed")
            .output()
        {
            if String::from_utf8_lossy(&out.stdout).trim() == "1" {
                break;
            }
        }
        thread::sleep(Duration::from_secs(1));
    }
    thread::sleep(Duration::from_secs(4));
}

fn refresh_xiaomi_thermal_if_needed() {
    let conf = "/data/vendor/thermal/config/thermal-normal.conf";
    if !Path::new(conf).exists() {
        return;
    }
    let _ = Command::new("/system/bin/restorecon").arg(conf).status();
    let cur = read_trimmed("/sys/class/thermal/thermal_message/sconfig").unwrap_or_default();
    let has_57000 = fs::read_to_string("/data/vendor/thermal/decrypt.txt")
        .map(|s| s.contains("57000"))
        .unwrap_or(false);
    if cur == "0" && !has_57000 {
        write_sysfs("/sys/class/thermal/thermal_message/sconfig", "1");
        thread::sleep(Duration::from_millis(200));
        write_sysfs("/sys/class/thermal/thermal_message/sconfig", "0");
        log_msg("Refreshed Xiaomi thermal-normal.conf profile");
    }
}

fn apply_baseline() {
    // Low-latency flash I/O (UFS/eMMC)
    for blk in ["sda", "sdb", "sdc", "mmcblk0"] {
        let base = format!("/sys/block/{blk}/queue");
        if Path::new(&base).exists() {
            write_sysfs(&format!("{base}/scheduler"), "deadline");
            write_sysfs(&format!("{base}/read_ahead_kb"), "128");
            write_sysfs(&format!("{base}/add_random"), "0");
            write_sysfs(&format!("{base}/iostats"), "0");
        }
    }

    // ZRAM + VM responsiveness & battery (page-cluster=0 avoids wasteful ZRAM readahead)
    write_sysfs("/proc/sys/vm/page-cluster", "0");
    write_sysfs("/proc/sys/vm/swappiness", "60");
    write_sysfs("/proc/sys/vm/vfs_cache_pressure", "50");
    write_sysfs("/proc/sys/vm/stat_interval", "10");

    // Network low-latency stack (BBR + ECN + TFO)
    write_sysfs("/proc/sys/net/ipv4/tcp_congestion_control", "bbr");
    write_sysfs("/proc/sys/net/ipv4/tcp_ecn", "1");
    write_sysfs("/proc/sys/net/ipv4/tcp_fastopen", "3");

    // Latency-sensitive foreground / UI cgroups
    write_sysfs("/dev/cpuctl/top-app/cpu.uclamp.latency_sensitive", "1");
    write_sysfs("/dev/cpuctl/foreground/cpu.uclamp.latency_sensitive", "1");

    // Gentle scheduler migration cost for responsive big.LITTLE wakeups
    write_sysfs("/proc/sys/kernel/sched_migration_cost_ns", "300000");

    // Hardware-accelerated Mali-G76 GED DVFS & boost
    write_sysfs("/sys/module/ged/parameters/gpu_dvfs_enable", "1");
    write_sysfs("/sys/module/ged/parameters/ged_boost_enable", "1");
    write_sysfs("/sys/module/ged/parameters/enable_gpu_boost", "1");

    log_msg("Applied baseline I/O, VM, BBR, uclamp, and Mali GED HW-accel tuning");
}

fn apply_screen_on() {
    // Fast ramp-up, smooth frame-pacing hold on LITTLE (policy0) and big (policy6) when exposed
    write_sysfs(
        "/sys/devices/system/cpu/cpufreq/policy0/schedutil/up_rate_limit_us",
        "500",
    );
    write_sysfs(
        "/sys/devices/system/cpu/cpufreq/policy0/schedutil/down_rate_limit_us",
        "10000",
    );
    write_sysfs(
        "/sys/devices/system/cpu/cpufreq/policy6/schedutil/up_rate_limit_us",
        "500",
    );
    write_sysfs(
        "/sys/devices/system/cpu/cpufreq/policy6/schedutil/down_rate_limit_us",
        "12000",
    );

    // Subtle UI floor for stutter-free 60 fps + cap noisy background tasks
    write_sysfs("/dev/cpuctl/top-app/cpu.uclamp.min", "5");
    write_sysfs("/dev/cpuctl/top-app/cpu.uclamp.latency_sensitive", "1");
    write_sysfs("/dev/cpuctl/background/cpu.uclamp.max", "50");

    // Responsive GPU DVFS threshold for locked 60 fps
    write_sysfs("/sys/module/ged/parameters/g_fb_dvfs_threshold", "75");

    // Standard writeback cadence
    write_sysfs("/proc/sys/vm/dirty_writeback_centisecs", "500");

    log_msg("Profile -> SCREEN_ON (locked 60fps low-latency)");
}

fn apply_screen_off() {
    // Aggressive ramp-down & conservative boost for cool standby
    write_sysfs(
        "/sys/devices/system/cpu/cpufreq/policy0/schedutil/up_rate_limit_us",
        "4000",
    );
    write_sysfs(
        "/sys/devices/system/cpu/cpufreq/policy0/schedutil/down_rate_limit_us",
        "1000",
    );
    write_sysfs(
        "/sys/devices/system/cpu/cpufreq/policy6/schedutil/up_rate_limit_us",
        "4000",
    );
    write_sysfs(
        "/sys/devices/system/cpu/cpufreq/policy6/schedutil/down_rate_limit_us",
        "1000",
    );

    // Zero UI floor, tighter cap on background wakeups
    write_sysfs("/dev/cpuctl/top-app/cpu.uclamp.min", "0");
    write_sysfs("/dev/cpuctl/background/cpu.uclamp.max", "35");

    // Relaxed GPU DVFS threshold
    write_sysfs("/sys/module/ged/parameters/g_fb_dvfs_threshold", "85");

    // Coalesce background dirty writebacks to save battery
    write_sysfs("/proc/sys/vm/dirty_writeback_centisecs", "1500");

    log_msg("Profile -> SCREEN_OFF (deep standby battery-saver)");
}

fn read_screen_state() -> ScreenState {
    match read_trimmed(BACKLIGHT_PATH) {
        Some(v) => match v.parse::<u32>() {
            Ok(0) => ScreenState::Off,
            Ok(_) => ScreenState::On,
            Err(_) => ScreenState::On,
        },
        None => ScreenState::On,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version") {
        println!("rodnee-tuner 1.0.0 (Rust aarch64-linux-android)");
        return;
    }

    let _ = fs::create_dir_all("/data/adb/rodnee");
    log_msg("rodnee-tuner starting (Rust native aarch64 daemon)");

    let once = args.iter().any(|a| a == "--once");
    if !once {
        wait_for_boot_completed();
    }

    refresh_xiaomi_thermal_if_needed();
    apply_baseline();

    let mut state = ScreenState::Unknown;
    loop {
        let cur = read_screen_state();
        if cur != state {
            match cur {
                ScreenState::On => apply_screen_on(),
                ScreenState::Off => apply_screen_off(),
                ScreenState::Unknown => {}
            }
            state = cur;
        }
        if once {
            break;
        }
        thread::sleep(Duration::from_secs(2));
    }
}
