use crate::app::context::AppContext;
use crate::app::hosts_state::HostsState;
use crate::app::search_state::SearchState;
use crate::app::session::SessionState;
use crate::app::ui_state::UiState;

#[derive(Debug)]
pub struct App {
    pub should_quit: bool,
    pub ctx: AppContext,
    pub hosts: HostsState,
    pub search: SearchState,
    pub ui: UiState,
    pub session: SessionState,
}
