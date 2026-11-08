#![cfg_attr(windows, windows_subsystem = "windows")]

use nd::prelude::*;
use nd::system::SystemEvent;
use nothingctl_tray::link::{self, Event};
use nothingctl_tray::panel::Panel;

fn pump(mut rx: tokio::sync::mpsc::UnboundedReceiver<Event>, panel: WeakEntity<Panel>) {
    nd::system::spawn(async move { (rx.recv().await, rx) }, move |(event, rx), cx| {
        let Some(event) = event else { return };
        if panel
            .update(cx, |p, cx| {
                p.handle(event, cx);
                p.update_tray();
            })
            .is_some()
        {
            pump(rx, panel);
        }
    });
}

fn main() -> nd::Result<()> {
    // One tray icon: a later launch (autostart beside a manual start) finds the lock held and
    // exits. The OS drops the lock with the process.
    let lock = nd::system::app_dirs()
        .ensure_data()
        .ok()
        .and_then(|dir| std::fs::File::create(dir.join("instance.lock")).ok());
    if lock.as_ref().is_some_and(|f| f.try_lock().is_err()) {
        return Ok(());
    }
    Application::new(nd::manifest!()).run(|cx| {
        cx.set_quit_when_windows_close(false);
        let (commands, events) = link::start();
        let panel = cx.new(|_| Panel::new(commands));
        panel.update(cx, |p, _| p.update_tray());
        pump(events, panel.downgrade());
        let sys = nd::system::system();
        nd::system::on_event(&sys, move |event, cx| match event {
            SystemEvent::Tray { item: None, .. } => panel.update(cx, |p, cx| p.toggle_window(cx)),
            SystemEvent::Tray { item: Some(item), .. } if item == "quit" => cx.quit(),
            SystemEvent::Deactivated => panel.update(cx, |p, cx| p.hide(cx)),
            _ => {}
        });
    })
}
