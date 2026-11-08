//! Renders the flyout headless in the Windows skin (Fluent), light and dark, for a headphone, a
//! pair of buds and no device, and checks the content fits the height the flyout opens at.
//! The pictures land in `target/flyout/<case>-<scheme>.png`.

use std::path::PathBuf;
use std::sync::Arc;

use nd_backend_gpui::GpuiHost;
use nd_gpui::gpui::HeadlessAppContext;
use nd_theme::{OsTheme, Preset, Scheme, ThemeRegistry};
use nd_ui::prelude::*;
use nd_ui::{SemanticNode, Titlebar, WindowId};
use nothingctl_tray::link::{Commands, Event, State};
use nothingctl_tray::panel::{Panel, WIDTH};

const HEADPHONE: &str = r#"{"type":"state","address":"2C:BE:EE:65:A0:21","name":"CMF Headphone Pro","model":{"base":"B175","id":"1EFB39","name":"CMF Headphone Pro","kind":"headphone"},"firmware":"1.0.1.49","battery":[{"id":"single","level":60,"charging":false}],"anc":{"mode":"mid","available":["off","transparency","high","mid","low","adaptive"]},"eq":{"preset":"electronic","available":["rock","electronic","pop","vocals","classical","custom"],"custom":null},"spatial":{"mode":"off","available":["off","concert","theatre"]},"bass":null,"lowLatency":true,"personalSound":false,"inEar":null}"#;
const BUDS: &str = r#"{"type":"state","address":"AA:BB:CC:DD:EE:FF","name":"CMF Buds Pro 2","model":{"base":"B172","id":"F29566","name":"CMF Buds Pro 2","kind":"earbuds"},"battery":[{"id":"left","level":80,"charging":false},{"id":"right","level":78,"charging":false},{"id":"case","level":40,"charging":true}],"anc":{"mode":"transparency","available":["off","transparency","high","mid","low","adaptive"]},"eq":{"preset":"dirac","available":["dirac","rock","pop","custom"],"custom":null},"spatial":{"mode":"off","available":["off","fixed"]},"lowLatency":false,"inEar":true}"#;

fn headless() -> HeadlessAppContext {
    let text = nd_gpui::gpui_platform::current_platform(true).text_system();
    HeadlessAppContext::with_platform(text, Arc::new(()), nd_gpui::gpui_platform::current_headless_renderer)
}

fn settle(cx: &mut HeadlessAppContext, host: &GpuiHost) {
    for _ in 0..6 {
        cx.run_until_parked();
        cx.update(|cx| {
            host.pump(cx);
            host.draw(cx);
        });
    }
    cx.run_until_parked();
}

/// Where the lowest leaf ends: containers fill the window, so only leaves say where the content
/// stops. Form cards end their bottom padding below their last row.
fn bottom(node: &SemanticNode) -> f32 {
    if node.children.is_empty() {
        return node.bounds.map_or(0., |b| b.origin.y + b.size.height);
    }
    node.children.iter().map(bottom).fold(0., f32::max)
}

fn render(case: &str, state: Option<&str>, scheme: Scheme) -> bool {
    let mut cx = headless();
    let mut opened: Option<(WindowId, f32)> = None;
    let state: Option<State> = state.map(|s| serde_json::from_str(s).expect("state line"));
    let host = cx.update(|cx| {
        GpuiHost::builder().quit_when_empty(false).start(cx, |app| {
            let panel = app.new(|_| Panel::new(Commands::default()));
            let event = match state {
                Some(s) => Event::State(Box::new(s)),
                None => Event::Down("No Nothing device is connected".into()),
            };
            panel.update(app, |p, cx| p.handle(event, cx));
            let height = panel.read(app).height();
            let handle = app.open_window(
                WindowOptions::main("flyout").titlebar(Titlebar::Custom).size(size(px(WIDTH), px(height))),
                move |_, _| panel,
            );
            opened = Some((handle.id(), height));
        })
    });
    let mut registry = ThemeRegistry::new(Preset::Fluent);
    registry.set_os(OsTheme { scheme: Some(scheme), ..registry.os().clone() });
    host.backend().set_theme(registry);
    settle(&mut cx, &host);
    let (window, height) = opened.expect("window");
    let content = host.read(|app| bottom(&app.semantic_window(window).expect("window").root));
    let capture = host.job(|app| app.capture(window)).expect("capture");
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/flyout");
    std::fs::create_dir_all(&dir).expect("out dir");
    let name = format!("{case}-{}.png", if scheme == Scheme::Dark { "dark" } else { "light" });
    std::fs::write(dir.join(&name), &capture.png).expect("write png");
    println!("{name}: content ends at {content:.0} of {height:.0}");
    content <= height
}

fn main() {
    if nd_gpui::gpui_platform::current_headless_renderer().ok().flatten().is_none() {
        println!("skipped: GPUI has no headless renderer on this OS");
        return;
    }
    let mut fits = true;
    for scheme in [Scheme::Light, Scheme::Dark] {
        fits &= render("headphone", Some(HEADPHONE), scheme);
        fits &= render("buds", Some(BUDS), scheme);
        fits &= render("none", None, scheme);
    }
    assert!(fits, "the content runs past the flyout's height");
}
