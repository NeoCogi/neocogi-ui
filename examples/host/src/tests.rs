//! Host context preserves each operation's status and owned text.
use super::*;

/// Later failures cannot overwrite an earlier error; all reports can be dropped before formatting.
#[test]
fn first_bind_error_preserves_phase_status_and_owned_detail() {
    for text in ["compiler rejected UI shader".to_string(), String::new(), "é".repeat(2048)] {
        let status = neocogi_com::NC_E_INVALID_ARG;
        let error = render_error("UI renderer setup", RenderError::new(status, text.clone()));
        let _later = render_error("frame rendering", RenderError::new(neocogi_com::NC_E_OUT_OF_MEMORY, "unrelated failure"));
        match &error {
            HostError::Operation { operation, status: actual, detail } => {
                assert_eq!(*operation, "UI renderer setup");
                assert_eq!(*actual, status);
                assert_eq!(detail.as_deref(), if text.is_empty() { None } else { Some(text.as_str()) });
            }
            _ => panic!("setup lost typed context"),
        }
        assert!(std::error::Error::source(&error).is_some());
        assert!(error.to_string().contains("UI renderer setup failed"));
    }
}

/// Every entry-point error representation reaches the same Display-based exit conversion.
#[test]
fn report_exit_accepts_host_status_and_boxed_errors() {
    assert_eq!(report_exit(Ok::<_, HostError>(())), std::process::ExitCode::SUCCESS);
    assert_eq!(report_exit(Err(HostError::Message("host diagnostic".into()))), std::process::ExitCode::FAILURE);
    assert_eq!(report_exit(Err(neocogi_com::NC_E_INVALID_ARG)), std::process::ExitCode::FAILURE);
    let boxed: Box<dyn std::error::Error> = Box::new(neocogi_com::NC_E_INVALID_ARG);
    assert_eq!(report_exit(Err(boxed)), std::process::ExitCode::FAILURE);
}

/// Invoke the real private parser with child-owned environment instead of mutating
/// this process's environment while unrelated tests may be running on other threads.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn frame_limit_distinguishes_absence_from_invalid_native_values() {
    const CHILD: &str = "NEOCOGI_TEST_FRAME_LIMIT_EXPECTED";
    if let Some(expected) = std::env::var_os(CHILD) {
        let result = frame_limit();
        match expected.to_str().unwrap() {
            "absent" => assert!(matches!(result, Ok(None))),
            "one" => assert!(matches!(result, Ok(Some(1)))),
            "max" => assert!(matches!(result, Ok(Some(u32::MAX)))),
            "invalid" => assert_eq!(result.unwrap_err().to_string(), "demo frame limit must be a positive u32"),
            _ => panic!("unknown child expectation"),
        }
        return;
    }
    let mut cases: Vec<(&str, Option<std::ffi::OsString>)> = vec![
        ("absent", None),
        ("one", Some("1".into())),
        ("max", Some(u32::MAX.to_string().into())),
        ("invalid", Some("".into())),
        ("invalid", Some("0".into())),
        ("invalid", Some("abc".into())),
        ("invalid", Some("-1".into())),
        ("invalid", Some("4294967296".into())),
    ];
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        cases.push(("invalid", Some(std::ffi::OsString::from_vec(vec![0xff]))));
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        cases.push(("invalid", Some(std::ffi::OsString::from_wide(&[0xd800]))));
    }
    for (expected, value) in cases {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args(["--exact", "tests::frame_limit_distinguishes_absence_from_invalid_native_values", "--nocapture"])
            .env(CHILD, expected);
        match &value {
            Some(value) => {
                child.env("NEOCOGI_DEMO_FRAMES", value);
            }
            None => {
                child.env_remove("NEOCOGI_DEMO_FRAMES");
            }
        }
        let output = child.output().unwrap();
        assert!(
            output.status.success(),
            "{value:?}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
