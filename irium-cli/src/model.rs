use std::{
    collections::{BTreeSet, HashMap, HashSet},
    hash::Hash,
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
    time::Instant,
};

use ratatui::{layout::Rect, text::Line};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use ratatui_interact::{state::FocusManager, traits::ClickRegionRegistry};
use tachyonfx::Effect;

use crate::ai::{
    ModelListItem,
    worker::{AiWorkerCommand, AiWorkerEvent},
};
use crate::preview::{
    PreviewSourceMeta,
    worker::{PreviewWorkerCommand, PreviewWorkerEvent},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    Scope,
    Naming,
    Apply,
}

impl Stage {
    pub const ALL: [Stage; 3] = [Stage::Scope, Stage::Naming, Stage::Apply];

    pub fn title(self) -> &'static str {
        match self {
            Stage::Scope => "Scope",
            Stage::Naming => "Naming",
            Stage::Apply => "Apply",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Stage::Scope => Stage::Naming,
            Stage::Naming => Stage::Apply,
            Stage::Apply => Stage::Scope,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Stage::Scope => Stage::Apply,
            Stage::Naming => Stage::Scope,
            Stage::Apply => Stage::Naming,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeTab {
    Files,
    Select,
    Constraint,
    Preset,
    Marketplace,
}

impl ScopeTab {
    pub fn title(self) -> &'static str {
        match self {
            ScopeTab::Files => "Files",
            ScopeTab::Select => "Category",
            ScopeTab::Constraint => "Constraint",
            ScopeTab::Preset => "Preset",
            ScopeTab::Marketplace => "Marketplace",
        }
    }

    pub fn next(self) -> Self {
        match self {
            ScopeTab::Files => ScopeTab::Select,
            ScopeTab::Select => ScopeTab::Constraint,
            ScopeTab::Constraint => ScopeTab::Preset,
            ScopeTab::Preset => ScopeTab::Marketplace,
            ScopeTab::Marketplace => ScopeTab::Files,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            ScopeTab::Files => ScopeTab::Marketplace,
            ScopeTab::Select => ScopeTab::Files,
            ScopeTab::Constraint => ScopeTab::Select,
            ScopeTab::Preset => ScopeTab::Constraint,
            ScopeTab::Marketplace => ScopeTab::Preset,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NamingTab {
    Suggestions,
    Style,
}

impl NamingTab {
    pub fn next(self) -> Self {
        match self {
            NamingTab::Suggestions => NamingTab::Style,
            NamingTab::Style => NamingTab::Suggestions,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusPane {
    ScopeCategory,
    ScopeFiles,
    ScopeOptions,
    NamingTable,
    NamingRight,
    NamingCommand,
    ApplyReview,
    ApplyHistory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub level: ToastLevel,
    pub message: String,
    pub ttl_ticks: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    EditingOverride,
    Command,
    NewCategory,
    AwaitGroup,
    EditingAiApiKey,
    EditingAiModelSearch,
    EditingAiManualModel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameLength {
    None,
    Long,
    Medium,
    Short,
}

impl NameLength {
    pub fn next(self) -> Self {
        match self {
            NameLength::None => NameLength::Long,
            NameLength::Long => NameLength::Medium,
            NameLength::Medium => NameLength::Short,
            NameLength::Short => NameLength::None,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            NameLength::None => NameLength::Short,
            NameLength::Long => NameLength::None,
            NameLength::Medium => NameLength::Long,
            NameLength::Short => NameLength::Medium,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capitalization {
    None,
    Lower,
    Title,
    Upper,
}

impl Capitalization {
    pub fn next(self) -> Self {
        match self {
            Capitalization::None => Capitalization::Lower,
            Capitalization::Lower => Capitalization::Title,
            Capitalization::Title => Capitalization::Upper,
            Capitalization::Upper => Capitalization::None,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Capitalization::None => Capitalization::Upper,
            Capitalization::Lower => Capitalization::None,
            Capitalization::Title => Capitalization::Lower,
            Capitalization::Upper => Capitalization::Title,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Separator {
    Space,
    Dash,
    Underscore,
}

impl Separator {
    pub fn next(self) -> Self {
        match self {
            Separator::Space => Separator::Dash,
            Separator::Dash => Separator::Underscore,
            Separator::Underscore => Separator::Space,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Separator::Space => Separator::Underscore,
            Separator::Dash => Separator::Space,
            Separator::Underscore => Separator::Dash,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeConstraint {
    Any,
    Day,
    Week,
    Month,
    Custom,
}

impl TimeConstraint {
    pub const ALL: [TimeConstraint; 5] = [
        TimeConstraint::Any,
        TimeConstraint::Day,
        TimeConstraint::Week,
        TimeConstraint::Month,
        TimeConstraint::Custom,
    ];

    pub fn title(self) -> &'static str {
        match self {
            TimeConstraint::Any => "Any time",
            TimeConstraint::Day => "Last day",
            TimeConstraint::Week => "Last week",
            TimeConstraint::Month => "Last month",
            TimeConstraint::Custom => "Custom (placeholder)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeConstraint {
    Any,
    Small,
    Medium,
    Large,
}

impl SizeConstraint {
    pub const ALL: [SizeConstraint; 4] = [
        SizeConstraint::Any,
        SizeConstraint::Small,
        SizeConstraint::Medium,
        SizeConstraint::Large,
    ];

    pub fn title(self) -> &'static str {
        match self {
            SizeConstraint::Any => "Any size",
            SizeConstraint::Small => "Small < 5 MB",
            SizeConstraint::Medium => "Medium 5-100 MB",
            SizeConstraint::Large => "Large > 100 MB",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StyleOptions {
    pub length: NameLength,
    pub capitalization: Capitalization,
    pub separator: Separator,
    pub keep_extension: bool,
    pub strip_colons: bool,
}

impl Default for StyleOptions {
    fn default() -> Self {
        Self {
            length: NameLength::Medium,
            capitalization: Capitalization::Title,
            separator: Separator::Dash,
            keep_extension: true,
            strip_colons: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenameRow {
    pub path: PathBuf,
    pub current_name: String,
    pub proposed_name: String,
    pub override_name: Option<String>,
    pub group_id: u8,
    pub selected: bool,
    pub conflict: bool,
}

#[derive(Debug, Clone)]
pub struct SessionUndoEntry {
    pub timestamp: String,
    pub affected_paths: Vec<PathBuf>,
    pub previous_names: Vec<String>,
    pub simulated_new_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CategoryFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MarketplacePreset {
    pub name: String,
    pub description: String,
    pub extensions: Vec<String>,
    pub time_constraint: TimeConstraint,
}

#[derive(Debug, Clone)]
pub struct PresetState {
    pub selected_extensions: HashSet<String>,
    pub time_constraint: TimeConstraint,
    pub size_constraint: SizeConstraint,
    pub style: StyleOptions,
    pub show_hidden: bool,
}

#[derive(Debug, Clone)]
pub struct FileNode {
    pub parent: Option<usize>,
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub children: Vec<usize>,
    pub children_loaded: bool,
    pub expanded: bool,
    pub selected: bool,
    pub unreadable: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct VisibleNode {
    pub node_id: usize,
    pub depth: u16,
}

#[derive(Debug, Clone)]
pub struct FileTree {
    pub nodes: Vec<FileNode>,
    pub root: usize,
    pub cursor: usize,
    pub scroll: usize,
}

impl FileTree {
    pub fn new(root: PathBuf) -> Self {
        Self {
            nodes: vec![FileNode {
                parent: None,
                path: root,
                name: ".".to_string(),
                is_dir: true,
                children: Vec::new(),
                children_loaded: false,
                expanded: true,
                selected: false,
                unreadable: false,
            }],
            root: 0,
            cursor: 0,
            scroll: 0,
        }
    }

    pub fn visible_nodes(&self) -> Vec<VisibleNode> {
        let mut out = Vec::new();
        self.collect_visible(self.root, 0, &mut out);
        out
    }

    fn collect_visible(&self, node_id: usize, depth: u16, out: &mut Vec<VisibleNode>) {
        if node_id != self.root {
            out.push(VisibleNode { node_id, depth });
        }

        let node = &self.nodes[node_id];
        if !node.is_dir || !node.expanded {
            return;
        }

        for child in &node.children {
            self.collect_visible(*child, depth.saturating_add(1), out);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClickTarget {
    LogOverlay,
    LogCopyLine(usize),
    StageTab(Stage),
    ScopeTab(ScopeTab),
    ScopeFileRow(usize),
    ScopeFileCheckbox(usize),
    ScopeFilesScrollbarThumb,
    ScopeFilesScrollbarTrack,
    ScopeFilesSettingsButton,
    ScopeFilesSettingShowSelectedCategoriesOnly,
    ScopeCategoryRow(usize),
    ScopeConstraintRow(usize),
    ScopePresetRow(usize),
    ScopeMarketplaceRow(usize),
    ScopePane(FocusPane),
    NamingPane(FocusPane),
    NamingRow(usize),
    NamingSuggestion(usize),
    NamingStyleRow(usize),
    NamingSuggestionsSettingsButton,
    NamingSuggestionsRefreshButton,
    NamingAiSettingsApiKeyField,
    NamingAiSettingsModelSearchField,
    NamingAiSettingsManualModelField,
    NamingAiSettingsSaveConfig,
    NamingAiSettingsClearApiKey,
    NamingAiSettingsToggleReveal,
    NamingAiSettingsDiscoverModels,
    NamingAiSettingsModelRow(usize),
    NamingRowSuggestionOption {
        row_index: usize,
        option_index: usize,
    },
    ApplyPane(FocusPane),
    ApplyRow(usize),
    ApplyHistoryRow(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiSettingsField {
    ApiKey,
    ModelSearch,
    ModelList,
    ManualModel,
    Save,
    Discover,
}

#[derive(Debug, Clone)]
pub struct AiSettingsState {
    pub popup_open: bool,
    pub reveal_api_key: bool,
    pub active_field: AiSettingsField,
    pub api_key: Option<String>,
    pub api_key_input: String,
    pub selected_model: Option<String>,
    pub model_search_query: String,
    pub manual_model_input: String,
    pub discovered_models: Vec<ModelListItem>,
    pub model_list_cursor: usize,
    pub discovery_error: Option<String>,
}

impl Default for AiSettingsState {
    fn default() -> Self {
        Self {
            popup_open: false,
            reveal_api_key: false,
            active_field: AiSettingsField::ApiKey,
            api_key: None,
            api_key_input: String::new(),
            selected_model: None,
            model_search_query: String::new(),
            manual_model_input: String::new(),
            discovered_models: Vec::new(),
            model_list_cursor: 0,
            discovery_error: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamingAiStatus {
    Idle,
    NeedsConfig,
    DiscoveringModels,
    AnalyzingFiles,
    Generating,
    Ready,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesPreviewStatus {
    Hidden,
    Empty,
    Loading,
    Ready,
    Unsupported,
    Error,
}

#[derive(Debug, Clone)]
pub struct SuggestionSet {
    pub request_id: u64,
    pub global_option_index: usize,
    pub per_path_options: HashMap<PathBuf, [String; 3]>,
    pub per_path_override_index: HashMap<PathBuf, usize>,
    pub source_model: String,
}

#[derive(Debug, Clone)]
pub enum FilesDragMode {
    CheckboxSelect {
        target_selected: bool,
        visited_rows: HashSet<usize>,
    },
    RowFocus {
        visited_rows: HashSet<usize>,
    },
    ScrollbarThumb {
        grab_offset_rows: u16,
    },
}

#[derive(Debug, Clone)]
pub struct FilesDragState {
    pub mode: FilesDragMode,
    pub start_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopeFilesScrollbarGeometry {
    pub track_area: Rect,
    pub thumb_area: Rect,
    pub content_len: usize,
    pub viewport_len: usize,
    pub max_scroll: usize,
}

impl ScopeFilesScrollbarGeometry {
    fn clamped_track_row(&self, row: u16) -> usize {
        if self.track_area.height == 0 {
            return 0;
        }
        row.saturating_sub(self.track_area.y)
            .min(self.track_area.height.saturating_sub(1)) as usize
    }

    fn scroll_for_thumb_top_offset(&self, thumb_top_offset: usize) -> usize {
        let track_height = self.track_area.height as usize;
        let thumb_height = self.thumb_area.height.max(1) as usize;
        let max_thumb_top = track_height.saturating_sub(thumb_height);
        if max_thumb_top == 0 || self.max_scroll == 0 {
            return 0;
        }

        let clamped_offset = thumb_top_offset.min(max_thumb_top);
        (clamped_offset
            .saturating_mul(self.max_scroll)
            .saturating_add(max_thumb_top / 2))
            / max_thumb_top
    }

    pub fn scroll_for_track_row_centered(&self, row: u16) -> usize {
        let thumb_height = self.thumb_area.height.max(1) as usize;
        let centered_thumb_top = self.clamped_track_row(row).saturating_sub(thumb_height / 2);
        self.scroll_for_thumb_top_offset(centered_thumb_top)
    }

    pub fn scroll_for_thumb_top_row(&self, row: u16) -> usize {
        self.scroll_for_thumb_top_offset(row.saturating_sub(self.track_area.y) as usize)
    }

    pub fn thumb_grab_offset_for_row(&self, row: u16) -> u16 {
        if self.thumb_area.height == 0 {
            return 0;
        }
        row.saturating_sub(self.thumb_area.y)
            .min(self.thumb_area.height.saturating_sub(1))
    }
}

pub struct AppState {
    pub cwd: PathBuf,
    pub stage: Stage,
    pub scope_tab: ScopeTab,
    pub scope_left_tab: ScopeTab,
    pub scope_right_tab: ScopeTab,
    pub naming_tab: NamingTab,
    pub focus: FocusPane,
    pub focus_manager: FocusManager<FocusPane>,
    pub mode: InputMode,
    pub should_quit: bool,
    pub show_help: bool,
    pub show_hidden: bool,
    pub files_settings_open: bool,
    pub show_selected_categories_only: bool,
    pub files_preview_visible: bool,
    pub files_preview_status: FilesPreviewStatus,
    pub files_preview_target: Option<PathBuf>,
    pub files_preview_error: Option<String>,
    pub files_preview_source_meta: Option<PreviewSourceMeta>,
    pub files_preview_protocol: Option<StatefulProtocol>,
    pub files_preview_picker: Option<Picker>,
    pub files_preview_next_request_id: u64,
    pub files_preview_active_request_id: Option<u64>,
    pub files_preview_worker_tx: Option<Sender<PreviewWorkerCommand>>,
    pub files_preview_worker_rx: Option<Receiver<PreviewWorkerEvent>>,

    pub tree: FileTree,
    pub scan_error: Option<String>,

    pub category_filters: Vec<CategoryFilter>,
    pub selected_extensions: HashSet<String>,
    pub select_cursor: usize,
    pub new_category_input: String,

    pub time_constraint: TimeConstraint,
    pub size_constraint: SizeConstraint,
    pub constraint_cursor: usize,

    pub presets: Vec<Option<PresetState>>,
    pub preset_cursor: usize,

    pub marketplace_presets: Vec<MarketplacePreset>,
    pub marketplace_cursor: usize,

    pub rename_rows: Vec<RenameRow>,
    pub rename_cursor: usize,
    pub rename_scroll: usize,

    pub suggestions: Vec<String>,
    pub suggestion_cursor: usize,
    pub suggestion_set: Option<SuggestionSet>,

    pub style: StyleOptions,
    pub style_cursor: usize,

    pub command_input: String,
    pub command_history_nav: Option<usize>,
    pub command_history_draft: Option<String>,
    pub override_input: String,
    pub editing_row: Option<usize>,
    pub override_vocab: BTreeSet<String>,

    pub toasts: Vec<Toast>,
    pub logs: Vec<String>,
    pub show_log_overlay: bool,
    pub log_scroll: usize,
    pub log_view_height: usize,
    pub log_col_scroll: usize,
    pub log_view_width: usize,
    pub prompt_history: Vec<String>,
    pub show_prompt_history_overlay: bool,
    pub prompt_history_cursor: usize,
    pub prompt_history_scroll: usize,
    pub prompt_history_view_height: usize,
    pub undo_history: Vec<SessionUndoEntry>,
    pub apply_cursor: usize,
    pub history_cursor: usize,

    pub click_regions: ClickRegionRegistry<ClickTarget>,
    pub ticks: u64,
    pub help_lines: Vec<Line<'static>>,

    pub selected_by_tree: HashSet<PathBuf>,
    pub filter_cache: HashMap<PathBuf, bool>,
    pub category_match_dirs: HashSet<PathBuf>,
    pub scope_files_focus_nodes: HashSet<usize>,
    pub scope_files_drag: Option<FilesDragState>,
    pub scope_files_scrollbar: Option<ScopeFilesScrollbarGeometry>,
    pub title_startup_fx: Option<Effect>,
    pub title_fx_last_frame: Option<Instant>,
    pub ai_settings: AiSettingsState,
    pub naming_ai_status: NamingAiStatus,
    pub naming_ai_error: Option<String>,
    pub ai_analyzing_paths: HashSet<PathBuf>,
    pub ai_next_request_id: u64,
    pub ai_active_request_id: Option<u64>,
    pub ai_worker_tx: Option<Sender<AiWorkerCommand>>,
    pub ai_worker_rx: Option<Receiver<AiWorkerEvent>>,
}
