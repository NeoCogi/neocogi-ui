//
// Copyright 2022-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
// this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
// this list of conditions and the following disclaimer in the documentation
// and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
// may be used to endorse or promote products derived from this software without
// specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.
//
// -----------------------------------------------------------------------------
// Ported to rust from https://github.com/rxi/microui/ and the original license
//
// Copyright (c) 2020 rxi
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
//
//! Minimal facade-only application over dynamically or statically assembled COM plugins.
use neocogi_ui::*;
use neocogi_demo_host::{AppState, Host, HostError, report_exit};

/// Application behavior decides whether caption intent removes the logical window.
struct State;
impl AppState for State {
    /// Own the captured event while observing the logical window without retaining it.
    type Message = neocogi_com::events::EventMessage;
    /// Approves this demo's close request after the caption handler has returned.
    fn update(&mut self, message: Self::Message) -> Result<(), neocogi_com::NcStatus> {
        let Some(origin) = message.origin.service::<IWindowAbi>()? else {
            return Ok(());
        };
        if message.query::<IWindowCloseRequestedEventAbi>()?.is_some()
            && let Some(window) = origin.upgrade()?
        {
            window.close()?;
        }
        Ok(())
    }
}

/// Builds the same small tree on native SDL and browser WebGL through the typed facade.
fn run() -> Result<(), HostError> {
    neocogi_demo_host::initialize()?;
    let host = Host::new("Hello", Size { width: 640, height: 480 }, neocogi_demo_assets::load_bundle()?)?;
    let mut hello = host.ui.widgets().button_unobserved("Hello World!")?;
    hello.replace_flags(WidgetOptions::FRAME | WidgetOptions::ALIGN_CENTER)?;
    let mut tree = host.ui.widgets().linear(FlowDirection::LeftToRight, CrossSize::Stretch)?;
    tree.insert(0, hello, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))
        .map_err(|(status, _candidate)| status)?;
    let (queue, messages) = neocogi_com::events::channel();
    host.ui
        .window_with_queue("Hello Window", Rect { x: 40, y: 40, width: 300, height: 400 }, tree, Some(queue.as_com_ref()))
        .map_err(|(status, _candidate)| status)?;
    host.run(State, messages)
}

/// Reports startup and frame failures through formatting retained by the min-size profile.
fn main() -> std::process::ExitCode {
    report_exit(run())
}
