//! Setup failure owns text/status independently of the fresh device's lifetime.
use super::*;
use std::{cell::Cell, rc::Rc};
#[com_object(interfaces(neocogi_render_core::abi::IRenderer, neocogi_render_core::abi::IRenderDiagnostics))]
/// Fresh device-shaped fixture exposes only the two actual diagnostic projections.
struct Renderer {
    /// Bounded device detail.
    text: String,
    /// Observes destruction before later error formatting.
    dropped: Rc<Cell<bool>>,
}
impl Drop for Renderer {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}
impl neocogi_render_core::abi::IRenderDriverBound for Renderer {
    fn driver_identity(&self) -> Result<neocogi_com::ComPtr<neocogi_render_core::abi::IRenderDriverIdentityAbi>, neocogi_com::NcStatus> {
        Err(neocogi_com::NC_E_NOINTERFACE)
    }
}
impl IRenderer for Renderer {
    fn render(&self, _: RenderInfo, _: neocogi_com::ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        Err(neocogi_com::NC_E_INVALID_ARG)
    }
}
impl IRenderDiagnostics for Renderer {
    fn diagnostic(&self) -> Result<ComPtr<neocogi_com::INcStringAbi>, NcStatus> {
        Ok(neocogi_com::NcString::from_utf8_str(&self.text))
    }
}
/// This is the exact capture function called by Host::new's failing first bind.
#[test]
fn first_bind_error_preserves_phase_status_and_owned_detail() {
    for text in ["compiler rejected UI shader".to_string(), String::new(), "é".repeat(2048)] {
        let dropped = Rc::new(Cell::new(false));
        let renderer = Renderer::from_value(Renderer {
            text: text.clone(),
            dropped: dropped.clone(),
        })
        .query()
        .unwrap();
        let status = neocogi_com::NC_E_INVALID_ARG;
        let error = setup_error(&renderer, status);
        drop(renderer);
        assert!(dropped.get());
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
