use gtk4::gdk::{Display, Key};
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    style_context_add_provider_for_display, Application, ApplicationWindow, Box as GtkBox, Button,
    CssProvider, EventControllerKey, Label, Orientation, ScrolledWindow,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AlexaUiState {
    pub status: String,
    pub text: String,
    #[serde(default)]
    pub subtext: String,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub is_long_text: bool,
    #[serde(default)]
    pub long_answer: String,
    #[serde(default)]
    pub updated_at: f64,
}

struct AppState {
    last_status: String,
    last_text: String,
    last_query: String,
    idle_since: Option<Instant>,
    is_visible: bool,
}

const CUSTOM_CSS: &str = r#"
window {
    background-color: transparent;
}

.overlay-container {
    background-color: rgba(18, 22, 34, 0.94);
    border-radius: 24px;
    border: 1.5px solid rgba(255, 255, 255, 0.16);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.6);
    padding: 16px 20px;
    margin: 12px;
}

.overlay-container.listening {
    border-color: #00d2ff;
    box-shadow: 0 0 20px rgba(0, 210, 255, 0.45);
}

.overlay-container.thinking {
    border-color: #a855f7;
    box-shadow: 0 0 20px rgba(168, 85, 247, 0.45);
}

.overlay-container.speaking {
    border-color: #10b981;
    box-shadow: 0 0 22px rgba(16, 185, 129, 0.45);
}

.status-badge {
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.8px;
    padding: 4px 12px;
    border-radius: 12px;
}

.status-badge.listening {
    background-color: rgba(0, 210, 255, 0.2);
    color: #00d2ff;
}

.status-badge.thinking {
    background-color: rgba(168, 85, 247, 0.2);
    color: #d8b4fe;
}

.status-badge.speaking {
    background-color: rgba(16, 185, 129, 0.2);
    color: #34d399;
}

.status-badge.idle {
    background-color: rgba(255, 255, 255, 0.08);
    color: #94a3b8;
}

.query-label {
    font-size: 14px;
    font-style: italic;
    color: #cbd5e1;
    margin-top: 6px;
    margin-bottom: 4px;
}

.answer-label {
    font-size: 15px;
    font-weight: 500;
    line-height: 1.45;
    color: #ffffff;
    margin-top: 6px;
}

.dismiss-button {
    background: transparent;
    color: #94a3b8;
    border: none;
    border-radius: 50%;
    padding: 2px 6px;
    font-size: 14px;
    font-weight: bold;
}

.dismiss-button:hover {
    background: rgba(255, 255, 255, 0.15);
    color: #ffffff;
}
"#;

fn get_state_file_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
        let p = PathBuf::from(xdg).join("alexa_state.json");
        if p.exists() {
            return p;
        }
    }
    PathBuf::from("/tmp/alexa_state.json")
}

fn read_ui_state() -> Option<AlexaUiState> {
    let path = get_state_file_path();
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str::<AlexaUiState>(&content).ok()
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Alexa Voice Overlay")
        .decorated(false)
        .default_width(540)
        .resizable(false)
        .build();

    // Escape key controller to dismiss
    let key_controller = EventControllerKey::new();
    let win_esc = window.downgrade();
    key_controller.connect_key_pressed(move |_, keyval, _, _| {
        if keyval == Key::Escape {
            if let Some(w) = win_esc.upgrade() {
                w.set_visible(false);
            }
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(key_controller);

    // Load custom CSS
    let provider = CssProvider::new();
    provider.load_from_data(CUSTOM_CSS);
    if let Some(display) = Display::default() {
        style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let root_box = GtkBox::new(Orientation::Vertical, 6);
    root_box.add_css_class("overlay-container");

    // Header box: Status Badge + Spacer + Close Button
    let header_box = GtkBox::new(Orientation::Horizontal, 8);

    let status_badge = Label::builder()
        .label("⚡ SẴN SÀNG")
        .halign(gtk4::Align::Start)
        .css_classes(["status-badge", "idle"])
        .build();

    let close_btn = Button::builder()
        .label("✕")
        .halign(gtk4::Align::End)
        .hexpand(true)
        .css_classes(["dismiss-button"])
        .build();

    header_box.append(&status_badge);
    header_box.append(&close_btn);
    root_box.append(&header_box);

    // Query Label
    let query_label = Label::builder()
        .label("")
        .halign(gtk4::Align::Start)
        .wrap(true)
        .wrap_mode(gtk4::pango::WrapMode::WordChar)
        .xalign(0.0)
        .visible(false)
        .css_classes(["query-label"])
        .build();
    root_box.append(&query_label);

    // Answer Scroll + Label
    let answer_label = Label::builder()
        .label("")
        .halign(gtk4::Align::Start)
        .wrap(true)
        .wrap_mode(gtk4::pango::WrapMode::WordChar)
        .xalign(0.0)
        .selectable(true)
        .visible(false)
        .css_classes(["answer-label"])
        .build();

    let scrolled_window = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .max_content_height(280)
        .propagate_natural_height(true)
        .child(&answer_label)
        .visible(false)
        .build();

    root_box.append(&scrolled_window);
    window.set_child(Some(&root_box));

    let win_weak = window.downgrade();
    close_btn.connect_clicked(move |_| {
        if let Some(w) = win_weak.upgrade() {
            w.set_visible(false);
        }
    });

    let app_state = Rc::new(RefCell::new(AppState {
        last_status: "idle".to_string(),
        last_text: String::new(),
        last_query: String::new(),
        idle_since: None,
        is_visible: false,
    }));

    // Poll /tmp/alexa_state.json every 60ms
    let win_ref = window.clone();
    let root_ref = root_box.clone();
    let badge_ref = status_badge.clone();
    let query_ref = query_label.clone();
    let answer_ref = answer_label.clone();
    let scroll_ref = scrolled_window.clone();
    let state_holder = app_state.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(60), move || {
        if let Some(data) = read_ui_state() {
            let mut st = state_holder.borrow_mut();
            let status = data.status.to_lowercase();
            let text = if data.is_long_text && !data.long_answer.is_empty() {
                data.long_answer.clone()
            } else {
                data.text.clone()
            };
            let query = data.query.clone();

            let status_changed = status != st.last_status;
            let text_changed = text != st.last_text;
            let query_changed = query != st.last_query;

            if status_changed || text_changed || query_changed {
                st.last_status = status.clone();
                st.last_text = text.clone();
                st.last_query = query.clone();

                // Clear previous classes
                root_ref.remove_css_class("listening");
                root_ref.remove_css_class("thinking");
                root_ref.remove_css_class("speaking");

                badge_ref.remove_css_class("listening");
                badge_ref.remove_css_class("thinking");
                badge_ref.remove_css_class("speaking");
                badge_ref.remove_css_class("idle");

                match status.as_str() {
                    "listening" => {
                        root_ref.add_css_class("listening");
                        badge_ref.add_css_class("listening");
                        badge_ref.set_label("🎙️ ĐANG LẮNG NGHE...");
                        query_ref.set_visible(false);
                        scroll_ref.set_visible(false);
                        answer_ref.set_visible(false);

                        st.idle_since = None;
                        if !st.is_visible {
                            win_ref.set_visible(true);
                            st.is_visible = true;
                        }
                    }
                    "thinking" => {
                        root_ref.add_css_class("thinking");
                        badge_ref.add_css_class("thinking");
                        badge_ref.set_label("🧠 ĐANG XỬ LÝ...");

                        if !query.is_empty() {
                            query_ref.set_label(&format!("👤 \"{}\"", query));
                            query_ref.set_visible(true);
                        } else {
                            query_ref.set_visible(false);
                        }
                        scroll_ref.set_visible(false);
                        answer_ref.set_visible(false);

                        st.idle_since = None;
                        if !st.is_visible {
                            win_ref.set_visible(true);
                            st.is_visible = true;
                        }
                    }
                    "speaking" => {
                        root_ref.add_css_class("speaking");
                        badge_ref.add_css_class("speaking");
                        badge_ref.set_label("🔊 TRỢ LÝ AI");

                        if !query.is_empty() {
                            query_ref.set_label(&format!("👤 \"{}\"", query));
                            query_ref.set_visible(true);
                        }

                        if !text.is_empty() {
                            answer_ref.set_label(&text);
                            answer_ref.set_visible(true);
                            scroll_ref.set_visible(true);
                        }

                        st.idle_since = None;
                        if !st.is_visible {
                            win_ref.set_visible(true);
                            st.is_visible = true;
                        }
                    }
                    _ => {
                        // Idle
                        badge_ref.add_css_class("idle");
                        badge_ref.set_label("⚡ SẴN SÀNG");
                        if st.idle_since.is_none() && st.is_visible {
                            st.idle_since = Some(Instant::now());
                        }
                    }
                }
            } else if status == "idle" && st.is_visible {
                // If it was idle for > 4.5 seconds, auto-hide the overlay
                if let Some(since) = st.idle_since {
                    if since.elapsed().as_secs_f32() > 4.5 {
                        win_ref.set_visible(false);
                        st.is_visible = false;
                        st.idle_since = None;
                    }
                }
            }
        }
        glib::ControlFlow::Continue
    });

    // Initially start hidden until wake word or trigger activates
    window.set_visible(false);
}

fn main() {
    let app = Application::builder()
        .application_id("com.tudm.voiceai.overlay")
        .build();

    app.connect_activate(build_ui);
    app.run_with_args::<String>(&[]);
}
