//! Explicit opt-ins for external services and persistent document excerpts.

fn enabled(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| value == "1")
}

pub fn remote_media() -> bool {
    enabled("LEAF_REMOTE_MEDIA")
}

pub fn online_speech() -> bool {
    enabled("LEAF_ONLINE_SPEECH")
}

pub fn agent_send() -> bool {
    enabled("LEAF_AGENT_SEND")
}

pub fn remember_selection() -> bool {
    enabled("LEAF_REMEMBER_SELECTION")
}

/// Media passed to a system opener or ffmpeg must be a web URL, never a
/// local path, command option, or another protocol supported by the helper.
pub fn web_url(text: &str) -> bool {
    !text.chars().any(char::is_control)
        && url::Url::parse(text).is_ok_and(|url| {
            matches!(url.scheme(), "https" | "http")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_cannot_name_files_or_helper_options() {
        for input in [
            "file:///etc/passwd",
            "/tmp/movie.mp4",
            "-aCalculator",
            "concat:https://example.com/a|file:///etc/passwd",
            "https://user:secret@example.com/a",
            "https://example.com/a\n",
        ] {
            assert!(!web_url(input), "accepted {input:?}");
        }
        assert!(web_url("https://example.com/movie.mp4?token=abc"));
    }

    #[test]
    fn disabled_features_have_no_side_effects() {
        const CHILD: &str = "LEAF_PRIVACY_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let data = std::env::temp_dir().join(format!("leaf-privacy-{}", std::process::id()));
            std::fs::create_dir(&data).unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "privacy::tests::disabled_features_have_no_side_effects",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .env("XDG_DATA_HOME", &data)
                .env_remove("LEAF_REMOTE_MEDIA")
                .env_remove("LEAF_ONLINE_SPEECH")
                .env_remove("LEAF_AGENT_SEND")
                .env_remove("LEAF_REMEMBER_SELECTION")
                .output()
                .unwrap();
            let saved = data.join("leaf/ask.json").exists();
            std::fs::remove_dir_all(data).unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            assert!(!saved, "a selection was written without an opt-in");
            return;
        }

        // A local server proves image fetching returns before making a request.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/tracking.png", listener.local_addr().unwrap());
        assert!(crate::article::fetch_image(&url).is_none());
        assert!(matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock));
        assert!(matches!(
            crate::edge::speak("private text", crate::edge::DEFAULT_VOICE),
            Err(crate::edge::EdgeFailed::Disabled)
        ));
        assert!(matches!(
            crate::edge::voices(),
            Err(crate::edge::EdgeFailed::Disabled)
        ));
        crate::edge::warm_up();
        assert!(matches!(
            crate::agent::deliver("private text"),
            Err(crate::agent::AgentFailed::Disabled)
        ));
        assert!(!remember_selection());
        crate::agent::remember("private text", "private context");
        assert!(crate::agent::recall("private text").is_none());
        assert!(matches!(
            crate::video::Playback::start(&url, (2, 2)),
            Err(crate::video::PlayFailed::Blocked)
        ));
        assert!(crate::prmedia::open("file:///etc/passwd").is_err());
    }
}
