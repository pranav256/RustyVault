#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod appearance;
mod browser_import;

use appearance::{Palette, CONTROL_RADIUS, WINDOW_RADIUS};
use browser_import::{BraveExportWatch, BrowserKind};
use eframe::egui::{self, RichText, Vec2};
use rusty_vault::{
    browser::{self, Request as BrowserRequest, Response as BrowserResponse},
    crypto::{self, DerivedKey},
    generator::{generate_passphrase, generate_password, GeneratorConfig},
    vault::{self, Attachment, Vault, VaultEntry, VaultError, VaultSettings},
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let demo = args.iter().any(|s| s == "--demo");
    let argument = |name: &str| {
        args.iter()
            .position(|s| s == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let size = argument("--size")
        .and_then(|s| {
            s.split_once('x')
                .and_then(|(w, h)| Some([w.parse::<f32>().ok()?, h.parse::<f32>().ok()?]))
        })
        .unwrap_or([1200.0, 800.0]);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_min_inner_size([900.0, 650.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/rustyvault.png"))
                    .expect("The bundled RustyVault icon must be a valid PNG"),
            )
            .with_title(if demo {
                "RustyVault — Demo"
            } else {
                "RustyVault"
            }),
        ..Default::default()
    };
    eframe::run_native(
        "RustyVault",
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let mut app = App::new(demo, argument("--vault").map(PathBuf::from));
            if demo {
                app.dark_mode = argument("--theme").as_deref() != Some("light");
            }
            theme(&cc.egui_ctx, app.dark_mode);
            if demo {
                app.capture = argument("--capture").map(PathBuf::from);
                match argument("--view").as_deref() {
                    Some("health") => app.view = View::Health,
                    Some("settings") => app.view = View::Settings,
                    Some("generator") => app.view = View::Generator,
                    Some("passphrase") => {
                        app.view = View::Generator;
                        app.passphrase = true;
                        app.generated =
                            generate_passphrase(app.words, &app.separator, app.capitalize);
                    }
                    Some("export") => {
                        app.view = View::Settings;
                        app.confirmation = Some(Confirm::Export);
                    }
                    Some("import") => {
                        app.view = View::Settings;
                        app.pending_import = Some(vec![VaultEntry::new(
                            "Example account",
                            "alex",
                            "",
                            "SYNTHETIC-DEMO-ONLY",
                        )]);
                    }
                    Some("create") => {
                        app.lock(&cc.egui_ctx);
                        app.create = true;
                    }
                    Some("edit") => {
                        if let Some(e) = app.selected_entry() {
                            app.edit(e);
                        }
                    }
                    Some("empty") => {
                        app.vault.as_mut().unwrap().entries.clear();
                        app.selected = None;
                        app.analyze();
                    }
                    Some("new") => app.add_password(),
                    Some("browser-import") => app.open_browser_guide(false),
                    Some("browser-setup") => app.open_browser_guide(true),
                    Some("named-password") => {
                        app.add_password();
                        app.editing.as_mut().unwrap().service = "Example account".into();
                    }
                    Some("import-duplicates") => {
                        let existing = app.selected_entry().unwrap();
                        let mut changed = existing.clone();
                        changed.id = format!("{:x}", rand::random::<u128>());
                        changed.password = "SYNTHETIC-CHANGED-PASSWORD".into();
                        app.pending_import = Some(vec![existing, changed]);
                    }
                    Some("locked") => app.lock(&cc.egui_ctx),
                    _ => {}
                }
            }
            Ok(Box::new(app))
        }),
    )
}
const INPUT_PADDING: egui::Margin = egui::Margin::symmetric(12.0, 8.0);

fn appearance_path() -> PathBuf {
    directories::ProjectDirs::from("com", "pranav256", "rusty-vault")
        .map(|d| d.config_dir().join("appearance.txt"))
        .unwrap_or_else(|| "appearance.txt".into())
}

fn save_appearance(path: &Path, dark: bool) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, if dark { "dark" } else { "light" })
}

fn display(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name("Display".into()))
}

fn apply_palette(ctx: &egui::Context, dark: bool) {
    let colors = Palette::new(dark);
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("active-palette"), colors));
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = colors.background;
    visuals.window_fill = colors.surface;
    visuals.faint_bg_color = colors.surface;
    visuals.extreme_bg_color = colors.surface;
    visuals.override_text_color = Some(colors.foreground);
    visuals.hyperlink_color = colors.accent;
    visuals.warn_fg_color = colors.accent;
    visuals.error_fg_color = colors.danger;
    visuals.selection.bg_fill = colors.warm;
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, colors.accent);
    visuals.window_rounding = WINDOW_RADIUS.into();
    visuals.window_stroke = egui::Stroke::new(1.0_f32, colors.border);
    visuals.window_shadow = egui::epaint::Shadow::NONE;
    visuals.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.45 };
    visuals.slider_trailing_fill = true;
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.rounding = CONTROL_RADIUS.into();
        widget.bg_stroke = egui::Stroke::NONE;
        widget.fg_stroke = egui::Stroke::new(1.0_f32, colors.foreground);
    }
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, colors.border);
    visuals.widgets.inactive.bg_fill = colors.surface;
    visuals.widgets.inactive.weak_bg_fill = colors.surface;
    visuals.widgets.hovered.bg_fill = colors.warm;
    visuals.widgets.hovered.weak_bg_fill = colors.warm;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, colors.accent);
    visuals.widgets.active.bg_fill = colors.warm;
    visuals.widgets.active.weak_bg_fill = colors.warm;
    visuals.widgets.active.bg_stroke = egui::Stroke::new(2.0_f32, colors.accent);
    visuals.widgets.open.bg_fill = colors.warm;
    visuals.widgets.open.weak_bg_fill = colors.warm;
    ctx.set_visuals(visuals);
}

fn theme(ctx: &egui::Context, dark: bool) {
    apply_palette(ctx, dark);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 8.0);
        s.spacing.icon_spacing = 8.0;
        s.spacing.slider_rail_height = 4.0;
        s.spacing.scroll.bar_width = 8.0;
        s.spacing.scroll.floating_width = 3.0;
        s.spacing.button_padding = Vec2::new(12.0, 6.0);
        s.spacing.interact_size.y = 32.0;
        s.spacing.window_margin = egui::Margin::same(24.0);
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        s.text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
        s.text_styles
            .insert(egui::TextStyle::Heading, display(26.0));
    });
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Inter".into(),
        egui::FontData::from_static(include_bytes!("../assets/Inter.ttf")),
    );
    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "Inter".into());
    fonts.families.insert(
        egui::FontFamily::Name("Display".into()),
        vec!["Inter".into()],
    );
    ctx.set_fonts(fonts);
}

#[derive(Clone, PartialEq)]
enum View {
    All,
    Favorites,
    Group(String),
    Tag(String),
    Trash,
    Health,
    Generator,
    Settings,
}
struct EntryRow {
    id: String,
    service: String,
    subtitle: String,
    favorite: bool,
}
impl View {
    fn title(&self) -> &str {
        match self {
            Self::All => "All entries",
            Self::Favorites => "Favorites",
            Self::Group(s) | Self::Tag(s) => s,
            Self::Trash => "Trash",
            Self::Health => "Password health",
            Self::Generator => "Password generator",
            Self::Settings => "Vault settings",
        }
    }
}
enum Confirm {
    BrowserSave(Box<VaultEntry>),
    Trash(String),
    Purge(String),
    Export,
    Discard,
    RestoreHistory(String, usize),
}

struct App {
    vault: Option<Vault>,
    key: Option<DerivedKey>,
    vault_lock: Option<std::fs::File>,
    path: PathBuf,
    demo: bool,
    dark_mode: bool,
    master: String,
    confirm_master: String,
    create: bool,
    view: View,
    query: String,
    selected: Option<String>,
    reveal: bool,
    editing: Option<VaultEntry>,
    editor_uses_email: bool,
    tags_input: String,
    expiry_input: String,
    error: Option<String>,
    notice: String,
    confirmation: Option<Confirm>,
    pending_import: Option<Vec<VaultEntry>>,
    pending_import_indices: Option<Vec<usize>>,
    dialog_focus: Option<Option<egui::Id>>,
    group_name: String,
    config: GeneratorConfig,
    generated: String,
    passphrase: bool,
    words: usize,
    separator: String,
    capitalize: bool,
    settings: VaultSettings,
    new_master: String,
    new_confirm: String,
    last_input: Instant,
    clipboard: Option<(Zeroizing<String>, Instant)>,
    weak: HashSet<String>,
    capture: Option<PathBuf>,
    frames: usize,
    bounds: Vec<serde_json::Value>,
    browser: Option<browser::Server>,
    browser_failed: bool,
    browser_kind: BrowserKind,
    browser_import_open: bool,
    browser_setup_only: bool,
    browser_setup_path: Option<PathBuf>,
    browser_import_pending: bool,
    brave_import_watch: Option<(BraveExportWatch, PathBuf, Instant)>,
    brave_import_next_poll: Instant,
    browser_last_seen: Option<Instant>,
}

impl App {
    fn new(demo: bool, path: Option<PathBuf>) -> Self {
        let path = path.unwrap_or_else(|| {
            directories::ProjectDirs::from("com", "pranav256", "rusty-vault")
                .map(|d| d.data_local_dir().join("vault.enc"))
                .unwrap_or_else(|| "vault.enc".into())
        });
        let mut app = Self {
            create: !path.exists(),
            path,
            demo,
            dark_mode: demo
                || std::fs::read_to_string(appearance_path()).ok().as_deref() != Some("light"),
            vault: None,
            key: None,
            vault_lock: None,
            master: String::new(),
            confirm_master: String::new(),
            view: View::All,
            query: String::new(),
            selected: None,
            reveal: false,
            editing: None,
            editor_uses_email: false,
            tags_input: String::new(),
            expiry_input: String::new(),
            error: None,
            notice: String::new(),
            confirmation: None,
            pending_import: None,
            pending_import_indices: None,
            dialog_focus: None,
            group_name: String::new(),
            config: GeneratorConfig::default(),
            generated: generate_password(&GeneratorConfig::default()),
            passphrase: false,
            words: 6,
            separator: "-".into(),
            capitalize: false,
            settings: VaultSettings::default(),
            new_master: String::new(),
            new_confirm: String::new(),
            last_input: Instant::now(),
            clipboard: None,
            weak: HashSet::new(),
            capture: None,
            frames: 0,
            bounds: Vec::new(),
            browser: None,
            browser_failed: false,
            browser_kind: BrowserKind::default(),
            browser_import_open: false,
            browser_setup_only: false,
            browser_setup_path: None,
            browser_import_pending: false,
            brave_import_watch: None,
            brave_import_next_poll: Instant::now(),
            browser_last_seen: None,
        };
        if demo {
            let mut vault = Vault::new();
            for (title, user, group, tags) in [
                ("GitHub", "alex.morgan", "Work", "Development"),
                ("Linear", "alex@studio.example", "Work", "Projects"),
                ("Proton Mail", "alex.morgan", "Personal", "Email"),
                ("Figma", "alex@studio.example", "Work", "Design"),
                ("Nintendo", "alex.morgan", "Personal", "Gaming"),
                ("Home network", "admin", "Personal", "Home"),
            ] {
                let mut e = VaultEntry::new(title, user, "", generate_password(&app.config));
                e.category = group.into();
                e.tags = vec![tags.into()];
                e.url = format!("https://{}.com", title.to_lowercase().replace(' ', ""));
                e.favorite = title == "GitHub" || title == "Proton Mail";
                e.notes =
                    "Personal account. Recovery codes are stored in the attached file.".into();
                if title == "GitHub" {
                    e.attachments.push(Attachment {
                        name: "recovery-codes.txt".into(),
                        bytes: b"SYNTHETIC-DEMO-ONLY".to_vec(),
                    });
                }
                if title == "Nintendo" {
                    e.password = "password123".into();
                }
                vault.add_entry(e);
            }
            app.selected = vault.entries.first().map(|e| e.id.clone());
            app.vault = Some(vault);
            app.create = false;
            app.analyze();
        }
        app
    }

    fn change_appearance(&mut self, ctx: &egui::Context, dark: bool) {
        self.dark_mode = dark;
        theme(ctx, dark);
        ctx.request_repaint();
        if !self.demo {
            let path = appearance_path();
            if let Err(error) = self
                .check_export_path(&path)
                .and_then(|_| save_appearance(&path, dark))
            {
                self.error = Some(format!("Appearance changed for this session, but the preference could not be saved: {error}"));
            }
        }
    }
    fn track(&mut self, id: &str, label: &str, role: &str, response: &egui::Response) {
        if self.capture.is_some() {
            let r = if role == "textbox" {
                response.interact_rect
            } else {
                response.rect
            };
            self.bounds.push(serde_json::json!({"id":id,"label":label,"role":role,"interactive":matches!(role,"button"|"textbox"|"slider"|"checkbox"|"combobox"),"rect":{"x":r.min.x,"y":r.min.y,"width":r.width(),"height":r.height()}}));
        }
    }
    fn selected_entry(&self) -> Option<VaultEntry> {
        self.vault
            .as_ref()?
            .entries
            .iter()
            .find(|e| Some(&e.id) == self.selected.as_ref())
            .cloned()
    }
    fn analyze(&mut self) {
        self.weak = self
            .vault
            .as_ref()
            .map(|v| {
                v.entries
                    .iter()
                    .filter(|e| e.deleted_at.is_none() && e.is_weak())
                    .map(|e| e.id.clone())
                    .collect()
            })
            .unwrap_or_default();
    }
    fn commit(&mut self, next: Vault, message: &str) -> bool {
        let result = if self.demo {
            Ok(())
        } else if let Some(key) = &self.key {
            next.save_to_file(&self.path, key)
        } else {
            Err(VaultError::InvalidData("Unlock the vault first".into()))
        };
        match result {
            Ok(()) => {
                self.settings = next.settings.clone();
                self.vault = Some(next);
                self.analyze();
                self.error = None;
                self.notice = message.into();
                true
            }
            Err(e) => {
                self.error = Some(format!("Nothing was saved: {e}"));
                false
            }
        }
    }
    fn edit(&mut self, entry: VaultEntry) {
        self.editor_uses_email = entry.username.is_empty() && !entry.email.is_empty();
        self.tags_input = entry.tags.join(", ");
        self.expiry_input = entry
            .expires_at
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_default();
        self.editing = Some(entry);
        self.error = None;
        self.reveal = false;
    }
    fn add_password(&mut self) {
        if self.editing.is_some() {
            return;
        }
        let mut entry = VaultEntry::new("", "", "", "");
        if let View::Group(group) = &self.view {
            entry.category.clone_from(group);
        }
        self.view = View::All;
        self.query.clear();
        self.edit(entry);
    }
    fn password_actions(&mut self, ui: &mut egui::Ui) {
        ui.add_enabled_ui(self.editing.is_none(), |ui| {
            let add = ui
                .add_sized([ui.available_width(), 44.0], primary(ui, "Add password"))
                .on_hover_text("Add password (Ctrl+N)");
            self.track("add-password", "Add password", "button", &add);
            if add.clicked() {
                self.add_password();
            }
            let import = ui.add_sized(
                [ui.available_width(), 40.0],
                egui::Button::new("Import from browser…"),
            );
            self.track("import-browser", "Import from browser", "button", &import);
            if import.clicked() {
                self.open_browser_guide(false);
            }
        });
    }

    fn open_browser_guide(&mut self, setup: bool) {
        self.browser_import_open = true;
        self.browser_setup_only = setup;
        self.brave_import_watch = None;
        self.error = None;
    }

    fn stage_import(&mut self, entries: Vec<VaultEntry>, from_browser: bool) {
        self.pending_import = Some(entries);
        self.pending_import_indices = None;
        self.browser_import_pending = from_browser;
        self.browser_import_open = false;
        self.brave_import_watch = None;
        self.error = None;
    }

    fn start_brave_import(&mut self) {
        if self.demo {
            self.choose_csv(true);
            return;
        }
        let result = browser_import::brave_export_directory().and_then(|directory| {
            let watcher = BraveExportWatch::new(directory.clone())?;
            BrowserKind::Brave.open_password_manager()?;
            Ok((watcher, directory, Instant::now()))
        });
        match result {
            Ok(watch) => {
                self.brave_import_watch = Some(watch);
                self.brave_import_next_poll = Instant::now();
                self.error = None;
                self.notice = "Waiting for your approved Brave export".into();
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn poll_brave_import(&mut self) {
        if self.brave_import_watch.is_none() || Instant::now() < self.brave_import_next_poll {
            return;
        }
        self.brave_import_next_poll = Instant::now() + Duration::from_secs(1);
        let Some((mut watcher, directory, started)) = self.brave_import_watch.take() else {
            return;
        };
        if self.vault.is_none() || !self.browser_import_open {
            return;
        }
        if started.elapsed() > Duration::from_secs(300) {
            self.error =
                Some("Import timed out. Start again, or choose an exported CSV file.".into());
            return;
        }
        match watcher.poll() {
            Ok(Some(path)) => match read_small_file(&path, 16 * 1024 * 1024)
                .map_err(VaultError::from)
                .and_then(|bytes| vault::read_csv(&Zeroizing::new(bytes)))
            {
                Ok(entries) => self.stage_import(entries, true),
                Err(error) => self.error = Some(error.to_string()),
            },
            Ok(None) => self.brave_import_watch = Some((watcher, directory, started)),
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    fn keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::L)) {
            self.lock(ctx);
        }
        if self.vault.is_none()
            || self.editing.is_some()
            || self.confirmation.is_some()
            || self.pending_import.is_some()
            || self.browser_import_open
        {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::N)) {
            self.add_password();
        } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::F)) {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("search")));
        }
    }

    fn choose_csv(&mut self, from_browser: bool) {
        let result = if self.demo {
            let mut entry = VaultEntry::new(
                "Example imported account",
                "demo@example.test",
                "",
                "SYNTHETIC-DEMO-ONLY",
            );
            entry.url = "https://example.test".into();
            Some(Ok(vec![entry]))
        } else {
            rfd::FileDialog::new()
                .add_filter("Password CSV", &["csv"])
                .pick_file()
                .map(|path| {
                    read_small_file(&path, 16 * 1024 * 1024)
                        .map_err(VaultError::from)
                        .and_then(|bytes| vault::read_csv(&Zeroizing::new(bytes)))
                })
        };
        if let Some(result) = result {
            match result {
                Ok(entries) => self.stage_import(entries, from_browser),
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }

    fn open_browser_password_manager(&mut self) {
        if self.demo {
            self.notice = format!(
                "Demo: {} stays closed. Choose CSV to preview a synthetic import.",
                self.browser_kind.label()
            );
        } else if let Err(error) = self.browser_kind.open_password_manager() {
            self.error = Some(error.to_string());
        } else {
            self.error = None;
            self.notice = format!("Opened {} password manager", self.browser_kind.label());
        }
    }

    fn setup_browser_connection(&mut self) {
        if self.demo {
            self.browser_setup_path = Some(PathBuf::from("Demo extension folder"));
            self.notice = "Demo: no browser files or registry settings were changed.".into();
            self.error = None;
            return;
        }
        match self.browser_kind.setup_connection() {
            Ok(path) => {
                self.browser_setup_path = Some(path);
                self.error = None;
                self.notice = format!(
                    "Connection prepared for {}. Add the extension next.",
                    self.browser_kind.label()
                );
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn browser_status(&self) -> &'static str {
        if self.demo {
            "Demo: browser connections and system changes are simulated."
        } else if !self
            .vault
            .as_ref()
            .is_some_and(|vault| vault.settings.browser_enabled)
        {
            "Connection off"
        } else if self.browser_failed {
            "Connection could not start. Turn it off and on to retry."
        } else if self.browser_last_seen.is_some() {
            "The extension contacted this vault during this unlocked session."
        } else if self.browser.is_some() {
            "Listening locally. Waiting for the extension to contact this vault."
        } else {
            "Starting the local connection..."
        }
    }

    fn browser_connection_controls(&mut self, ui: &mut egui::Ui) {
        let colors = Palette::get(ui.ctx());
        let mut enabled = self.vault.as_ref().unwrap().settings.browser_enabled;
        let response = ui
            .add_enabled_ui(cfg!(windows) || self.demo, |ui| {
                checkbox(
                    ui,
                    &mut enabled,
                    "Allow the extension while this vault is unlocked",
                )
            })
            .inner;
        self.track(
            "browser-enable",
            "Allow the browser extension",
            "checkbox",
            &response,
        );
        if response.changed() {
            let mut next = self.vault.as_ref().unwrap().clone();
            next.settings.browser_enabled = enabled;
            if self.commit(next, "Browser connection settings saved") {
                self.browser_failed = false;
                self.browser_last_seen = None;
            }
        }
        ui.label(self.browser_status());
        ui.label(RichText::new("Enabling this does not install the extension or import passwords. Saving a submitted login still needs your approval in RustyVault.").small().color(colors.muted));
    }

    fn browser_import_dialog(&mut self, ctx: &egui::Context, focus_new: bool) {
        if !self.browser_import_open {
            return;
        }
        let colors = Palette::get(ctx);
        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        egui::Window::new(if self.browser_setup_only {
            "Set up browser extension"
        } else {
            "Import passwords from your browser"
        })
            .order(egui::Order::Foreground).collapsible(false).resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).default_width(540.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().id_salt("browser-import-scroll")
                    .max_height((ctx.screen_rect().height() - 220.0).clamp(240.0, 480.0))
                    .show(ui, |ui| {
                        ui.label(if self.browser_setup_only {
                            "Connect your browser to save and fill logins with RustyVault."
                        } else {
                            "Bring saved passwords into this vault using your browser's password export."
                        });
                        ui.horizontal(|ui| {
                            let label = ui.label("Browser");
                            let selected = self.browser_kind;
                            let response = egui::ComboBox::from_id_salt("browser-kind")
                                .selected_text(self.browser_kind.label()).show_ui(ui, |ui| {
                                    for browser in BrowserKind::ALL {
                                        ui.selectable_value(&mut self.browser_kind, browser, browser.label());
                                    }
                                }).response.labelled_by(label.id);
                            self.track("browser-kind", "Browser", "combobox", &response);
                            if self.browser_kind != selected { self.browser_setup_path = None; self.brave_import_watch = None; }
                        });
                        ui.add_space(8.0);
                        if self.browser_setup_only {
                            self.browser_setup_controls(ui);
                        } else {
                        if self.browser_kind == BrowserKind::Brave {
                            ui.label(RichText::new("1. Start the import").strong());
                            let start = ui.add_enabled(self.brave_import_watch.is_none(), primary(ui, "Open Brave and detect export"));
                            self.track("brave-start-import", "Open Brave and detect export", "button", &start);
                            if start.clicked() { self.start_brave_import(); }
                            ui.label(RichText::new("2. Approve the export in Brave").strong());
                            ui.label("In Brave's Export passwords section, choose Download file and complete any Windows authentication.");
                            ui.label(RichText::new("3. Save to Downloads, then review here").strong());
                            ui.label("RustyVault detects the new password export automatically. Keep this vault unlocked until the review appears.");
                            if let Some((_, directory, _)) = &self.brave_import_watch {
                                ui.label(RichText::new("Waiting for your Brave export...").strong());
                                ui.label(directory.to_string_lossy().as_ref());
                            }
                            ui.label("Already exported, or saved somewhere else?");
                        } else {
                        ui.label(RichText::new("1. Open your browser's password manager").strong());
                        let open = ui.button(format!("Open {} password manager", self.browser_kind.label()));
                        self.track("browser-open-manager", "Open browser password manager", "button", &open);
                        if open.clicked() { self.open_browser_password_manager(); }
                        ui.label(RichText::new("2. Export passwords to a CSV file").strong());
                        ui.label(self.browser_kind.export_instructions());
                        ui.label(RichText::new("Your browser may ask for your Windows password or device authentication.").small().color(colors.muted));
                        ui.label(RichText::new("3. Choose the exported file, then review it").strong());
                        }
                        let choose = ui.add_sized([ui.available_width(), 40.0], egui::Button::new("Choose exported CSV..."));
                        self.track("browser-choose-csv", "Choose exported CSV", "button", &choose);
                        if choose.clicked() { self.choose_csv(true); }
                        ui.label(RichText::new("The CSV contains readable passwords. Delete it after importing, including copies in Downloads and the Recycle Bin.").small().color(colors.danger));
                        ui.add_space(8.0);
                        egui::CollapsingHeader::new("Optional: save and fill future logins")
                            .id_salt("browser-connection-setup")
                            .show(ui, |ui| self.browser_setup_controls(ui));
                        }
                        self.message(ui);
                        if self.demo && !self.notice.is_empty() {
                            ui.label(RichText::new(&self.notice).small().color(colors.muted));
                        }
                    });
                ui.add_space(12.0);
                let response = ui.add_sized([88.0, 36.0], egui::Button::new("Close"));
                self.track("browser-close", "Close browser guide", "button", &response);
                close |= response.clicked();
                if focus_new { response.request_focus(); }
            });
        if close {
            self.browser_import_open = false;
            self.brave_import_watch = None;
        }
        if !self.browser_import_open && self.pending_import.is_none() {
            if let Some(id) = self.dialog_focus.take().flatten() {
                ctx.memory_mut(|memory| memory.request_focus(id));
            }
        }
    }

    fn browser_setup_controls(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("1. Prepare the browser connection").strong());
        let label = format!("Prepare {} connection", self.browser_kind.label());
        let setup = ui.add_enabled(cfg!(windows) || self.demo, egui::Button::new(&label));
        self.track("browser-setup-native", &label, "button", &setup);
        if setup.clicked() {
            self.setup_browser_connection();
        }
        ui.add_space(8.0);
        ui.label(RichText::new("2. Add the RustyVault extension").strong());
        if let Some(path) = &self.browser_setup_path {
            ui.label("Use this extension folder:");
            ui.label(path.to_string_lossy().as_ref());
            ui.label(match self.browser_kind {
                BrowserKind::Firefox => "In Firefox, open about:debugging#/runtime/this-firefox, choose Load Temporary Add-on, then select manifest.json in this folder. Repeat after restarting Firefox.",
                BrowserKind::Brave => "In Brave, open brave://extensions, turn on Developer mode, choose Load unpacked, then select this folder.",
                BrowserKind::Chrome => "In Chrome, open chrome://extensions, turn on Developer mode, choose Load unpacked, then select this folder.",
                BrowserKind::Edge => "In Edge, open edge://extensions, turn on Developer mode, choose Load unpacked, then select this folder.",
            });
        } else {
            ui.label(
                "Prepare the connection above to see the extension folder and installation steps.",
            );
        }
        if !cfg!(windows) && !self.demo {
            ui.label("Automatic extension setup currently supports Windows. CSV import is available here.");
        }
        ui.add_space(8.0);
        ui.label(RichText::new("3. Allow saves and fills").strong());
        self.browser_connection_controls(ui);
    }

    fn clear_clipboard(&mut self) {
        if let Some((text, _)) = self.clipboard.take() {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                if cb.get_text().is_ok_and(|current| current == *text) {
                    let _ = cb.clear();
                }
            }
        }
    }
    fn copy(&mut self, text: &str) {
        match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.to_owned())) {
            Ok(()) => {
                self.clipboard = Some((Zeroizing::new(text.to_owned()), Instant::now()));
                self.notice = format!(
                    "Copied · clears in {} seconds",
                    self.settings.clipboard_seconds
                );
            }
            Err(e) => self.error = Some(format!("Clipboard unavailable: {e}")),
        }
    }
    fn lock(&mut self, ctx: &egui::Context) {
        self.browser = None;
        self.browser_failed = false;
        self.browser_last_seen = None;
        self.browser_import_open = false;
        self.browser_setup_only = false;
        self.browser_import_pending = false;
        self.brave_import_watch = None;
        self.browser_setup_path = None;
        ctx.memory_mut(|m| m.data.clear());
        self.clear_clipboard();
        self.vault = None;
        self.key = None;
        self.vault_lock = None;
        self.editing = None;
        self.pending_import = None;
        self.pending_import_indices = None;
        self.confirmation = None;
        self.dialog_focus = None;
        self.master.zeroize();
        self.confirm_master.zeroize();
        self.new_master.zeroize();
        self.new_confirm.zeroize();
        self.generated.zeroize();
        self.reveal = false;
        self.query.clear();
        self.notice = "Vault locked".into();
        self.error = None;
    }
    fn unlock(&mut self) {
        if self.demo {
            let mut demo = App::new(true, None);
            self.vault = demo.vault.take();
            self.selected = demo.selected.take();
            self.master.zeroize();
            self.analyze();
            return;
        }
        if self.create && self.path.exists() {
            self.error = Some("A vault already exists at this path. Open it instead.".into());
            return;
        }
        let guard = (|| -> std::io::Result<std::fs::File> {
            if let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)?;
            }
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(self.path.with_extension("enc.lock"))?;
            file.try_lock().map_err(std::io::Error::other)?;
            Ok(file)
        })();
        let guard = match guard {
            Ok(guard) => guard,
            Err(_) => {
                self.error=Some("Could not lock this vault. Close other RustyVault windows and check folder permissions.".into());
                return;
            }
        };
        if self.create && self.path.exists() {
            self.error = Some("A vault already exists at this path. Open it instead.".into());
            return;
        }
        let result = if self.create {
            if self.master != self.confirm_master {
                self.error = Some("Master passwords do not match".into());
                return;
            }
            if self.master.chars().count() < 12 {
                self.error = Some("Use at least 12 characters for the master password".into());
                return;
            }
            let vault = Vault::new();
            crypto::derive_key(&self.master, &vault.salt)
                .map_err(VaultError::from)
                .and_then(|key| {
                    vault.save_to_file(&self.path, &key)?;
                    Ok((vault, key))
                })
        } else {
            Vault::load_from_file(&self.path, &self.master)
        };
        match result {
            Ok((vault, key)) => {
                self.vault_lock = Some(guard);
                self.settings = vault.settings.clone();
                self.selected = vault
                    .entries
                    .iter()
                    .find(|e| e.deleted_at.is_none())
                    .map(|e| e.id.clone());
                self.vault = Some(vault);
                self.key = Some(key);
                self.create = false;
                self.error = None;
                self.master.zeroize();
                self.confirm_master.zeroize();
                self.analyze();
                self.last_input = Instant::now();
                self.notice = "All changes are saved locally".into();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    fn message(&self, ui: &mut egui::Ui) {
        let colors = Palette::get(ui.ctx());
        if let Some(error) = &self.error {
            ui.label(RichText::new(error).color(colors.danger));
        }
    }

    fn locked(&mut self, ui: &mut egui::Ui) {
        let colors = Palette::get(ui.ctx());
        ui.add_space((ui.available_height() / 2.0 - 310.0).max(16.0));
        ui.vertical_centered(|ui| {
            ui.add(
                egui::Image::from_bytes(
                    "bytes://key.svg",
                    include_bytes!("../assets/icons/key-round.svg"),
                )
                .fit_to_exact_size(Vec2::splat(32.0))
                .tint(colors.brand),
            );
            ui.add_space(8.0);
            ui.label(RichText::new("RustyVault").font(display(36.0)));
            ui.label(RichText::new("Your passwords. On your computer.").color(colors.muted));
            ui.add_space(16.0);
            egui::Frame::none()
                .fill(colors.surface)
                .rounding(ui.visuals().window_rounding)
                .stroke(egui::Stroke::new(1.0_f32, colors.border))
                .inner_margin(24.0)
                .show(ui, |ui| {
                    ui.set_width(336.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(if self.create {
                                "Create your vault"
                            } else {
                                "Unlock your vault"
                            })
                            .font(display(24.0)),
                        );
                        ui.add_space(8.0);
                        ui.label(if self.create {
                            "Choose a master passphrase of 12 characters or more."
                        } else {
                            "An encrypted vault already exists. Enter its master password, or create a new vault below."
                        });
                        ui.add_space(16.0);
                        let master_label=ui.label("Master password");
                        let r = ui.scope(|ui| {
                            ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, colors.control_border);
                            ui.add(
                            egui::TextEdit::singleline(&mut self.master)
                                .margin(INPUT_PADDING)
                                .password(true)
                                .vertical_align(egui::Align::Center)
                                .desired_width(f32::INFINITY).min_size(Vec2::new(0.0,44.0)),
                        ).labelled_by(master_label.id)
                        }).inner;
                        self.track("master", "Master password", "textbox", &r);
                        if self.create {
                            form_field(ui,"Confirm master password",&mut self.confirm_master,true);
                        }
                        self.message(ui);
                        ui.add_space(8.0);
                        let r = ui.add_sized(
                            [336.0, 40.0],
                            primary(ui, if self.create {
                                "Create vault"
                            } else {
                                "Unlock vault"
                            }),
                        );
                        self.track("unlock", "Unlock vault", "button", &r);
                        if r.clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            self.unlock();
                        }
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(
                                "There is no password recovery. Keep your master password safe.",
                            )
                            .small()
                            .color(colors.muted),
                        );
                    });
                });
            ui.add_space(16.0);
            if ui.add_enabled(!self.demo, egui::Button::new("Open another vault…")).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Encrypted vault", &["enc", "bak"])
                    .pick_file()
                {
                    self.path = path;
                    self.create = false;
                    self.master.zeroize();
                    self.confirm_master.zeroize();
                    self.error = None;
                }
            }
            if ui.add_enabled(!self.demo, egui::Button::new("Create a new vault…")).clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("Encrypted vault", &["enc"]).set_file_name("vault.enc").save_file() {
                    if path.exists() {
                        self.error = Some("Choose a new filename. Existing vaults are never replaced when creating.".into());
                    } else {
                        self.path = path;
                        self.create = true;
                        self.master.zeroize();
                        self.confirm_master.zeroize();
                        self.error = None;
                    }
                }
            }
            ui.label(
                RichText::new(if self.demo {
                    "Demo · synthetic data · no files are changed"
                } else {
                    "AES-256-GCM encryption · Argon2id key derivation"
                })
                .small()
                .color(colors.muted),
            );
        });
    }

    fn nav(&mut self, ui: &mut egui::Ui, label: &str, view: View, count: Option<usize>) {
        let colors = Palette::get(ui.ctx());
        let (name, bytes): (&str, &[u8]) = match &view {
            View::All => ("layers", include_bytes!("../assets/icons/layers.svg")),
            View::Favorites => ("star", include_bytes!("../assets/icons/star.svg")),
            View::Trash => ("trash", include_bytes!("../assets/icons/trash-2.svg")),
            View::Health => ("health", include_bytes!("../assets/icons/shield-check.svg")),
            View::Generator => ("generator", include_bytes!("../assets/icons/key-round.svg")),
            View::Settings => ("settings", include_bytes!("../assets/icons/settings-2.svg")),
            View::Group(_) => ("folder", include_bytes!("../assets/icons/folder.svg")),
            View::Tag(_) => ("tag", include_bytes!("../assets/icons/tag.svg")),
        };
        let image = egui::Image::from_bytes(format!("bytes://nav-{name}.svg"), bytes)
            .fit_to_exact_size(Vec2::splat(16.0))
            .tint(if self.view == view {
                colors.foreground
            } else {
                colors.muted
            });
        let response = ui
            .add_enabled(
                self.editing.is_none(),
                egui::Button::image_and_text(
                    image,
                    RichText::new(label).color(if self.view == view {
                        colors.foreground
                    } else {
                        colors.secondary
                    }),
                )
                .shortcut_text(count.map(|value| value.to_string()).unwrap_or_default())
                .selected(self.view == view)
                .rounding(CONTROL_RADIUS)
                .fill(if self.view == view {
                    colors.warm
                } else {
                    egui::Color32::TRANSPARENT
                })
                .stroke(egui::Stroke::NONE)
                .truncate()
                .min_size(Vec2::new(ui.available_width(), 36.0)),
            )
            .on_hover_text(label);
        if ui.clip_rect().contains_rect(response.rect) {
            self.track(&format!("nav-{label}"), label, "button", &response);
        }
        if response.clicked() {
            self.view = view;
            self.reveal = false;
        }
    }
    fn sidebar(&mut self, ctx: &egui::Context) {
        let colors = Palette::get(ctx);
        let compact = ctx.screen_rect().height() < 750.0;
        let entries = &self.vault.as_ref().unwrap().entries;
        let mut groups = self.vault.as_ref().unwrap().groups.clone();
        for e in entries.iter().filter(|e| e.deleted_at.is_none()) {
            if !groups.contains(&e.category) {
                groups.push(e.category.clone());
            }
        }
        groups.sort();
        let tags: std::collections::BTreeSet<_> = entries
            .iter()
            .filter(|e| e.deleted_at.is_none())
            .flat_map(|e| e.tags.clone())
            .collect();
        let count = entries.iter().filter(|e| e.deleted_at.is_none()).count();
        let favorite_count = entries
            .iter()
            .filter(|e| e.favorite && e.deleted_at.is_none())
            .count();
        let trash_count = entries.iter().filter(|e| e.deleted_at.is_some()).count();
        let r = egui::SidePanel::left("navigation")
            .exact_width(192.0)
            .resizable(false)
            .frame(egui::Frame::none().fill(colors.surface).inner_margin(16.0))
            .show(ctx, |ui| {
                if self.confirmation.is_some()
                    || self.pending_import.is_some()
                    || self.browser_import_open
                {
                    ui.disable();
                }
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Image::from_bytes(
                            "bytes://key.svg",
                            include_bytes!("../assets/icons/key-round.svg"),
                        )
                        .fit_to_exact_size(Vec2::splat(20.0))
                        .tint(colors.brand),
                    );
                    ui.label(RichText::new("RustyVault").font(display(20.0)));
                });
                ui.add_space(if compact { 8.0 } else { 16.0 });
                self.password_actions(ui);
                ui.add_space(if compact { 8.0 } else { 16.0 });
                ui.label(RichText::new("My vault").small().color(colors.muted));
                ui.add_space(4.0);
                self.nav(ui, "All entries", View::All, Some(count));
                self.nav(ui, "Favorites", View::Favorites, Some(favorite_count));
                self.nav(ui, "Trash", View::Trash, Some(trash_count));
                ui.add_space(if compact { 8.0 } else { 16.0 });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Groups").small().color(colors.muted));
                    ui.menu_button("Add…", |ui| {
                        form_field(ui, "Group name", &mut self.group_name, false);
                        if ui.button("Create group").clicked() && !self.group_name.trim().is_empty()
                        {
                            let mut next = self.vault.as_ref().unwrap().clone();
                            let name = self.group_name.trim().to_owned();
                            if !next.groups.contains(&name) {
                                next.groups.push(name);
                            }
                            if self.commit(next, "Group created") {
                                self.group_name.clear();
                                ui.close_menu();
                            }
                        }
                    });
                });
                let footer_rect = egui::Rect::from_min_max(
                    egui::pos2(ui.max_rect().left(), ui.max_rect().bottom() - 196.0),
                    ui.max_rect().max,
                );
                egui::ScrollArea::vertical()
                    .min_scrolled_height(0.0)
                    .max_height((footer_rect.top() - ui.cursor().top() - 8.0).max(0.0))
                    .show(ui, |ui| {
                        for group in &groups {
                            self.nav(ui, group, View::Group(group.clone()), None);
                        }

                        if !tags.is_empty() {
                            ui.add_space(16.0);
                            ui.label(RichText::new("Tags").small().color(colors.muted));
                            for tag in tags {
                                self.nav(ui, &tag, View::Tag(tag.clone()), None);
                            }
                        }
                    });
                let mut footer = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("nav-footer")
                        .max_rect(footer_rect),
                );
                footer.separator();
                footer.add_space(8.0);
                self.nav(&mut footer, "Password health", View::Health, None);
                self.nav(&mut footer, "Generator", View::Generator, None);
                self.nav(&mut footer, "Settings", View::Settings, None);
                if footer
                    .add_sized(
                        [footer.available_width(), 36.0],
                        egui::Button::new("Lock vault").shortcut_text("Ctrl+L"),
                    )
                    .clicked()
                {
                    self.lock(ctx);
                }
            });
        self.track("sidebar", "Navigation", "navigation", &r.response);
    }

    fn entries(&mut self, ctx: &egui::Context) {
        let colors = Palette::get(ctx);
        let mut entries: Vec<EntryRow> = self
            .vault
            .as_ref()
            .unwrap()
            .search(&self.query)
            .into_iter()
            .filter(|e| match &self.view {
                View::Trash => e.deleted_at.is_some(),
                View::Favorites => e.deleted_at.is_none() && e.favorite,
                View::Group(g) => {
                    e.deleted_at.is_none()
                        && (e.category == *g || e.category.starts_with(&format!("{g}/")))
                }
                View::Tag(t) => e.deleted_at.is_none() && e.tags.contains(t),
                _ => e.deleted_at.is_none(),
            })
            .map(|e| EntryRow {
                id: e.id.clone(),
                service: e.service.clone(),
                subtitle: if e.kind == "Secure note" {
                    "Secure note".into()
                } else {
                    e.username_or_email().into()
                },
                favorite: e.favorite,
            })
            .collect();
        entries.sort_by_key(|e| e.service.to_lowercase());
        if self.editing.is_none()
            && !entries
                .iter()
                .any(|e| self.selected.as_ref() == Some(&e.id))
        {
            self.selected = entries.first().map(|e| e.id.clone());
        }
        let width = if ctx.screen_rect().width() < 1000.0 {
            256.0
        } else {
            304.0
        };
        let r = egui::SidePanel::left("entry-list")
            .exact_width(width)
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(colors.background)
                    .inner_margin(16.0),
            )
            .show(ctx, |ui| {
                if self.confirmation.is_some()
                    || self.pending_import.is_some()
                    || self.browser_import_open
                {
                    ui.disable();
                }
                ui.label(RichText::new(self.view.title()).font(display(20.0)));
                ui.add_space(16.0);
                let r = ui
                    .scope(|ui| {
                        ui.visuals_mut().widgets.inactive.bg_stroke =
                            egui::Stroke::new(1.0_f32, colors.control_border);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.query)
                                .margin(INPUT_PADDING)
                                .id(egui::Id::new("search"))
                                .min_size(Vec2::new(0.0, 44.0))
                                .vertical_align(egui::Align::Center)
                                .hint_text("Search entries")
                                .desired_width(f32::INFINITY),
                        )
                    })
                    .inner;
                self.track("search", "Search your vault", "textbox", &r);
                ui.label(
                    RichText::new(format!("{} entries", entries.len()))
                        .small()
                        .color(colors.muted),
                );
                ui.add_space(8.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if entries.is_empty() {
                        ui.add_space(24.0);
                        ui.label(if self.query.is_empty() {
                            "No entries here yet"
                        } else {
                            "No matching entries"
                        });
                        if !self.query.is_empty() && ui.button("Clear search").clicked() {
                            self.query.clear();
                        }
                    }
                    for (index, entry) in entries.iter().enumerate() {
                        let selected = self.selected.as_ref() == Some(&entry.id);
                        let response =
                            egui::Frame::none()
                                .fill(if selected {
                                    colors.warm
                                } else {
                                    colors.background
                                })
                                .stroke(egui::Stroke::NONE)
                                .rounding(ui.visuals().widgets.inactive.rounding)
                                .inner_margin(8.0)
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        egui::Frame::none()
                                            .fill(colors.border)
                                            .rounding(ui.visuals().widgets.inactive.rounding)
                                            .inner_margin(8.0)
                                            .show(ui, |ui| {
                                                ui.add_sized(
                                                    [20.0, 20.0],
                                                    egui::Label::new(
                                                        RichText::new(
                                                            entry
                                                                .service
                                                                .chars()
                                                                .next()
                                                                .unwrap_or('?')
                                                                .to_uppercase()
                                                                .to_string(),
                                                        )
                                                        .size(16.0)
                                                        .strong(),
                                                    ),
                                                );
                                            });
                                        ui.vertical(|ui| {
                                            ui.spacing_mut().item_spacing.y = 2.0;
                                            ui.set_max_width(
                                                (ui.available_width()
                                                    - if entry.favorite { 24.0 } else { 0.0 })
                                                .max(32.0),
                                            );
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(entry.service.clone()).strong(),
                                                )
                                                .truncate(),
                                            )
                                            .on_hover_text(format!(
                                                "{}{}",
                                                entry.service,
                                                if entry.favorite { " · Favorite" } else { "" }
                                            ));
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(&entry.subtitle).small().color(
                                                        if selected {
                                                            colors.secondary
                                                        } else {
                                                            colors.muted
                                                        },
                                                    ),
                                                )
                                                .truncate(),
                                            )
                                            .on_hover_text(&entry.subtitle);
                                        });
                                        if entry.favorite {
                                            ui.add(
                                                egui::Image::from_bytes(
                                                    "bytes://favorite.svg",
                                                    include_bytes!("../assets/icons/star.svg"),
                                                )
                                                .fit_to_exact_size(Vec2::splat(14.0))
                                                .tint(colors.muted),
                                            );
                                        }
                                    });
                                })
                                .response;
                        let r = ui.interact(
                            response.rect,
                            ui.id().with(&entry.id),
                            if self.editing.is_none() {
                                egui::Sense::click()
                            } else {
                                egui::Sense::hover()
                            },
                        );
                        r.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::SelectableLabel,
                                ui.is_enabled() && self.editing.is_none(),
                                selected,
                                &entry.service,
                            )
                        });
                        if r.hovered() && !selected {
                            ui.painter().rect_stroke(
                                r.rect,
                                CONTROL_RADIUS,
                                egui::Stroke::new(1.0_f32, colors.border),
                            );
                        }
                        if r.has_focus() {
                            ui.painter().rect_stroke(
                                r.rect,
                                10.0,
                                egui::Stroke::new(2.0_f32, colors.accent),
                            );
                        }
                        self.track(&format!("entry-{index}"), &entry.service, "button", &r);
                        if r.clicked() {
                            if self.editing.is_some() {
                                self.confirmation = Some(Confirm::Discard);
                            } else {
                                self.selected = Some(entry.id.clone());
                                self.reveal = false;
                                self.error = None;
                            }
                        }
                        if r.double_clicked() && self.view != View::Trash && self.editing.is_none()
                        {
                            if let Some(e) = self
                                .vault
                                .as_ref()
                                .unwrap()
                                .entries
                                .iter()
                                .find(|e| e.id == entry.id)
                                .cloned()
                            {
                                self.edit(e);
                            }
                        }
                    }
                });
            });
        self.track("entry-list", "Entries", "region", &r.response);
    }
}
impl App {
    fn detail(&mut self, ui: &mut egui::Ui) {
        let colors = Palette::get(ui.ctx());
        let Some(entry) = self.selected_entry() else {
            ui.add_space(64.0);
            let empty = !self
                .vault
                .as_ref()
                .unwrap()
                .entries
                .iter()
                .any(|entry| entry.deleted_at.is_none());
            ui.heading(if empty {
                "Your passwords, in one place"
            } else {
                "No entry selected"
            });
            ui.label(
                RichText::new(if empty {
                    "Add your first password or bring your saved passwords from a browser."
                } else {
                    "Choose an entry, add a password, or import from your browser."
                })
                .color(colors.muted),
            );
            ui.add_space(20.0);
            ui.add_enabled_ui(self.view != View::Trash, |ui| {
                let add = ui.add_sized(
                    [ui.available_width().min(360.0), 44.0],
                    primary(ui, "Add password"),
                );
                self.track("empty-add-password", "Add password", "button", &add);
                if add.clicked() {
                    self.add_password();
                }
                let import = ui.add_sized(
                    [ui.available_width().min(360.0), 40.0],
                    egui::Button::new("Import from browser…"),
                );
                self.track(
                    "empty-import-browser",
                    "Import from browser",
                    "button",
                    &import,
                );
                if import.clicked() {
                    self.open_browser_guide(false);
                }
            });
            return;
        };
        ui.horizontal(|ui| {
            egui::Frame::none()
                .fill(colors.warm)
                .rounding(12.0)
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.add_sized(
                        [28.0, 28.0],
                        egui::Label::new(
                            RichText::new(
                                entry
                                    .service
                                    .chars()
                                    .next()
                                    .unwrap_or('?')
                                    .to_uppercase()
                                    .to_string(),
                            )
                            .size(24.0),
                        ),
                    );
                });
            ui.add_space(4.0);
            ui.vertical(|ui| {
                let title = ui
                    .add(
                        egui::Label::new(RichText::new(&entry.service).font(display(30.0)))
                            .truncate(),
                    )
                    .on_hover_text(&entry.service);
                self.track("detail-title", &entry.service, "heading", &title);
                ui.label(
                    RichText::new(format!(
                        "{} · {}",
                        if entry.kind.is_empty() {
                            "Login"
                        } else {
                            &entry.kind
                        },
                        entry.category
                    ))
                    .small()
                    .color(colors.muted),
                );
            });
        });
        ui.add_space(24.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Account details").color(colors.secondary));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if entry.deleted_at.is_some() {
                    if ui.button("Delete forever…").clicked() {
                        self.confirmation = Some(Confirm::Purge(entry.id.clone()));
                    }
                    if ui.button("Restore").clicked() {
                        let mut next = self.vault.as_ref().unwrap().clone();
                        next.entries
                            .iter_mut()
                            .find(|e| e.id == entry.id)
                            .unwrap()
                            .deleted_at = None;
                        self.commit(next, "Entry restored");
                    }
                } else {
                    ui.menu_button("More", |ui| {
                        ui.menu_button("Copy", |ui| {
                            for (label, value) in [
                                ("Username", entry.username_or_email()),
                                ("Website", entry.url.as_str()),
                            ] {
                                if ui.button(label).clicked() {
                                    self.copy(value);
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.menu_button("View", |ui| {
                            if ui
                                .button(if self.reveal {
                                    "Hide password"
                                } else {
                                    "Reveal password"
                                })
                                .clicked()
                            {
                                self.reveal = !self.reveal;
                                ui.close_menu();
                            }
                            if (entry.url.starts_with("http://")
                                || entry.url.starts_with("https://"))
                                && ui.button("Open website").clicked()
                            {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(&entry.url));
                                ui.close_menu();
                            }
                            if !entry.attachments.is_empty() {
                                ui.menu_button("Save attachment", |ui| {
                                    for attachment in &entry.attachments {
                                        if ui
                                            .add_enabled(
                                                !self.demo,
                                                egui::Button::new(&attachment.name),
                                            )
                                            .clicked()
                                        {
                                            if let Some(path) = rfd::FileDialog::new()
                                                .set_file_name(&attachment.name)
                                                .save_file()
                                            {
                                                match self.check_export_path(&path).and_then(|_| {
                                                    std::fs::write(path, &attachment.bytes)
                                                }) {
                                                    Ok(()) => {
                                                        self.notice = "Attachment saved".into()
                                                    }
                                                    Err(e) => self.error = Some(e.to_string()),
                                                }
                                            }
                                            ui.close_menu();
                                        }
                                    }
                                });
                            }
                        });
                        ui.menu_button("Manage", |ui| {
                            if ui
                                .button(if entry.favorite {
                                    "Remove favorite"
                                } else {
                                    "Add favorite"
                                })
                                .clicked()
                            {
                                let mut next = self.vault.as_ref().unwrap().clone();
                                next.entries
                                    .iter_mut()
                                    .find(|e| e.id == entry.id)
                                    .unwrap()
                                    .favorite = !entry.favorite;
                                self.commit(next, "Favorite updated");
                                ui.close_menu();
                            }
                            if ui.button("Duplicate").clicked() {
                                let mut copy = entry.clone();
                                copy.id = format!("{:x}", rand::random::<u128>());
                                copy.service.push_str(" (copy)");
                                copy.history.clear();
                                copy.created_at = chrono::Utc::now();
                                copy.updated_at = copy.created_at;
                                self.edit(copy);
                                ui.close_menu();
                            }
                            if ui.button("Move to trash…").clicked() {
                                self.confirmation = Some(Confirm::Trash(entry.id.clone()));
                                ui.close_menu();
                            }
                        });
                    });
                    if ui.button("Edit").clicked() {
                        self.edit(entry.clone());
                    }
                    let label = if entry.password.is_empty() {
                        "Copy note"
                    } else {
                        "Copy password"
                    };
                    let response = ui.add(primary(ui, label));
                    self.track("copy-entry", label, "button", &response);
                    if response.clicked() {
                        self.copy(if entry.password.is_empty() {
                            &entry.notes
                        } else {
                            &entry.password
                        });
                    }
                }
            });
        });
        ui.add_space(12.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Frame::none()
                .fill(colors.surface)
                .rounding(12.0)
                .inner_margin(20.0)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 4.0;
                    if !entry.username.is_empty() {
                        self.detail_field(ui, "Username", &entry.username, false);
                    }
                    if !entry.email.is_empty() {
                        self.detail_field(ui, "Email", &entry.email, false);
                    }
                    if !entry.password.is_empty() {
                        self.detail_field(
                            ui,
                            "Password",
                            if self.reveal {
                                &entry.password
                            } else {
                                "••••••••••••••••"
                            },
                            true,
                        );
                        ui.label(
                            RichText::new(if self.weak.contains(&entry.id) {
                                "Weak password · consider replacing it"
                            } else {
                                "Strong password"
                            })
                            .small()
                            .color(if self.weak.contains(&entry.id) {
                                colors.danger
                            } else {
                                colors.success
                            }),
                        );
                        ui.add_space(8.0);
                    }
                    if !entry.url.is_empty() {
                        self.detail_field(ui, "Website", &entry.url, false);
                    }
                });
            ui.add_space(20.0);
            if !entry.tags.is_empty() {
                ui.label(RichText::new("Tags").color(colors.muted));
                ui.label(entry.tags.join(" · "));
                ui.add_space(16.0);
            }
            if !entry.notes.is_empty() {
                ui.label(RichText::new("Notes").color(colors.muted));
                ui.label(&entry.notes);
                ui.add_space(16.0);
            }
            if !entry.fields.is_empty() {
                egui::CollapsingHeader::new("Custom fields").show(ui, |ui| {
                    for (name, value) in &entry.fields {
                        ui.horizontal(|ui| {
                            ui.label(name);
                            ui.label("••••••••");
                            if ui.button("Copy field").clicked() {
                                self.copy(value);
                            }
                        });
                    }
                });
            }
            if !entry.attachments.is_empty() {
                ui.label(RichText::new("Attachments").color(colors.muted));
                for file in &entry.attachments {
                    ui.label(format!("{} · {} bytes", file.name, file.bytes.len()));
                }
                ui.add_space(16.0);
            }
            ui.separator();
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("Created {}", entry.created_at.format("%d %b %Y")))
                    .small()
                    .color(colors.muted),
            );
            ui.label(
                RichText::new(format!(
                    "Updated {}",
                    entry.updated_at.format("%d %b %Y · %H:%M")
                ))
                .small()
                .color(colors.muted),
            );
            if let Some(expiry) = entry.expires_at {
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        if entry.is_expired() {
                            "Expired"
                        } else {
                            "Expires"
                        },
                        expiry.format("%d %b %Y")
                    ))
                    .color(colors.muted),
                );
            }
            if !entry.history.is_empty() {
                ui.add_space(16.0);
                egui::CollapsingHeader::new(format!(
                    "Credential history · {} versions",
                    entry.history.len()
                ))
                .show(ui, |ui| {
                    for (i, revision) in entry.history.iter().enumerate().rev() {
                        egui::CollapsingHeader::new(
                            revision.saved_at.format("%d %b %Y · %H:%M:%S").to_string(),
                        )
                        .id_salt(i)
                        .show(ui, |ui| {
                            ui.label(&revision.service);
                            ui.label(&revision.username);
                            ui.label(&revision.email);
                            ui.label(&revision.url);
                            ui.label(&revision.notes);
                            if ui.button("Copy previous password").clicked() {
                                self.copy(&revision.password);
                            }
                            ui.label(format!("{} custom fields", revision.fields.len()));
                            if ui.button("Restore this version…").clicked() {
                                self.confirmation =
                                    Some(Confirm::RestoreHistory(entry.id.clone(), i));
                            }
                        });
                    }
                });
            }
        });
    }
    fn detail_field(&mut self, ui: &mut egui::Ui, label: &str, value: &str, secret: bool) {
        let colors = Palette::get(ui.ctx());
        let response = ui
            .horizontal(|ui| {
                let action_width = if secret { 0.0 } else { 112.0 };
                let value_width = (ui.available_width() - action_width - 8.0).max(48.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(value_width, 44.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_min_width(value_width);
                        ui.spacing_mut().item_spacing.y = 4.0;
                        ui.label(RichText::new(label).small().color(colors.muted));
                        let text = if secret {
                            RichText::new(value).monospace().size(17.0)
                        } else {
                            RichText::new(value).size(16.0)
                        };
                        let value_label = ui.add(egui::Label::new(text).truncate());
                        if !secret {
                            value_label.on_hover_text(value);
                        }
                    },
                );
                if !secret {
                    let copy_label = format!("Copy {}", label.to_lowercase());
                    let copy =
                        ui.add_sized([104.0, 32.0], egui::Button::new(&copy_label).frame(false));
                    if ui.clip_rect().contains_rect(copy.rect) {
                        self.track(&format!("copy-{label}"), &copy_label, "button", &copy);
                    }
                    if copy.clicked() {
                        self.copy(value);
                    }
                }
            })
            .response;
        self.track(&format!("field-{label}"), label, "group", &response);
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);
    }
    fn editor(&mut self, ui: &mut egui::Ui) {
        let colors = Palette::get(ui.ctx());
        let mut entry = self.editing.take().unwrap();
        let is_new = !self
            .vault
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .any(|e| e.id == entry.id);
        let note = entry.kind == "Secure note";
        ui.heading(if note {
            "Secure note"
        } else if is_new {
            "Add password"
        } else {
            "Edit password"
        });
        ui.label(
            RichText::new("Save when you are ready. Optional details can wait.")
                .color(colors.muted),
        );
        ui.add_space(12.0);
        let mut save = false;
        ui.horizontal(|ui| {
            let label = if note { "Save note" } else { "Save password" };
            let response = ui.add_sized([144.0, 40.0], primary(ui, label));
            self.track("save-entry", label, "button", &response);
            save = response.clicked();
            let cancel = ui.add_sized([88.0, 40.0], egui::Button::new("Cancel"));
            self.track("cancel-entry", "Cancel", "button", &cancel);
            if cancel.clicked() {
                self.confirmation = Some(Confirm::Discard);
            }
        });
        self.message(ui);
        ui.add_space(12.0);
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("editor-scroll")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let label = ui.label("Type");
                    egui::ComboBox::from_id_salt("kind")
                        .selected_text(if entry.kind.is_empty() || entry.kind == "Login" {
                            "Password"
                        } else {
                            &entry.kind
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut entry.kind, "Login".into(), "Password");
                            ui.selectable_value(
                                &mut entry.kind,
                                "Secure note".into(),
                                "Secure note",
                            );
                        })
                        .response
                        .labelled_by(label.id);
                });
                form_field(
                    ui,
                    if entry.kind == "Secure note" {
                        "Name"
                    } else {
                        "Website or name"
                    },
                    &mut entry.service,
                    false,
                );
                if entry.kind != "Secure note" {
                    ui.label(
                        RichText::new("Use example.com or an account name.")
                            .small()
                            .color(colors.muted),
                    );
                    if self.editor_uses_email {
                        form_field(ui, "Username or email", &mut entry.email, false);
                    } else {
                        form_field(ui, "Username or email", &mut entry.username, false);
                    }
                    form_field(ui, "Password", &mut entry.password, !self.reveal);
                    ui.horizontal(|ui| {
                        checkbox(ui, &mut self.reveal, "Show password");
                        if ui.button("Generate password").clicked() {
                            entry.password = generate_password(&self.config);
                        }
                    });
                } else {
                    let label = ui.label("Note");
                    ui.add(
                        egui::TextEdit::multiline(&mut entry.notes)
                            .margin(INPUT_PADDING)
                            .desired_rows(6)
                            .desired_width(f32::INFINITY),
                    )
                    .labelled_by(label.id);
                }
                ui.add_space(8.0);
                let separate_website = entry.kind != "Secure note"
                    && !entry.service.trim().is_empty()
                    && inferred_website(&entry.service).is_none();
                if separate_website {
                    form_field(
                        ui,
                        "Website for browser filling (optional)",
                        &mut entry.url,
                        false,
                    );
                    let help = ui.label(
                        RichText::new("Use github.com, or leave blank for a local account.")
                            .small()
                            .color(colors.muted),
                    );
                    self.track(
                        "autofill-website-help",
                        "Website needed for browser filling",
                        "text",
                        &help,
                    );
                    ui.add_space(8.0);
                }
                egui::CollapsingHeader::new("Optional details")
                    .id_salt((&entry.id, "optional-details"))
                    .show(ui, |ui| {
                        if !separate_website {
                            form_field(ui, "Website URL (optional)", &mut entry.url, false);
                        }
                        if self.editor_uses_email {
                            form_field(
                                ui,
                                "Separate username (optional)",
                                &mut entry.username,
                                false,
                            );
                        } else {
                            form_field(ui, "Separate email (optional)", &mut entry.email, false);
                        }
                        form_field(ui, "Group", &mut entry.category, false);
                        form_field(ui, "Tags (comma separated)", &mut self.tags_input, false);
                        form_field(
                            ui,
                            "Expiry (YYYY-MM-DD, optional)",
                            &mut self.expiry_input,
                            false,
                        );
                        if entry.kind != "Secure note" {
                            let label = ui.label("Notes");
                            ui.add(
                                egui::TextEdit::multiline(&mut entry.notes)
                                    .margin(INPUT_PADDING)
                                    .desired_rows(4)
                                    .desired_width(f32::INFINITY),
                            )
                            .labelled_by(label.id);
                        }
                        checkbox(ui, &mut entry.favorite, "Add to favorites");
                        egui::CollapsingHeader::new("Custom fields").show(ui, |ui| {
                            let mut remove = None;
                            for (index, (name, value)) in entry.fields.iter_mut().enumerate() {
                                ui.push_id(index, |ui| {
                                    ui.columns(2, |columns| {
                                        form_field(&mut columns[0], "Field name", name, false);
                                        form_field(&mut columns[1], "Field value", value, true);
                                    });
                                    if ui.button("Remove field").clicked() {
                                        remove = Some(index);
                                    }
                                });
                            }
                            if let Some(index) = remove {
                                entry.fields.remove(index);
                            }
                            if ui.button("Add field").clicked() {
                                entry.fields.push((String::new(), String::new()));
                            }
                        });
                        egui::CollapsingHeader::new("Attachments (up to 5 MB each)").show(
                            ui,
                            |ui| {
                                let mut remove = None;
                                for (index, attachment) in entry.attachments.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        ui.label(&attachment.name);
                                        if ui.button("Remove").clicked() {
                                            remove = Some(index);
                                        }
                                    });
                                }
                                if let Some(index) = remove {
                                    entry.attachments.remove(index);
                                }
                                if ui
                                    .add_enabled(!self.demo, egui::Button::new("Attach file..."))
                                    .clicked()
                                {
                                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                                        match read_small_file(&path, 5 * 1024 * 1024) {
                                            Ok(bytes) => entry.attachments.push(Attachment {
                                                name: path
                                                    .file_name()
                                                    .unwrap_or_default()
                                                    .to_string_lossy()
                                                    .into_owned(),
                                                bytes,
                                            }),
                                            Err(error) => self.error = Some(error.to_string()),
                                        }
                                    }
                                }
                            },
                        );
                    });
            });
        let mut saved = false;
        if save {
            if entry.kind != "Secure note" && !entry.url.is_empty() && !entry.url.contains("://") {
                if let Some(website) = inferred_website(&entry.url) {
                    entry.url = website;
                }
            }
            if is_new && entry.kind != "Secure note" && entry.url.is_empty() {
                if let Some(website) = inferred_website(&entry.service) {
                    entry.url = website;
                }
            }
            entry.tags = self
                .tags_input
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(String::from)
                .collect();
            let expiry = if self.expiry_input.trim().is_empty() {
                Ok(None)
            } else {
                chrono::NaiveDate::parse_from_str(self.expiry_input.trim(), "%Y-%m-%d")
                    .map(|date| Some(date.and_hms_opt(23, 59, 59).unwrap().and_utc()))
                    .map_err(|_| VaultError::InvalidData("Expiry must use YYYY-MM-DD".into()))
            };
            match expiry.and_then(|date| {
                entry.expires_at = date;
                entry.validate()
            }) {
                Ok(()) => {
                    let mut next = self.vault.as_ref().unwrap().clone();
                    if !next.update_entry(entry.clone()) {
                        next.add_entry(entry.clone());
                    }
                    if self.commit(
                        next,
                        if entry.kind == "Secure note" {
                            "Secure note saved"
                        } else {
                            "Password saved"
                        },
                    ) {
                        self.selected = Some(entry.id.clone());
                        self.reveal = false;
                        saved = true;
                    }
                }
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        if !saved {
            self.editing = Some(entry);
        }
    }

    fn generator(&mut self, ui: &mut egui::Ui) {
        let colors = Palette::get(ui.ctx());
        ui.heading("Password generator");
        ui.label(
            RichText::new("Create a new password or a memorable random passphrase.")
                .color(colors.muted),
        );
        ui.add_space(24.0);
        let mut changed = false;
        ui.horizontal(|ui| {
            changed |= ui
                .selectable_value(&mut self.passphrase, false, "Password")
                .changed();
            changed |= ui
                .selectable_value(&mut self.passphrase, true, "Passphrase")
                .changed();
        });
        ui.add_space(16.0);
        egui::Frame::none()
            .fill(colors.surface)
            .rounding(ui.visuals().window_rounding)
            .inner_margin(20.0)
            .show(ui, |ui| {
                ui.label(
                    RichText::new(&self.generated)
                        .monospace()
                        .size(22.0)
                        .color(colors.accent),
                );
            });
        ui.add_space(16.0);
        if self.passphrase {
            changed |= ui
                .add(egui::Slider::new(&mut self.words, 4..=12).text("Words"))
                .changed();
            let separator_label = ui.label("Separator");
            changed |= ui
                .scope(|ui| {
                    ui.visuals_mut().widgets.inactive.bg_stroke =
                        egui::Stroke::new(1.0_f32, colors.control_border);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.separator)
                            .margin(INPUT_PADDING)
                            .vertical_align(egui::Align::Center)
                            .desired_width(80.0)
                            .min_size(Vec2::new(0.0, 44.0)),
                    )
                })
                .inner
                .labelled_by(separator_label.id)
                .changed();
            changed |= checkbox(ui, &mut self.capitalize, "Capitalize words").changed();
            ui.label(
                RichText::new(format!(
                    "About {:.0} bits from {} random EFF words",
                    self.words as f64 * (7776.0f64).log2(),
                    self.words
                ))
                .small()
                .color(colors.muted),
            );
        } else {
            changed |= ui
                .add(egui::Slider::new(&mut self.config.length, 8..=128).text("Characters"))
                .changed();
            changed |= checkbox(ui, &mut self.config.uppercase, "Uppercase letters").changed();
            changed |= checkbox(ui, &mut self.config.lowercase, "Lowercase letters").changed();
            changed |= checkbox(ui, &mut self.config.numbers, "Numbers").changed();
            changed |= checkbox(ui, &mut self.config.symbols, "Symbols").changed();
            changed |= checkbox(
                ui,
                &mut self.config.exclude_ambiguous,
                "Exclude look-alike characters",
            )
            .changed();
            if !self.config.uppercase
                && !self.config.lowercase
                && !self.config.numbers
                && !self.config.symbols
            {
                self.config.lowercase = true;
            }
        }
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.add(primary(ui, "Generate again")).clicked() || changed {
                self.generated = if self.passphrase {
                    generate_passphrase(self.words, &self.separator, self.capitalize)
                } else {
                    generate_password(&self.config)
                };
            }
            if ui.button("Copy").clicked() {
                let value = self.generated.clone();
                self.copy(&value);
            }
        });
        self.message(ui);
    }
    fn health(&mut self, ui: &mut egui::Ui) {
        let colors = Palette::get(ui.ctx());
        let entries = &self.vault.as_ref().unwrap().entries;
        let mut passwords = std::collections::HashMap::new();
        for e in entries
            .iter()
            .filter(|e| e.deleted_at.is_none() && !e.password.is_empty())
        {
            *passwords.entry(e.password.as_str()).or_insert(0usize) += 1;
        }
        let reused: HashSet<_> = entries
            .iter()
            .filter(|e| {
                passwords
                    .get(e.password.as_str())
                    .is_some_and(|count| *count > 1)
            })
            .map(|e| e.id.clone())
            .collect();
        let active: Vec<_> = entries
            .iter()
            .filter(|e| e.deleted_at.is_none())
            .map(|e| (e.id.clone(), e.service.clone(), e.is_expired()))
            .collect();
        ui.heading("Password health");
        ui.label(
            RichText::new("Checks run on this computer. Your passwords are never sent anywhere.")
                .color(colors.muted),
        );
        ui.add_space(24.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{} weak", self.weak.len())).size(20.0));
            ui.separator();
            ui.label(RichText::new(format!("{} reused", reused.len())).size(20.0));
            ui.separator();
            ui.label(
                RichText::new(format!("{} expired", active.iter().filter(|e| e.2).count()))
                    .size(20.0),
            );
        });
        ui.add_space(24.0);
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut found = false;
            for e in active {
                let mut issues = Vec::new();
                if self.weak.contains(&e.0) {
                    issues.push("Weak password");
                }
                if reused.contains(&e.0) {
                    issues.push("Reused password");
                }
                if e.2 {
                    issues.push("Expired");
                }
                if !issues.is_empty() {
                    found = true;
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&e.1).strong());
                            ui.label(RichText::new(issues.join(" · ")).color(colors.danger));
                        });
                        if ui.button("Review entry").clicked() {
                            self.selected = Some(e.0.clone());
                            self.view = View::All;
                        }
                    });
                    ui.add_space(12.0);
                    ui.separator();
                }
            }
            if !found {
                ui.add_space(24.0);
                ui.label("No weak, reused or expired passwords found");
                ui.label(
                    RichText::new("This is a local estimate, not a breach-database check.")
                        .color(colors.muted),
                );
            }
        });
    }
}
impl App {
    fn browser_requests(&mut self, ctx: &egui::Context) {
        let enabled = !self.demo
            && self
                .vault
                .as_ref()
                .is_some_and(|v| v.settings.browser_enabled);
        if !enabled {
            self.browser = None;
            self.browser_failed = false;
            self.browser_last_seen = None;
            return;
        }
        if self.browser.is_none() && !self.browser_failed {
            let wake = ctx.clone();
            let connection = browser::session_path().and_then(|path| {
                self.check_export_path(&path)?;
                self.check_export_path(&path.with_extension("lock"))?;
                browser::Server::start(move || wake.request_repaint())
            });
            match connection {
                Ok(server) => self.browser = Some(server),
                Err(error) => {
                    self.browser_failed = true;
                    self.error = Some(format!("Browser connection could not start: {error}. Disable and re-enable it to retry."));
                }
            }
        }
        if let Some(incoming) = self.browser.as_ref().and_then(|server| server.try_recv()) {
            self.browser_last_seen = Some(Instant::now());
            let response = if self.editing.is_some()
                || self.confirmation.is_some()
                || self.pending_import.is_some()
                || self.browser_import_open
            {
                BrowserResponse::error(
                    "Finish the current edit or dialog in RustyVault, then try again",
                )
            } else if matches!(&incoming.request, BrowserRequest::Save { .. }) {
                match browser::saved_entry(self.vault.as_ref().unwrap(), &incoming.request) {
                    Ok(entry) => {
                        self.confirmation = Some(Confirm::BrowserSave(Box::new(entry)));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        BrowserResponse::success()
                    }
                    Err(error) => BrowserResponse::error(error),
                }
            } else {
                browser::read_login(self.vault.as_ref().unwrap(), &incoming.request)
            };
            let _ = incoming.reply.send(response);
        }
    }

    fn settings_screen(&mut self, ui: &mut egui::Ui) {
        let colors = Palette::get(ui.ctx());
        ui.heading("Vault settings");
        ui.label(RichText::new("Protection, backups and data portability").color(colors.muted));
        ui.add_space(24.0);
        egui::ScrollArea::vertical().max_width(760.0).show(ui, |ui| {
            settings_section(ui, "Browser connection", |ui| {
                let setup = ui.button("Set up browser extension...");
                self.track("settings-browser-setup", "Set up browser extension", "button", &setup);
                if setup.clicked() { self.open_browser_guide(true); }
                ui.label(RichText::new("Connect your browser for future saves and fills.").small().color(colors.muted));
                ui.add_space(8.0);
                let import = ui.button("Import from browser...");
                self.track("settings-import-browser", "Import from browser", "button", &import);
                if import.clicked() { self.open_browser_guide(false); }
                ui.label(RichText::new("Bring existing passwords into this vault using a browser CSV export.").small().color(colors.muted));
                self.browser_connection_controls(ui);
            });
            settings_section(ui, "Appearance", |ui| {

            ui.horizontal(|ui| {
                for (label, dark) in [("Dark", true), ("Light", false)] {
                    let response = ui.selectable_label(self.dark_mode == dark, label);
                    self.track(&format!("theme-{label}"), label, "button", &response);
                    if response.clicked() {
                        self.change_appearance(ui.ctx(), dark);
                    }
                }
            });
            ui.label(RichText::new(if self.demo { "Demo appearance changes stay in memory." } else { "Applies to this computer, including the lock screen. Saved automatically." }).small().color(colors.muted));
            ui.add_space(8.0);
            ui.label(RichText::new("Raycast · light and dark").small().color(colors.muted));


            });
            settings_section(ui, "Security", |ui| {
ui.add_space(8.0);egui::Grid::new("security-timing").num_columns(2).spacing([24.0, 12.0]).show(ui, |ui| {
                    ui.label("Lock after inactivity");
                    ui.add(egui::DragValue::new(&mut self.settings.lock_seconds).range(60..=3600).suffix(" s"));
                    ui.end_row();
                    ui.label("Clear clipboard after");
                    ui.add(egui::DragValue::new(&mut self.settings.clipboard_seconds).range(5..=120).suffix(" s"));
                    ui.end_row();
                });
                ui.add_space(12.0);checkbox(ui, &mut self.settings.lock_on_focus_loss,"Lock when the window loses focus");if ui.add(primary(ui, "Save security settings")).clicked(){let mut next=self.vault.as_ref().unwrap().clone();next.settings=self.settings.clone();self.commit(next,"Security settings saved");}

            });
            settings_section(ui, "Backups and import", |ui| {
ui.label(RichText::new("The previous encrypted file is kept as vault.enc.bak after every save.").color(colors.muted));ui.horizontal_wrapped(|ui|{if ui.add_enabled(!self.demo,egui::Button::new("Encrypted backup…")).clicked(){if let Some(path)=rfd::FileDialog::new().add_filter("Encrypted vault",&["enc"]).set_file_name("rustyvault-backup.enc").save_file(){if let Err(e)=self.check_export_path(&path).and_then(|_|self.check_export_path(&path.with_extension("enc.bak"))){self.error=Some(e.to_string());}else if let Some(key)=&self.key{match self.vault.as_ref().unwrap().save_to_file(&path,key){Ok(())=>self.notice="Encrypted backup saved".into(),Err(e)=>self.error=Some(e.to_string())}}else{self.error=Some("Backups are unavailable in demo mode".into());}}}
                if ui.button("Import CSV...").clicked() { self.choose_csv(false); }
                if ui.add_enabled(!self.demo,egui::Button::new("Import legacy emails.txt…")).clicked(){if let Some(path)=rfd::FileDialog::new().add_filter("Legacy text",&["txt"]).pick_file(){let mut imported=Vault::new();match imported.import_legacy_txt(&path){Ok(_)=>{self.pending_import=Some(imported.entries);self.pending_import_indices=None;},Err(e)=>self.error=Some(e.to_string())}}}
                if ui.add_enabled(!self.demo,egui::Button::new("Export plaintext CSV…")).clicked(){self.confirmation=Some(Confirm::Export);}
            });ui.add_space(8.0);ui.label(RichText::new(self.path.to_string_lossy()).small().color(colors.muted));

            });
            settings_section(ui, "Change master password", |ui| {
form_field(ui,"Current master password",&mut self.master,true);form_field(ui,"New master password · at least 12 characters",&mut self.new_master,true);form_field(ui,"Confirm new master password",&mut self.new_confirm,true);
            if ui.button("Change master password").clicked(){if self.new_master!=self.new_confirm||self.new_master.chars().count()<12{self.error=Some("New passwords must match and contain at least 12 characters".into());}else if self.demo{self.error=Some("Master password changes are unavailable in demo mode".into());}else{match Vault::load_from_file(&self.path,&self.master){Ok(_)=>{let mut next=self.vault.as_ref().unwrap().clone();next.salt=crypto::generate_salt();match crypto::derive_key(&self.new_master,&next.salt).map_err(VaultError::from).and_then(|key|{next.save_to_file(&self.path,&key)?;Ok(key)}){Ok(key)=>{self.vault=Some(next);self.key=Some(key);self.master.zeroize();self.new_master.zeroize();self.new_confirm.zeroize();self.error=None;self.notice="Master password changed. Older backups still need the old password.".into();},Err(e)=>self.error=Some(e.to_string())}},Err(_)=>self.error=Some("Current master password is incorrect".into())}}}self.message(ui);

            });
        });
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        let focus_new = self.dialog_focus.is_none();
        if (self.confirmation.is_some()
            || self.pending_import.is_some()
            || self.browser_import_open)
            && focus_new
        {
            self.dialog_focus = Some(ctx.memory(|m| m.focused()));
        }
        self.browser_import_dialog(ctx, focus_new);
        if let Some(action) = self.confirmation.take() {
            let mut confirm = false;
            let mut cancel =
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            let (title,body,button)=match &action{Confirm::BrowserSave(_)=>("Save browser login?","Check the website and account below before saving. An existing login for this website and username will be updated, keeping its history.","Save login"),Confirm::Trash(_)=>("Move entry to trash?","You can restore this entry from Trash.","Move to trash"),Confirm::Purge(_)=>("Delete this entry forever?","The entry and its history will be removed. This cannot be undone.","Delete forever"),Confirm::Export=>("Export unencrypted passwords?","This file contains passwords in plain text. Anyone can read it. Values are preserved exactly and may be spreadsheet formulas. Open as text, not in a spreadsheet. Attachments, custom fields and history are excluded.","Export file"),Confirm::Discard=>("Discard your changes?","Unsaved entry changes will be lost.","Discard changes"),Confirm::RestoreHistory(..)=>("Restore this version?","Current credential values will be saved in history before restoring.","Restore version")};
            egui::Window::new(title)
                .order(egui::Order::Foreground)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .default_width(400.0)
                .show(ctx, |ui| {
                    ui.label(body);
                    if let Confirm::BrowserSave(entry) = &action {
                        ui.label(&entry.url);
                        ui.label(format!("Account: {}", entry.username_or_email()));
                    }
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        let confirm_response = ui.button(button);
                        confirm = confirm_response.clicked();
                        let cancel_response = ui.button("Cancel");
                        cancel |= cancel_response.clicked();
                        if !ui.memory(|m| {
                            m.focused().is_some_and(|id| {
                                id == cancel_response.id || id == confirm_response.id
                            })
                        }) {
                            cancel_response.request_focus();
                        }
                        self.track("dialog-confirm", button, "button", &confirm_response);
                        self.track("dialog-cancel", "Cancel", "button", &cancel_response);
                    });
                });
            if confirm {
                match action {
                    Confirm::BrowserSave(entry) => {
                        let mut next = self.vault.as_ref().unwrap().clone();
                        if !next.update_entry((*entry).clone()) {
                            next.add_entry((*entry).clone());
                        }
                        if !self.commit(next, "Browser login saved") {
                            self.confirmation = Some(Confirm::BrowserSave(entry));
                        }
                    }
                    Confirm::Discard => {
                        self.editing = None;
                        self.error = None;
                        self.reveal = false;
                    }
                    Confirm::Export => {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name("rustyvault-export.csv")
                            .save_file()
                        {
                            match vault::write_csv(&self.vault.as_ref().unwrap().entries).and_then(
                                |bytes| {
                                    let bytes = Zeroizing::new(bytes);
                                    self.check_export_path(&path)
                                        .and_then(|_| std::fs::write(path, &*bytes))
                                        .map_err(VaultError::from)
                                },
                            ) {
                                Ok(()) => {
                                    self.notice =
                                        "Plaintext CSV exported — store it securely".into()
                                }
                                Err(e) => self.error = Some(e.to_string()),
                            }
                        }
                    }
                    Confirm::Trash(id) => {
                        let mut next = self.vault.as_ref().unwrap().clone();
                        if let Some(e) = next.entries.iter_mut().find(|e| e.id == id) {
                            e.deleted_at = Some(chrono::Utc::now());
                        }
                        if self.commit(next, "Entry moved to trash") {
                            self.selected = None;
                        }
                    }
                    Confirm::Purge(id) => {
                        let mut next = self.vault.as_ref().unwrap().clone();
                        next.remove_entry(&id);
                        if self.commit(next, "Entry permanently deleted") {
                            self.selected = None;
                        }
                    }
                    Confirm::RestoreHistory(id, index) => {
                        let mut next = self.vault.as_ref().unwrap().clone();
                        if let Some(e) = next.entries.iter().find(|e| e.id == id) {
                            if let Some(old) = e.history.get(index) {
                                let mut restored = e.clone();
                                restored.service = old.service.clone();
                                restored.username = old.username.clone();
                                restored.email = old.email.clone();
                                restored.password = old.password.clone();
                                restored.url = old.url.clone();
                                restored.notes = old.notes.clone();
                                restored.totp_secret = old.totp_secret.clone();
                                restored.fields = old.fields.clone();
                                next.update_entry(restored);
                                self.commit(next, "Version restored");
                            }
                        }
                    }
                }
            } else if !cancel {
                self.confirmation = Some(action);
            }
            if confirm || cancel {
                if let Some(id) = self.dialog_focus.take().flatten() {
                    ctx.memory_mut(|m| m.request_focus(id));
                }
            }
        }
        if let Some(entries) = self.pending_import.take() {
            let new_indices = self.pending_import_indices.take().unwrap_or_else(|| {
                vault::new_import_indices(&self.vault.as_ref().unwrap().entries, &entries)
            });
            let skipped = entries.len() - new_indices.len();
            let mut confirm = false;
            let mut cancel =
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            egui::Window::new("Review import").order(egui::Order::Foreground).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER,Vec2::ZERO).default_width(440.0).show(ctx,|ui|{
                ui.label(format!("To import: {} · Exact duplicates skipped: {}",new_indices.len(),skipped));
                ui.label("Existing entries are kept. Different passwords or details are imported separately.");
                if self.browser_import_pending { ui.label(RichText::new("After importing, delete the exported plaintext CSV, including any extra copies.").color(Palette::get(ui.ctx()).danger)); }
                egui::ScrollArea::vertical().max_height(240.0).show(ui,|ui|{for &index in new_indices.iter().take(50){let entry=&entries[index];ui.label(format!("{} · {}",entry.service,entry.username_or_email()));}});
                if new_indices.len()>50{ui.label(format!("And {} more entries",new_indices.len()-50));}
                if new_indices.is_empty(){ui.label("No new entries to import.");}
                ui.add_space(16.0);
                ui.horizontal(|ui|{
                    let import_response=ui.add_enabled(!new_indices.is_empty(),egui::Button::new("Import entries"));
                    let cancel_response=ui.button("Cancel");
                    confirm=import_response.clicked();
                    cancel |= cancel_response.clicked();
                    if !ui.memory(|m|m.focused().is_some_and(|id|id==import_response.id||id==cancel_response.id)){cancel_response.request_focus();}
                    self.track("dialog-confirm","Import entries","button",&import_response);
                    self.track("dialog-cancel","Cancel","button",&cancel_response);
                });
            });
            if confirm {
                let mut next = self.vault.as_ref().unwrap().clone();
                next.entries
                    .extend(new_indices.iter().map(|&index| entries[index].clone()));
                let mut message = format!(
                    "Imported: {}. Exact duplicates skipped: {}.",
                    new_indices.len(),
                    skipped
                );
                if self.browser_import_pending {
                    message.push_str(" Delete the exported plaintext CSV.");
                }
                if self.commit(next, &message) {
                    self.view = View::All;
                    self.query.clear();
                    self.selected = new_indices.first().map(|&index| entries[index].id.clone());
                    self.reveal = false;
                } else {
                    self.pending_import = Some(entries);
                }
            } else if !cancel {
                self.pending_import = Some(entries);
            }
            if self.pending_import.is_none() {
                self.browser_import_pending = false;
                if let Some(id) = self.dialog_focus.take().flatten() {
                    ctx.memory_mut(|m| m.request_focus(id));
                }
            } else {
                self.pending_import_indices = Some(new_indices);
            }
        }
    }
    fn check_export_path(&self, path: &Path) -> std::io::Result<()> {
        if self.demo {
            return Err(std::io::Error::other(
                "File exports are unavailable in demo mode",
            ));
        }
        for protected in [
            &self.path,
            &self.path.with_extension("enc.bak"),
            &self.path.with_extension("enc.lock"),
        ] {
            if path == protected
                || path
                    .canonicalize()
                    .ok()
                    .zip(protected.canonicalize().ok())
                    .is_some_and(|(a, b)| a == b)
            {
                return Err(std::io::Error::other("Choose a different file. Exports cannot replace the open vault, its backup or lock file."));
            }
        }
        Ok(())
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let colors = Palette::get(ctx);
        self.frames += 1;
        self.bounds.clear();
        ctx.request_repaint_after(Duration::from_secs(1));
        if ctx.input(|i| !i.events.is_empty()) {
            self.last_input = Instant::now();
        }
        if self
            .clipboard
            .as_ref()
            .is_some_and(|(_, time)| time.elapsed().as_secs() >= self.settings.clipboard_seconds)
        {
            self.clear_clipboard();
        }
        if self.vault.is_some() {
            if !self.demo
                && (self.last_input.elapsed().as_secs()
                    >= self.vault.as_ref().unwrap().settings.lock_seconds
                    || (self.vault.as_ref().unwrap().settings.lock_on_focus_loss
                        && !ctx.input(|i| i.focused)))
            {
                self.lock(ctx);
            }
            self.keyboard_shortcuts(ctx);
        }
        self.poll_brave_import();
        self.browser_requests(ctx);
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::none()
                    .fill(colors.background)
                    .inner_margin(egui::Margin::symmetric(20.0, 8.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(if self.demo {
                            "Demo · synthetic data · changes stay in memory"
                        } else if self.vault.is_some() {
                            "Vault unlocked · stored locally"
                        } else {
                            "Vault locked"
                        })
                        .small()
                        .color(colors.muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if self.dark_mode {
                            "Light mode"
                        } else {
                            "Dark mode"
                        };
                        let response = ui.add_enabled(
                            self.confirmation.is_none()
                                && self.pending_import.is_none()
                                && !self.browser_import_open,
                            egui::Button::new(label),
                        );
                        self.track("theme-toggle", label, "button", &response);
                        if response.clicked() {
                            self.change_appearance(ctx, !self.dark_mode);
                        }
                        ui.label(RichText::new(&self.notice).small().color(colors.muted));
                    });
                });
            });
        if self.vault.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.locked(ui));
            });
        } else {
            self.sidebar(ctx);
            if self.vault.is_some() {
                let list = matches!(
                    self.view,
                    View::All | View::Favorites | View::Group(_) | View::Tag(_) | View::Trash
                );
                if list {
                    self.entries(ctx);
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::none().fill(colors.background).inner_margin(
                        if ctx.screen_rect().width() < 1000.0 {
                            16.0
                        } else {
                            24.0
                        },
                    ))
                    .show(ctx, |ui| {
                        if self.confirmation.is_some()
                            || self.pending_import.is_some()
                            || self.browser_import_open
                        {
                            ui.disable();
                        }
                        if self.editing.is_some() {
                            self.editor(ui);
                        } else {
                            match self.view {
                                View::Generator => self.generator(ui),
                                View::Health => self.health(ui),
                                View::Settings => self.settings_screen(ui),
                                _ => self.detail(ui),
                            }
                        }
                        self.message(ui);
                    });
                self.dialogs(ctx);
            }
        }
        if let Some(directory) = self.capture.clone() {
            for event in ctx.input(|i| i.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    let result = std::fs::create_dir_all(&directory).and_then(|_| {
                        image::save_buffer(
                            directory.join("screen.png"),
                            &bytes,
                            image.size[0] as u32,
                            image.size[1] as u32,
                            image::ColorType::Rgba8,
                        )
                        .map_err(std::io::Error::other)
                    });
                    if result.is_ok() {
                        let rect = ctx.screen_rect();
                        let name = if self.vault.is_none() {
                            if self.create {
                                "Create vault"
                            } else {
                                "Unlock vault"
                            }
                        } else if self.editing.is_some() {
                            "Edit entry"
                        } else if self.confirmation.is_some() {
                            "Confirmation"
                        } else if self.pending_import.is_some() {
                            "Import preview"
                        } else if self.browser_import_open {
                            if self.browser_setup_only {
                                "Browser setup"
                            } else {
                                "Browser import"
                            }
                        } else {
                            self.view.title()
                        };
                        let state = serde_json::json!({"name":name,"viewport":{"width":rect.width(),"height":rect.height(),"devicePixelRatio":ctx.pixels_per_point()},"elements":self.bounds});
                        if let Err(e) = std::fs::write(
                            directory.join("bounds.json"),
                            serde_json::to_vec_pretty(&state).unwrap(),
                        ) {
                            self.error = Some(e.to_string());
                        } else {
                            self.capture = None;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    } else {
                        self.error = Some("Could not write demo capture".into());
                        self.capture = None;
                    }
                }
            }
            if self.frames == 8 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            }
        }
    }
}
impl Drop for App {
    fn drop(&mut self) {
        self.clear_clipboard();
        self.master.zeroize();
        self.confirm_master.zeroize();
        self.new_master.zeroize();
        self.new_confirm.zeroize();
        self.generated.zeroize();
    }
}
fn settings_section(ui: &mut egui::Ui, title: &str, contents: impl FnOnce(&mut egui::Ui)) {
    let colors = Palette::get(ui.ctx());
    egui::Frame::none()
        .fill(colors.surface)
        .rounding(12.0)
        .inner_margin(16.0)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(title).size(18.0));
            ui.add_space(8.0);
            contents(ui);
        });
    ui.add_space(16.0);
}

fn checkbox(ui: &mut egui::Ui, value: &mut bool, label: &str) -> egui::Response {
    let border = Palette::get(ui.ctx()).control_border;
    ui.scope(|ui| {
        ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, border);
        ui.visuals_mut().widgets.inactive.rounding = 3.0.into();
        ui.checkbox(value, label)
    })
    .inner
}

fn primary<'a>(ui: &egui::Ui, label: &'a str) -> egui::Button<'a> {
    let colors = Palette::get(ui.ctx());
    egui::Button::new(RichText::new(label).color(colors.action_on).strong()).fill(colors.action)
}
fn form_field(ui: &mut egui::Ui, label: &str, value: &mut String, secret: bool) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.visuals_mut().widgets.inactive.bg_stroke =
            egui::Stroke::new(1.0_f32, Palette::get(ui.ctx()).control_border);
        let label = ui.label(RichText::new(label).color(Palette::get(ui.ctx()).secondary));
        ui.add(
            egui::TextEdit::singleline(value)
                .margin(INPUT_PADDING)
                .password(secret)
                .vertical_align(egui::Align::Center)
                .desired_width(f32::INFINITY)
                .min_size(Vec2::new(0.0, 44.0)),
        )
        .labelled_by(label.id);
    });
}
fn inferred_website(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        return None;
    }
    let explicit_url = name.starts_with("https://") || name.starts_with("http://");
    if !explicit_url && (name.contains("://") || !name.contains('.')) {
        return None;
    }
    let candidate = if explicit_url {
        name.to_owned()
    } else {
        format!("https://{name}")
    };
    let url = url::Url::parse(&candidate).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    Some(url.to_string())
}

fn read_small_file(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(std::io::Error::other("File exceeds the size limit"));
    }
    Ok(bytes)
}
#[cfg(test)]
mod app_tests {
    use super::*;
    use eframe::egui::Color32;

    #[test]
    fn startup_prompts_for_creation_only_when_the_selected_vault_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("personal.enc");
        let fresh = App::new(false, Some(path.clone()));
        assert!(fresh.create && fresh.vault.is_none());
        assert!(fresh.master.is_empty() && fresh.confirm_master.is_empty());
        assert!(!path.exists());

        std::fs::write(&path, b"existing vault sentinel").unwrap();
        let existing = App::new(false, Some(path.clone()));
        assert!(!existing.create && existing.vault.is_none());
        assert_eq!(std::fs::read(path).unwrap(), b"existing vault sentinel");
    }

    #[test]
    fn native_window_icon_is_valid_and_square() {
        let icon =
            eframe::icon_data::from_png_bytes(include_bytes!("../assets/rustyvault.png")).unwrap();
        assert!(icon.width >= 64);
        assert_eq!(icon.width, icon.height);
        assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
    }

    #[test]
    fn native_navigation_selects_views_without_discarding_drafts() {
        fn render(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) {
            let _ = ctx.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.set_width(160.0);
                        app.nav(ui, "Favorites", View::Favorites, Some(2));
                    });
                },
            );
        }
        let ctx = egui::Context::default();
        theme(&ctx, true);
        let mut app = App::new(true, None);
        app.capture = Some("unused-demo-capture".into());
        render(&mut app, &ctx, vec![]);
        let rect = &app.bounds.last().unwrap()["rect"];
        let pos = egui::pos2(
            (rect["x"].as_f64().unwrap() + rect["width"].as_f64().unwrap() / 2.0) as f32,
            (rect["y"].as_f64().unwrap() + rect["height"].as_f64().unwrap() / 2.0) as f32,
        );
        for editing in [false, true] {
            app.view = View::All;
            if editing {
                app.edit(app.selected_entry().unwrap());
                app.editing.as_mut().unwrap().service = "Unsaved draft".into();
            }
            for pressed in [true, false] {
                render(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            assert!(app.view == if editing { View::All } else { View::Favorites });
            if editing {
                assert_eq!(app.editing.as_ref().unwrap().service, "Unsaved draft");
                assert!(app.confirmation.is_none());
            }
        }
    }

    #[test]
    fn raycast_appearance_switching_preserves_vault_and_survives_lock() {
        let mut app = App::new(true, None);
        let original = serde_json::to_string(&app.vault.as_ref().unwrap().entries).unwrap();
        let ctx = egui::Context::default();
        for dark in [false, true] {
            app.change_appearance(&ctx, dark);
            assert_eq!(ctx.style().visuals.dark_mode, dark);
            assert_eq!(Palette::get(&ctx).background, Palette::new(dark).background);
        }
        assert_eq!(
            serde_json::to_string(&app.vault.as_ref().unwrap().entries).unwrap(),
            original
        );
        app.lock(&ctx);
        assert!(ctx.style().visuals.dark_mode);
        assert_eq!(Palette::get(&ctx).background, Color32::from_rgb(7, 8, 10));
    }

    #[test]
    fn palettes_keep_text_and_controls_readable() {
        fn contrast(a: Color32, b: Color32) -> f32 {
            let luminance = |color: Color32| {
                let channels = [color.r(), color.g(), color.b()].map(|channel| {
                    let value = f32::from(channel) / 255.0;
                    if value <= 0.04045 {
                        value / 12.92
                    } else {
                        ((value + 0.055) / 1.055).powf(2.4)
                    }
                });
                channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722
            };
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        }
        let ctx = egui::Context::default();
        for dark in [true, false, true] {
            apply_palette(&ctx, dark);
            let colors = Palette::new(dark);
            assert_eq!(ctx.style().visuals.dark_mode, dark);
            assert_eq!(ctx.style().visuals.panel_fill, colors.background);
            for background in [colors.background, colors.surface] {
                for text in [
                    colors.foreground,
                    colors.muted,
                    colors.accent,
                    colors.danger,
                ] {
                    assert!(
                        contrast(text, background) >= 4.5,
                        "Unreadable text in dark={dark}"
                    );
                }
            }
            assert!(contrast(colors.secondary, colors.warm) >= 4.5);
            assert!(contrast(colors.foreground, colors.warm) >= 4.5);
            assert!(contrast(colors.action_on, colors.action) >= 4.5);
            assert!(contrast(colors.muted, colors.surface) >= 3.0);
            assert!(contrast(colors.accent, colors.warm) >= 3.0);
        }
    }

    #[test]
    fn appearance_preference_contains_only_the_theme() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config/appearance.txt");
        for (dark, expected) in [(true, "dark"), (false, "light")] {
            save_appearance(&path, dark).unwrap();
            assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);
        }
        assert!(save_appearance(directory.path(), true).is_err());
        let mut app = App::new(true, None);
        let ctx = egui::Context::default();
        app.change_appearance(&ctx, false);
        app.lock(&ctx);
        assert!(!app.dark_mode);
        assert!(!ctx.style().visuals.dark_mode);
        app.demo = false;
        app.path = appearance_path();
        app.change_appearance(&ctx, true);
        assert!(app
            .error
            .as_deref()
            .unwrap()
            .contains("cannot replace the open vault"));
    }

    #[test]
    fn escape_cancels_dialogs_without_changing_data() {
        let mut app = App::new(true, None);
        let original = app.vault.as_ref().unwrap().entries.len();
        let ctx = egui::Context::default();
        let escape = || egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        };
        app.confirmation = Some(Confirm::Purge(app.selected.clone().unwrap()));
        let _ = ctx.run(escape(), |ctx| app.dialogs(ctx));
        assert!(app.confirmation.is_none());
        assert_eq!(app.vault.as_ref().unwrap().entries.len(), original);
        app.pending_import = Some(vec![VaultEntry::new("Import", "", "", "secret")]);
        let _ = ctx.run(escape(), |ctx| app.dialogs(ctx));
        assert!(app.pending_import.is_none());
        assert_eq!(app.vault.as_ref().unwrap().entries.len(), original);
    }

    #[test]
    fn exports_cannot_overwrite_vault_and_only_one_window_can_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.enc");
        let mut first = App::new(false, Some(path.clone()));
        first.master = "long test master passphrase".into();
        first.confirm_master = first.master.clone();
        first.unlock();
        assert!(first.vault.is_some());
        let original = std::fs::read(&path).unwrap();
        assert!(first.check_export_path(&path).is_err());
        assert!(first
            .check_export_path(&dir.path().join(".").join("vault.enc"))
            .is_err());
        assert!(first
            .check_export_path(&path.with_extension("enc.bak"))
            .is_err());
        assert!(first
            .check_export_path(&dir.path().join("export.csv"))
            .is_ok());
        let mut second = App::new(false, Some(path.clone()));
        second.master = "long test master passphrase".into();
        second.unlock();
        assert!(second.vault.is_none());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        first.lock(&egui::Context::default());
        second.unlock();
        assert!(second.vault.is_some());
        second.lock(&egui::Context::default());
        second.create = true;
        second.unlock();
        assert!(second.vault.is_none());
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    #[test]
    fn failed_save_preserves_visible_data_and_draft() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, b"not a directory").unwrap();
        let mut app = App::new(false, Some(blocker.join("vault.enc")));
        let v = Vault::new();
        app.key = Some(crypto::derive_key("test passphrase", &v.salt).unwrap());
        app.vault = Some(v);
        app.edit(VaultEntry::new("Draft", "user", "", "secret"));
        let mut next = app.vault.as_ref().unwrap().clone();
        next.add_entry(app.editing.as_ref().unwrap().clone());
        assert!(!app.commit(next, "Saved"));
        assert!(app.vault.as_ref().unwrap().entries.is_empty());
        assert_eq!(app.editing.as_ref().unwrap().service, "Draft");
        assert!(app.error.as_ref().unwrap().contains("Nothing was saved"));
    }

    #[test]
    fn locking_erases_editors_and_demo_never_opens_real_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.enc");
        std::fs::write(&path, b"personal vault sentinel").unwrap();
        let mut app = App::new(true, Some(path.clone()));
        app.master = "master secret".into();
        app.new_master = "new secret".into();
        app.edit(app.selected_entry().unwrap());
        app.lock(&egui::Context::default());
        assert!(app.vault.is_none() && app.key.is_none() && app.editing.is_none());
        assert!(app.master.is_empty() && app.new_master.is_empty() && app.generated.is_empty());
        app.unlock();
        assert!(app.vault.is_some());
        assert_eq!(std::fs::read(path).unwrap(), b"personal vault sentinel");
    }

    fn fragment_frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>, screen: &str) {
        app.bounds.clear();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 650.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                if screen == "dialogs" {
                    app.dialogs(ctx);
                } else {
                    egui::CentralPanel::default().show(ctx, |ui| match screen {
                        "actions" => app.password_actions(ui),
                        "editor" => app.editor(ui),
                        "settings" => app.settings_screen(ui),
                        _ => unreachable!(),
                    });
                }
            },
        );
    }

    fn click_fragment(app: &mut App, ctx: &egui::Context, screen: &str, id: &str) {
        for _ in 0..2 {
            fragment_frame(app, ctx, vec![], screen);
        }
        let rect = &app
            .bounds
            .iter()
            .find(|value| value["id"] == id)
            .expect("Tracked control missing")["rect"];
        let pos = egui::pos2(
            (rect["x"].as_f64().unwrap() + rect["width"].as_f64().unwrap() / 2.0) as f32,
            (rect["y"].as_f64().unwrap() + rect["height"].as_f64().unwrap() / 2.0) as f32,
        );
        assert!(
            (0.0..650.0).contains(&pos.y),
            "Control is outside the minimum viewport"
        );
        for pressed in [true, false] {
            fragment_frame(
                app,
                ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                screen,
            );
        }
    }

    #[test]
    fn add_password_action_preserves_group_and_existing_draft() {
        let ctx = egui::Context::default();
        theme(&ctx, true);
        let mut app = App::new(true, None);
        app.capture = Some("unused-demo-capture".into());
        app.view = View::Group("Work".into());
        app.query = "old filter".into();
        click_fragment(&mut app, &ctx, "actions", "add-password");
        assert!(app.view == View::All && app.query.is_empty());
        assert_eq!(app.editing.as_ref().unwrap().category, "Work");
        app.editing.as_mut().unwrap().service = "Unsaved draft".into();
        click_fragment(&mut app, &ctx, "actions", "add-password");
        assert_eq!(app.editing.as_ref().unwrap().service, "Unsaved draft");
    }

    #[test]
    fn short_password_form_saves_and_infers_only_safe_websites() {
        for (input, expected) in [
            ("example.com", Some("https://example.com/")),
            (
                "https://example.com/login",
                Some("https://example.com/login"),
            ),
            ("Local account", None),
            ("javascript:alert(1)", None),
            ("https://name:secret@example.com", None),
        ] {
            assert_eq!(inferred_website(input).as_deref(), expected);
        }
        let ctx = egui::Context::default();
        theme(&ctx, true);
        let mut app = App::new(true, None);
        app.capture = Some("unused-demo-capture".into());
        app.add_password();
        let entry = app.editing.as_mut().unwrap();
        entry.service = "example.test".into();
        entry.username = "demo".into();
        entry.password = "SYNTHETIC-DEMO-ONLY".into();
        let before = app.vault.as_ref().unwrap().entries.len();
        click_fragment(&mut app, &ctx, "editor", "save-entry");
        assert!(app.editing.is_none());
        assert_eq!(app.selected_entry().unwrap().url, "https://example.test/");
        assert_eq!(app.vault.as_ref().unwrap().entries.len(), before + 1);
    }

    #[test]
    fn short_editor_preserves_email_and_optional_metadata() {
        let ctx = egui::Context::default();
        theme(&ctx, true);
        let mut app = App::new(true, None);
        app.capture = Some("unused-demo-capture".into());
        let mut entry = app.selected_entry().unwrap();
        entry.username.clear();
        entry.email = "demo@example.test".into();
        entry.fields = vec![("Recovery".into(), "synthetic-only".into())];
        let original = entry.clone();
        *app.vault
            .as_mut()
            .unwrap()
            .entries
            .iter_mut()
            .find(|value| value.id == entry.id)
            .unwrap() = entry.clone();
        app.edit(entry);
        assert!(app.editor_uses_email);
        app.editing.as_mut().unwrap().email = "updated@example.test".into();
        click_fragment(&mut app, &ctx, "editor", "save-entry");
        assert!(app.editing.is_none());
        let saved = app.selected_entry().unwrap();
        assert_eq!(saved.email, "updated@example.test");
        assert!(saved.username.is_empty());
        assert_eq!(saved.url, original.url);
        assert_eq!(saved.notes, original.notes);
        assert_eq!(saved.fields, original.fields);
        assert_eq!(saved.tags, original.tags);
        assert_eq!(saved.favorite, original.favorite);
        assert_eq!(saved.attachments.len(), original.attachments.len());
        assert_eq!(saved.attachments[0].bytes, original.attachments[0].bytes);
        assert_eq!(saved.history.len(), original.history.len() + 1);
    }

    #[test]
    fn short_editor_keeps_invalid_draft_and_saves_secure_notes() {
        let ctx = egui::Context::default();
        theme(&ctx, true);
        let mut app = App::new(true, None);
        app.capture = Some("unused-demo-capture".into());
        app.add_password();
        app.editing.as_mut().unwrap().service = "Synthetic note".into();
        app.editing.as_mut().unwrap().kind = "Secure note".into();
        app.editing.as_mut().unwrap().notes = "Synthetic note content".into();
        app.expiry_input = "not-a-date".into();
        let before = app.vault.as_ref().unwrap().entries.len();
        click_fragment(&mut app, &ctx, "editor", "save-entry");
        assert_eq!(app.vault.as_ref().unwrap().entries.len(), before);
        assert_eq!(
            app.editing.as_ref().unwrap().notes,
            "Synthetic note content"
        );
        assert!(app.error.as_deref().unwrap().contains("Expiry"));
        app.expiry_input.clear();
        click_fragment(&mut app, &ctx, "editor", "save-entry");
        assert!(app.editing.is_none());
        assert_eq!(
            app.selected_entry().unwrap().notes,
            "Synthetic note content"
        );
    }

    #[test]
    fn browser_setup_is_direct_and_import_remains_separate() {
        let ctx = egui::Context::default();
        theme(&ctx, true);
        let mut app = App::new(true, None);
        app.capture = Some("unused-demo-capture".into());
        click_fragment(&mut app, &ctx, "settings", "settings-browser-setup");
        assert!(app.browser_import_open && app.browser_setup_only);
        fragment_frame(&mut app, &ctx, vec![], "dialogs");
        assert!(app
            .bounds
            .iter()
            .any(|value| value["id"] == "browser-setup-native"));
        assert!(!app
            .bounds
            .iter()
            .any(|value| value["id"] == "browser-choose-csv"));
        click_fragment(&mut app, &ctx, "dialogs", "browser-setup-native");
        assert!(app.browser_setup_path.is_some() && app.browser.is_none());
        click_fragment(&mut app, &ctx, "dialogs", "browser-close");
        assert!(!app.browser_import_open);
        click_fragment(&mut app, &ctx, "settings", "settings-import-browser");
        assert!(app.browser_import_open && !app.browser_setup_only);
        fragment_frame(&mut app, &ctx, vec![], "dialogs");
        assert!(app
            .bounds
            .iter()
            .any(|value| value["id"] == "browser-choose-csv"));
        app.lock(&ctx);
        assert!(!app.browser_import_open && !app.browser_setup_only);
    }

    #[test]
    fn named_accounts_explain_autofill_and_accept_websites_or_local_accounts() {
        let ctx = egui::Context::default();
        theme(&ctx, true);
        let mut app = App::new(true, None);
        app.capture = Some("unused-demo-capture".into());
        app.add_password();
        app.editing.as_mut().unwrap().service = "Example account".into();
        app.editing.as_mut().unwrap().password = "SYNTHETIC-DEMO-ONLY".into();
        fragment_frame(&mut app, &ctx, vec![], "editor");
        assert!(app
            .bounds
            .iter()
            .any(|value| value["id"] == "autofill-website-help"));
        app.editing.as_mut().unwrap().url = "example.test/login".into();
        click_fragment(&mut app, &ctx, "editor", "save-entry");
        assert!(app.editing.is_none());
        assert_eq!(
            app.selected_entry().unwrap().url,
            "https://example.test/login"
        );
        app.add_password();
        app.editing.as_mut().unwrap().service = "Local account".into();
        app.editing.as_mut().unwrap().password = "SYNTHETIC-DEMO-ONLY".into();
        click_fragment(&mut app, &ctx, "editor", "save-entry");
        assert!(app.editing.is_none());
        assert!(app.selected_entry().unwrap().url.is_empty());
        app.add_password();
        app.editing.as_mut().unwrap().service = "example.test".into();
        fragment_frame(&mut app, &ctx, vec![], "editor");
        assert!(!app
            .bounds
            .iter()
            .any(|value| value["id"] == "autofill-website-help"));
    }

    #[test]
    fn import_review_skips_exact_duplicates_and_preserves_changed_passwords() {
        let ctx = egui::Context::default();
        theme(&ctx, true);
        let mut app = App::new(true, None);
        app.capture = Some("unused-demo-capture".into());
        let original = app.selected_entry().unwrap();
        let mut changed = original.clone();
        changed.id = "synthetic-import-id".into();
        changed.password = "SYNTHETIC-CHANGED-PASSWORD".into();
        let before = app.vault.as_ref().unwrap().entries.len();
        app.view = View::Favorites;
        app.query = "filter hiding imported entries".into();
        app.pending_import = Some(vec![original.clone(), changed.clone(), changed.clone()]);
        click_fragment(&mut app, &ctx, "dialogs", "dialog-confirm");
        assert!(app.pending_import.is_none());
        assert_eq!(app.vault.as_ref().unwrap().entries.len(), before + 1);
        assert!(app.view == View::All && app.query.is_empty());
        assert_eq!(app.selected.as_deref(), Some("synthetic-import-id"));
        assert!(app
            .notice
            .contains("Imported: 1. Exact duplicates skipped: 2."));
        assert_eq!(
            app.vault
                .as_ref()
                .unwrap()
                .entries
                .iter()
                .find(|entry| entry.id == original.id)
                .unwrap()
                .password,
            original.password
        );
        app.pending_import = Some(vec![original, changed]);
        click_fragment(&mut app, &ctx, "dialogs", "dialog-confirm");
        assert!(app.pending_import.is_some());
        assert_eq!(app.vault.as_ref().unwrap().entries.len(), before + 1);
        click_fragment(&mut app, &ctx, "dialogs", "dialog-cancel");
        assert!(app.pending_import.is_none());
    }

    #[test]
    fn automatic_brave_export_stages_review_and_recovers_from_failed_save() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        theme(&ctx, true);
        let mut app = App::new(true, Some(dir.path().join("vault.enc")));
        app.capture = Some("unused-demo-capture".into());
        app.browser_import_open = true;
        app.brave_import_watch = Some((
            BraveExportWatch::new(dir.path().into()).unwrap(),
            dir.path().into(),
            Instant::now(),
        ));
        let path = dir.path().join("Brave Passwords.csv");
        std::fs::write(&path, "name,url,username,password,note\nExample,https://example.test,demo,SYNTHETIC-DEMO-ONLY,Imported note\n").unwrap();
        for _ in 0..3 {
            app.brave_import_next_poll = Instant::now();
            app.poll_brave_import();
        }
        assert!(app.brave_import_watch.is_none() && !app.browser_import_open);
        assert_eq!(app.pending_import.as_ref().unwrap().len(), 1);
        assert!(app.browser_import_pending);
        let before = app.vault.as_ref().unwrap().entries.len();
        app.demo = false;
        click_fragment(&mut app, &ctx, "dialogs", "dialog-confirm");
        assert!(app.pending_import.is_some());
        assert_eq!(app.vault.as_ref().unwrap().entries.len(), before);
        assert!(!dir.path().join("vault.enc").exists());
        app.demo = true;
        click_fragment(&mut app, &ctx, "dialogs", "dialog-confirm");
        assert!(app.pending_import.is_none());
        assert_eq!(app.vault.as_ref().unwrap().entries.len(), before + 1);
        assert!(
            path.is_file(),
            "The source export must not be deleted automatically"
        );
    }

    #[test]
    fn automatic_brave_import_times_out_and_cancels_on_close_or_lock() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        theme(&ctx, true);
        let mut app = App::new(true, None);
        app.capture = Some("unused-demo-capture".into());
        for action in ["timeout", "close", "lock"] {
            app.browser_import_open = true;
            app.brave_import_next_poll = Instant::now();
            app.brave_import_watch = Some((
                BraveExportWatch::new(dir.path().into()).unwrap(),
                dir.path().into(),
                if action == "timeout" {
                    Instant::now() - Duration::from_secs(301)
                } else {
                    Instant::now()
                },
            ));
            match action {
                "timeout" => {
                    app.poll_brave_import();
                    assert!(app.error.as_deref().unwrap().contains("timed out"));
                }
                "close" => click_fragment(&mut app, &ctx, "dialogs", "browser-close"),
                _ => app.lock(&ctx),
            }
            assert!(app.brave_import_watch.is_none());
            assert!(app.pending_import.is_none());
        }
    }

    #[test]
    fn browser_import_demo_reviews_before_commit() {
        let ctx = egui::Context::default();
        theme(&ctx, true);
        let mut app = App::new(true, None);
        app.capture = Some("unused-demo-capture".into());
        let before = app.vault.as_ref().unwrap().entries.len();
        click_fragment(&mut app, &ctx, "actions", "import-browser");
        assert!(app.browser_import_open);
        click_fragment(&mut app, &ctx, "dialogs", "brave-start-import");
        assert!(!app.browser_import_open && app.browser_import_pending);
        assert_eq!(app.pending_import.as_ref().unwrap().len(), 1);
        assert_eq!(app.vault.as_ref().unwrap().entries.len(), before);
        click_fragment(&mut app, &ctx, "dialogs", "dialog-confirm");
        assert!(app.pending_import.is_none() && !app.browser_import_pending);
        assert_eq!(app.vault.as_ref().unwrap().entries.len(), before + 1);
        assert!(app.notice.contains("Delete the exported plaintext CSV"));
        app.browser_import_open = true;
        fragment_frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            "dialogs",
        );
        assert!(!app.browser_import_open);
    }

    #[test]
    fn demo_browser_setup_never_starts_a_real_connection() {
        let mut app = App::new(true, None);
        for browser in BrowserKind::ALL {
            app.browser_kind = browser;
            app.open_browser_password_manager();
            assert!(app.notice.starts_with("Demo:"));
            app.setup_browser_connection();
            assert_eq!(
                app.browser_setup_path.as_deref(),
                Some(Path::new("Demo extension folder"))
            );
            app.vault.as_mut().unwrap().settings.browser_enabled = true;
            app.browser_requests(&egui::Context::default());
            assert!(app.browser.is_none() && app.browser_last_seen.is_none());
            assert!(app.browser_status().starts_with("Demo:"));
        }
    }
}
