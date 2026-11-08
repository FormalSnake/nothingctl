//! The flyout: what the tray icon opens.

use nd::components::Form;
use nd::core::WindowId;
use nd::prelude::*;
use nd::system::{Tray, TrayIcon, TrayMenuItem};
use nd::time::Instant;

use crate::link::{Commands, Event, State};
use crate::{art, place};

pub const TRAY: &str = "nothingctl";
const TRAY_PNG: &[u8] = include_bytes!("../assets/tray.png");
pub const WIDTH: f32 = 380.;
/// A click on the tray icon while the flyout is open first deactivates the app, which closes
/// it; a click this soon after that close is the same click and must not reopen it.
const REOPEN_GUARD: f32 = 0.4;

const TIERS: [&str; 5] = ["high", "mid", "low", "adaptive", "nc"];

/// The on/off settings shown as switches: JSON key, `set` verb, title, subtitle.
const TOGGLES: [(&str, &str, &str, &str); 7] = [
    ("lowLatency", "low-latency", "Low latency", "Less audio delay for games and video"),
    ("inEar", "in-ear", "In-ear detection", "Pause when you take them out"),
    ("personalSound", "personal-sound", "Personal sound", "Tuned to your hearing test"),
    ("personalizedAnc", "personalized-anc", "Personalised noise cancelling", "Tuned to your ear fit"),
    ("superMic", "super-mic", "Super mic", "Less background noise on calls"),
    ("callTransparency", "call-transparency", "Hear yourself on calls", "Transparency while you talk"),
    ("walkieTalkie", "walkie-talkie", "Walkie-talkie", "Hold to talk"),
];

fn title(word: &str) -> String {
    match word {
        "nc" => "Noise cancelling".into(),
        "dirac" => "Dirac Opteo".into(),
        _ => {
            let word = word.replace('-', " ");
            let mut chars = word.chars();
            chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
        }
    }
}

/// The noise cancelling strengths the device offers, in [`TIERS`] order.
fn tiers(anc: &crate::link::Choice) -> Vec<&String> {
    anc.available.iter().filter(|m| TIERS[..4].contains(&m.as_str())).collect()
}

pub struct Panel {
    state: Option<State>,
    reason: SharedString,
    failure: Option<SharedString>,
    commands: Commands,
    window: Option<WindowId>,
    closed_at: Option<Instant>,
    /// The noise cancelling tier picked last, restored when noise cancelling is switched on.
    tier: String,
    tooltip: String,
}

impl Panel {
    pub fn new(commands: Commands) -> Self {
        Self {
            state: None,
            reason: "Looking for headphones".into(),
            failure: None,
            commands,
            window: None,
            closed_at: None,
            tier: "high".into(),
            tooltip: String::new(),
        }
    }

    pub fn handle(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::State(s) => {
                if let Some(mode) = s.anc.as_ref().and_then(|a| a.mode.clone())
                    && TIERS.contains(&mode.as_str())
                {
                    self.tier = mode;
                }
                self.state = Some(*s);
                self.failure = None;
            }
            Event::Failed(message) => self.failure = Some(message.into()),
            Event::Down(reason) => {
                self.state = None;
                self.reason = if reason.starts_with("No Nothing device is connected") {
                    "Turn them on and connect them in Bluetooth settings.".into()
                } else {
                    reason.into()
                };
            }
        }
        cx.notify();
    }

    /// Shows the device and its battery on the tray icon's tooltip.
    pub fn update_tray(&mut self) {
        let tooltip = match &self.state {
            Some(s) => match s.battery.iter().map(|b| b.level).min() {
                Some(level) => format!("{} · {level}%", s.name),
                None => s.name.clone(),
            },
            None => "No headphones connected".into(),
        };
        if tooltip == self.tooltip {
            return;
        }
        self.tooltip = tooltip.clone();
        let tray = Tray::new(TRAY)
            .icon(TrayIcon::Png { bytes: TRAY_PNG.to_vec(), template: false })
            .tooltip(tooltip)
            .item(TrayMenuItem::item("quit", "Quit"));
        nd::system::spawn(nd::system::system().tray().set(tray), |_, _| {});
    }

    fn send(&mut self, verb: String, cx: &mut Context<Self>) {
        self.commands.send(&verb);
        cx.notify();
    }

    /// The flyout's height for the device: the window cannot resize once open, so every row it
    /// can show is counted. The metrics are the Fluent skin's, which `tests/flyout.rs` checks.
    pub fn height(&self) -> f32 {
        // Top padding, the hero, the gap under it and the form's top inset.
        const HERO: f32 = 165.;
        const TITLE: f32 = 23.;
        const GAP: f32 = 34.;
        const BOTTOM: f32 = 16.;
        const SEGMENTED: f32 = 50.;
        const ROW: f32 = 49.;
        const SWITCH: f32 = 63.;
        let Some(s) = &self.state else { return 168. };
        let mut cards = Vec::new();
        if let Some(anc) = &s.anc {
            cards.push(if tiers(anc).len() > 1 { SEGMENTED + ROW } else { SEGMENTED });
        }
        cards.push(ROW);
        if s.spatial.is_some() {
            cards.push(SEGMENTED);
        }
        let toggles = TOGGLES.iter().filter(|t| s.toggle(t.0).is_some()).count();
        if toggles > 0 {
            cards.push(SWITCH * toggles as f32);
        }
        let n = cards.len() as f32;
        HERO + cards.iter().map(|c| TITLE + c).sum::<f32>() + GAP * (n - 1.) + BOTTOM
    }

    pub fn toggle_window(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.window.take() {
            cx.close_window(id);
            return;
        }
        if self.closed_at.is_some_and(|t| t.elapsed().as_secs_f32() < REOPEN_GUARD) {
            return;
        }
        let height = self.height();
        let mut options = WindowOptions::main("flyout")
            .title("Nothing")
            .kind(WindowKind::Popup)
            .titlebar(Titlebar::Custom)
            .size(size(px(WIDTH), px(height)));
        if let Some(at) = place::origin(WIDTH, height) {
            options = options.origin(at);
        }
        let me = cx.entity();
        let handle = cx.open_window(options, move |_, _| me);
        self.window = Some(handle.id());
    }

    pub fn hide(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.window.take() {
            cx.close_window(id);
            self.closed_at = Some(Instant::now());
        }
    }

    fn hero(&self, s: &State, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let picture = match art::url(&s.model.base, &s.model.id) {
            Some(url) => image(ImageSource::url(url)).fit(ImageFit::Contain).size(px(112.)).into_any_element(),
            None => image_placeholder().size(px(112.)).into_any_element(),
        };
        let mode = s.anc.as_ref().and_then(|a| a.mode.as_deref()).map(|m| match m {
            "off" => "Noise control off".to_string(),
            "transparency" => "Transparency".to_string(),
            "nc" => "Noise cancelling".to_string(),
            tier => format!("{} noise cancelling", title(tier)),
        });
        let mut info = v_flex().gap_1().flex_1().child(label(s.name.clone()).text_style(TextStyle::Title3)).child(
            match &self.failure {
                Some(f) => label(f.clone()).tone(LabelTone::Danger).lines(1),
                None => label(mode.unwrap_or_else(|| "Connected".into())).secondary(),
            },
        );
        if let [single] = s.battery.as_slice() {
            info = info.child(
                v_flex()
                    .pt_2()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_1()
                            .items_baseline()
                            .child(label(format!("{}%", single.level)).text_style(TextStyle::Title1))
                            .when(single.charging, |r| r.child(label("Charging").secondary())),
                    )
                    .child(progress(Some(single.level as f32 / 100.)).w(px(140.))),
            );
        } else {
            let mut row = h_flex().pt_2().gap_4();
            for b in &s.battery {
                row = row.child(v_flex().child(label(format!("{}%", b.level)).text_style(TextStyle::Title3)).child(
                    label(if b.charging { format!("{}, charging", title(&b.id)) } else { title(&b.id) }).secondary(),
                ));
            }
            info = info.child(row);
        }
        h_flex().gap_4().items_center().child(picture).child(info).text_color(theme.text.primary)
    }

    fn controls(&self, s: &State, cx: &mut Context<Self>) -> Form {
        let mut form = form();
        if let Some(anc) = &s.anc {
            let tiers = tiers(anc);
            let has_nc = anc.available.iter().any(|m| TIERS.contains(&m.as_str()));
            let mut top: Vec<&str> = anc.available.iter().map(String::as_str).filter(|m| !TIERS.contains(m)).collect();
            if has_nc {
                top.push("nc");
            }
            let current = anc.mode.as_deref().unwrap_or("off");
            let in_tier = TIERS.contains(&current);
            let selected = top.iter().position(|m| *m == current || (in_tier && *m == "nc")).unwrap_or(0);
            let verbs: Vec<String> = top.iter().map(|m| m.to_string()).collect();
            let fallback = if tiers.is_empty() { "nc".to_string() } else { self.tier.clone() };
            let mut section = form_section().title("Noise control").child(
                segmented("anc", top.iter().map(|m| title(m))).selected(selected).on_change(cx.listener(
                    move |this, e: &SelectChanged, _, cx| {
                        let verb = &verbs[e.index];
                        let mode = if verb == "nc" { fallback.clone() } else { verb.clone() };
                        this.send(format!("anc {mode}"), cx);
                    },
                )),
            );
            if tiers.len() > 1 {
                let names: Vec<String> = tiers.iter().map(|t| t.to_string()).collect();
                let shown = if in_tier { current } else { self.tier.as_str() };
                let at = names.iter().position(|t| t == shown).unwrap_or(0);
                section = section.child(segmented("tier", names.iter().map(|t| title(t))).selected(at).on_change(
                    cx.listener(move |this, e: &SelectChanged, _, cx| {
                        this.send(format!("anc {}", names[e.index]), cx);
                    }),
                ));
            }
            form = form.child(section);
        }
        let presets = s.eq.available.clone();
        let at = s.eq.preset.as_ref().and_then(|p| presets.iter().position(|x| x == p)).unwrap_or(0);
        form = form.child(form_section().title("Equalizer").child(form_row("Preset").child(
            select("eq", presets.iter().map(|p| title(p))).selected(at).on_change(cx.listener(
                move |this, e: &SelectChanged, _, cx| {
                    this.send(format!("eq {}", presets[e.index]), cx);
                },
            )),
        )));
        if let Some(spatial) = &s.spatial {
            let modes = spatial.available.clone();
            let at = spatial.mode.as_ref().and_then(|m| modes.iter().position(|x| x == m)).unwrap_or(0);
            form = form.child(form_section().title("Spatial audio").child(
                segmented("spatial", modes.iter().map(|m| title(m))).selected(at).on_change(cx.listener(
                    move |this, e: &SelectChanged, _, cx| {
                        this.send(format!("spatial {}", modes[e.index]), cx);
                    },
                )),
            ));
        }
        let toggles: Vec<_> = TOGGLES.iter().filter_map(|t| Some((t, s.toggle(t.0)?))).collect();
        if !toggles.is_empty() {
            let mut section = form_section().title("Settings");
            for ((key, verb, name, hint), on) in toggles {
                section = section.child(switch_row(*verb, *name).subtitle(*hint).checked(on).on_toggle(cx.listener(
                    move |this, e: &Toggled, _, cx| {
                        if let Some(s) = this.state.as_mut() {
                            s.set_toggle(key, e.checked);
                        }
                        this.send(format!("{verb} {}", if e.checked { "on" } else { "off" }), cx);
                    },
                )));
            }
            form = form.child(section);
        }
        form
    }
}

impl Render for Panel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = v_flex().size_full().p_4().gap_4();
        let Some(state) = self.state.clone() else {
            return body
                .justify_center()
                .child(label("No headphones connected").text_style(TextStyle::Title3))
                .child(label(self.reason.clone()).secondary().wraps());
        };
        body.child(self.hero(&state, cx)).child(self.controls(&state, cx))
    }
}
