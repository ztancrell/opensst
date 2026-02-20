//! Window and device event handling for GameState.
//! Extracted from main.rs to keep the event loop and input handling in one place.

use glam::{Quat, Vec3};
use engine_core::{Health, Transform, Velocity};
use winit::event::{DeviceEvent, WindowEvent};
use winit::keyboard::KeyCode;
use winit::window::CursorGrabMode;

use crate::bug::{Bug, BugType};
use crate::bug_entity::PhysicsBug;
use crate::state::{GamePhase, WarpSequence};

impl crate::GameState {
    /// Handle a window event. Returns true if the app should exit.
    pub(crate) fn handle_window_event(&mut self, event: WindowEvent) -> bool {
        match event {
            WindowEvent::CloseRequested => {
                self.running = false;
                true
            }
            WindowEvent::Resized(size) => {
                self.renderer.resize(size);
                self.camera.set_aspect(size.width, size.height);
                false
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let winit::keyboard::PhysicalKey::Code(key) = event.physical_key {
                    self.input.process_keyboard(key, event.state);

                    if key == KeyCode::Escape && event.state.is_pressed() {
                        if self.phase == GamePhase::Paused {
                            if self.pause_menu_selected == 0 {
                                if let Some(prev) = self.previous_phase.take() {
                                    self.phase = prev;
                                    let _ = self.renderer.window.set_cursor_grab(CursorGrabMode::Locked)
                                        .or_else(|_| self.renderer.window.set_cursor_grab(CursorGrabMode::Confined));
                                    self.renderer.window.set_cursor_visible(false);
                                    self.input.set_cursor_locked(true);
                                }
                            }
                        } else if self.phase == GamePhase::Playing || self.phase == GamePhase::InShip {
                            self.previous_phase = Some(self.phase);
                            self.phase = GamePhase::Paused;
                            self.pause_menu_selected = 0;
                            let _ = self.renderer.window.set_cursor_grab(CursorGrabMode::None);
                            self.renderer.window.set_cursor_visible(true);
                            self.input.set_cursor_locked(false);
                        } else {
                            let _ = self.renderer.window.set_cursor_grab(CursorGrabMode::None);
                            self.renderer.window.set_cursor_visible(true);
                            self.input.set_cursor_locked(false);
                        }
                    }

                    if self.phase == GamePhase::Paused && event.state.is_pressed() {
                        match key {
                            KeyCode::ArrowUp | KeyCode::KeyW => {
                                self.pause_menu_selected = self.pause_menu_selected.saturating_sub(1);
                            }
                            KeyCode::ArrowDown | KeyCode::KeyS => {
                                self.pause_menu_selected = (self.pause_menu_selected + 1).min(1);
                            }
                            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => {
                                if self.pause_menu_selected == 0 {
                                    if let Some(prev) = self.previous_phase.take() {
                                        self.phase = prev;
                                        let _ = self.renderer.window.set_cursor_grab(CursorGrabMode::Locked)
                                            .or_else(|_| self.renderer.window.set_cursor_grab(CursorGrabMode::Confined));
                                        self.renderer.window.set_cursor_visible(false);
                                        self.input.set_cursor_locked(true);
                                    }
                                } else {
                                    self.transition_to_main_menu();
                                }
                            }
                            _ => {}
                        }
                    }

                    if key == KeyCode::F1 && event.state.is_pressed() {
                        for _ in 0..10 {
                            let angle = rand::random::<f32>() * std::f32::consts::TAU;
                            let dist = 15.0 + rand::random::<f32>() * 20.0;
                            let pos = self.player.position + Vec3::new(angle.cos() * dist, 0.5, angle.sin() * dist);

                            let (bug_type, variant) = self.random_bug_type();
                            let bug = Bug::new_with_variant(bug_type, variant);
                            let scale = bug_type.scale();
                            let body_handle = self.physics.add_kinematic_body(pos);
                            let collider_handle = self.physics.add_capsule_collider(body_handle, scale.y * 0.5, scale.x * 0.5);

                            self.world.spawn((
                                Transform { position: pos, rotation: Quat::IDENTITY, scale },
                                Velocity::default(),
                                Health::new(bug.effective_health()),
                                bug,
                                PhysicsBug {
                                    body_handle: Some(body_handle),
                                    collider_handle: Some(collider_handle),
                                    ..Default::default()
                                },
                                engine_core::AIComponent::new(85.0, 2.5, 1.0),
                            ));
                        }
                        #[cfg(debug_assertions)]
                        self.game_messages.info("Spawned 10 debug bugs!");
                    }

                    if key == KeyCode::F2 && event.state.is_pressed() {
                        self.player.heal(50.0);
                        self.player.add_armor(25.0);
                        #[cfg(debug_assertions)]
                        self.game_messages.info("Debug heal applied!");
                    }

                    if key == KeyCode::F3 && event.state.is_pressed() {
                        self.debug.menu_open = !self.debug.menu_open;
                        if self.debug.menu_open {
                            #[cfg(debug_assertions)]
                            self.game_messages.info("[DEBUG] Debug menu opened (Arrow keys + Enter)");
                        }
                    }

                    if self.debug.menu_open && event.state.is_pressed() {
                        match key {
                            KeyCode::ArrowUp => {
                                if self.debug.selected > 0 {
                                    self.debug.selected -= 1;
                                } else {
                                    self.debug.selected = self.debug.menu_item_count() - 1;
                                }
                            }
                            KeyCode::ArrowDown => {
                                self.debug.selected = (self.debug.selected + 1) % self.debug.menu_item_count();
                            }
                            KeyCode::Enter | KeyCode::NumpadEnter => {
                                self.debug.toggle_selected();
                                let items = self.debug.menu_items();
                                if let Some((name, val)) = items.get(self.debug.selected) {
                                    #[cfg(debug_assertions)]
                                    {
                                        if name.starts_with("--") {
                                            self.game_messages.info(format!("[DEBUG] {}", name.trim_matches('-').trim()));
                                        } else {
                                            self.game_messages.info(format!("[DEBUG] {} = {}", name, if *val { "ON" } else { "OFF" }));
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    if key == KeyCode::KeyR && event.state.is_pressed() && !self.galaxy_map_open
                        && self.phase != GamePhase::InShip && self.phase != GamePhase::ApproachPlanet && self.phase != GamePhase::DropSequence
                        && (self.debug.noclip || self.current_planet_idx.is_none())
                    {
                        self.regenerate_planet();
                    }

                    if key == KeyCode::KeyM && event.state.is_pressed()
                        && self.phase != GamePhase::DropSequence && self.phase != GamePhase::InShip && self.phase != GamePhase::ApproachPlanet
                    {
                        self.galaxy_map_open = !self.galaxy_map_open;
                        if self.galaxy_map_open {
                            self.galaxy_map_selected = self.current_system_idx;
                        }
                    }

                    if self.galaxy_map_open && event.state.is_pressed() {
                        let num_systems = self.universe.systems.len();
                        match key {
                            KeyCode::ArrowRight | KeyCode::ArrowDown => {
                                self.galaxy_map_selected = (self.galaxy_map_selected + 1) % num_systems;
                            }
                            KeyCode::ArrowLeft | KeyCode::ArrowUp => {
                                self.galaxy_map_selected = if self.galaxy_map_selected == 0 {
                                    num_systems - 1
                                } else {
                                    self.galaxy_map_selected - 1
                                };
                            }
                            KeyCode::Enter | KeyCode::NumpadEnter => {
                                if self.current_planet_idx.is_some() {
                                    self.game_messages.warning("Must be in orbit to initiate warp drive!".to_string());
                                } else if self.galaxy_map_selected != self.current_system_idx {
                                    let target = self.galaxy_map_selected;
                                    let target_name = self.universe.systems[target].name.clone();
                                    self.game_messages.warning(format!("Initiating warp to {}...", target_name));
                                    self.warp_sequence = Some(WarpSequence::new(target));
                                    self.galaxy_map_open = false;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                false
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.input.process_mouse_button(button, state);

                if self.phase == GamePhase::MainMenu {
                    return false;
                }
                if state.is_pressed() && !self.input.is_cursor_locked() {
                    let _ = self.renderer.window
                        .set_cursor_grab(CursorGrabMode::Locked)
                        .or_else(|_| self.renderer.window.set_cursor_grab(CursorGrabMode::Confined));
                    self.renderer.window.set_cursor_visible(false);
                    self.input.set_cursor_locked(true);
                }
                false
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.input.process_cursor_position((position.x, position.y));
                false
            }
            WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => {
                        if y > 0.0 { self.input.set_scroll_up(); }
                        else if y < 0.0 { self.input.set_scroll_down(); }
                    }
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        if pos.y > 0.0 { self.input.set_scroll_up(); }
                        else if pos.y < 0.0 { self.input.set_scroll_down(); }
                    }
                }
                false
            }
            WindowEvent::RedrawRequested => {
                self.update();
                if let Err(e) = self.render() {
                    log::error!("Render error: {}", e);
                }
                self.renderer.window.request_redraw();
                false
            }
            _ => false,
        }
    }

    /// Handle device events (e.g. raw mouse motion).
    pub(crate) fn handle_device_event(&mut self, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.input.is_cursor_locked() {
                self.input.process_mouse_motion(delta);
            }
        }
    }
}
