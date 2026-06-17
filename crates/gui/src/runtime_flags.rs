pub(crate) fn env_flag_enabled(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on" | "yes"
    )
}

pub(crate) fn disable_ipc() -> bool {
    env_flag("NOET_DISABLE_IPC")
}

pub(crate) fn disable_tray() -> bool {
    env_flag("NOET_DISABLE_TRAY")
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|value| env_flag_enabled(&value))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::env_flag_enabled;

    #[test]
    fn env_flag_enabled_accepts_common_true_values() {
        for value in ["1", "true", "TRUE", "on", "yes", " yes "] {
            assert!(env_flag_enabled(value), "{value:?} should be true");
        }
    }

    #[test]
    fn env_flag_enabled_rejects_false_and_unknown_values() {
        for value in ["", "0", "false", "off", "no", "anything-else"] {
            assert!(!env_flag_enabled(value), "{value:?} should be false");
        }
    }
}
