use crate::app::App;

pub trait AppKeymapExt {
    fn get_current_group(&self) -> Option<&str>;
}

impl AppKeymapExt for App {
    fn get_current_group(&self) -> Option<&str> {
        self.groups.get(self.current_group_index).map(|s| s.as_str())
    }
}
