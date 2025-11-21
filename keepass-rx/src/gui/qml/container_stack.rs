use std::str::FromStr;

use actor_macro::observing_model;
use qmetaobject::prelude::*;
use uuid::Uuid;

use crate::gui::RxViewMode;
use crate::rx::virtual_hierarchy::VirtualHierarchy;

use crate::gui::instructions::get_instructions;

#[observing_model]
#[derive(QObject, Default)]
#[allow(non_snake_case)]
pub struct RxUiContainerStack {
    base: qt_base_class!(trait QObject),

    /// The stack of containers (group/entry/etc) identified by UUID.
    container_stack: Vec<Uuid>,

    view_mode: RxViewMode,
    viewMode: qt_property!(RxViewMode; READ get_view_mode WRITE set_view_mode NOTIFY viewModeChanged),

    /// QML-visible property of the stack, that returns current top of
    /// the stack.
    containerUuid: qt_property!(QString; READ get_current_container NOTIFY containerChanged),

    containerName: qt_property!(QString; NOTIFY containerNameChanged),
    instructions: qt_property!(QString; NOTIFY instructionsChanged),
    isAtRoot: qt_property!(bool; READ is_at_root NOTIFY isAtRootChanged),

    // Signals
    containerChanged: qt_signal!(container_uuid: QString),
    containerNameChanged: qt_signal!(),
    instructionsChanged: qt_signal!(),
    isAtRootChanged: qt_signal!(),
    viewModeChanged: qt_signal!(value: RxViewMode),

    // Control over the UI
    pushContainer: qt_method!(fn(&mut self, container_uuid: QString)),
    popContainer: qt_method!(fn(&mut self)),
}

/// What group/template container we are in. Used in conjunction with
/// RxViewMode to determine if we should be able to travel back up the
/// tree and so on.
#[allow(non_snake_case)]
impl RxUiContainerStack {
    pub fn init_from_view(&mut self, view: &dyn VirtualHierarchy) {
        println!("Init from view for container stack");
        self.containerUuid = QString::from(view.root().uuid().to_string());
        self.containerName = QString::from(view.root().root_name());
        self.instructions = get_instructions(&view.feature())
            .map(QString::from)
            .unwrap_or_default();

        self.containerChanged(self.containerUuid.clone());
        self.containerNameChanged();
        self.instructionsChanged();
        self.isAtRootChanged();
    }

    pub fn get_view_mode(&self) -> RxViewMode {
        self.view_mode
    }

    pub fn set_view_mode(&mut self, value: RxViewMode) {
        self.reinit();

        if self.viewMode != value {
            self.viewMode = value;
            self.viewModeChanged(value);
        }
    }

    pub fn is_at_root(&self) -> bool {
        let at_root_single_check = || {
            let app_state = self._app.as_pinned().expect("No app state");
            let app_state = app_state.borrow();
            let view = app_state.curr_view().expect("No view?");

            view.root().uuid() == *self.container_stack.first().unwrap()
        };

        self.container_stack.len() == 0 || at_root_single_check()
    }

    pub fn get_current_container(&self) -> QString {
        self.container_stack
            .last()
            .map(|uuid| QString::from(uuid.to_string()))
            .unwrap_or_default()
    }

    pub fn pushContainer(&mut self, container_uuid: QString) {
        let new_uuid = Uuid::from_str(&container_uuid.to_string()).expect("Invalid UUID");
        let app_state = self._app.as_pinned().expect("No app state");
        let app_state = app_state.borrow();
        let view = app_state.curr_view().expect("No view?");
        let was_at_root = self.is_at_root();

        if let Some(container) = view.get(new_uuid) {
            self.container_stack.push(container.uuid());
            self.containerChanged(container_uuid);

            let container_name = QString::from(container.name());

            if self.containerName != container_name {
                self.containerName = container_name;
                self.containerNameChanged();
            }

            if was_at_root != self.is_at_root() {
                self.isAtRootChanged();
            }

            if let Some(instructions) = get_instructions(&view.feature()) {
                let instructions = QString::from(instructions);
                if instructions != self.instructions {
                    self.instructions = instructions;
                    self.instructionsChanged();
                }
            }
        } else {
            println!("Could not find container in view: {}", container_uuid);
        }
    }

    pub fn popContainer(&mut self) {
        let app_state = self._app.as_pinned().expect("No app state");
        let app_state = app_state.borrow();
        let view = app_state.curr_view().expect("No view?");
        let was_at_root = self.is_at_root();

        if let Some(prev_container_uuid) = self.container_stack.pop() {
            let parent_uuid = self.container_stack.last().cloned();
            let new_uuid = parent_uuid.unwrap_or_else(|| view.root().uuid());
            let new_container = view.get(new_uuid);

            // No need to assign because self.container is a dynamic
            // property.
            if new_uuid != prev_container_uuid {
                self.containerChanged(QString::from(new_uuid.to_string()));
            }

            let container_name = QString::from(
                new_container
                    .map(|c| c.name())
                    .map(QString::from)
                    .unwrap_or_else(|| QString::from("Unknown Container".to_string())),
            );

            if self.containerName != container_name {
                self.containerName = container_name;
                self.containerNameChanged();
            }

            if was_at_root != self.is_at_root() {
                self.isAtRootChanged();
            }

            if let Some(instructions) = get_instructions(&view.feature()) {
                let instructions = QString::from(instructions);
                if instructions != self.instructions {
                    self.instructions = instructions;
                    self.instructionsChanged();
                }
            }
        } else {
            println!("Can't go above root!");
        }
    }
}
