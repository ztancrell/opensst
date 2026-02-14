//! Audio system using Kira for spatial sound.

use anyhow::Result;
use engine_core::Vec3;
use kira::{
    manager::{AudioManager, AudioManagerSettings, backend::DefaultBackend},
    sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
    spatial::{
        emitter::{EmitterHandle, EmitterSettings},
        listener::{ListenerHandle, ListenerSettings},
        scene::{SpatialSceneHandle, SpatialSceneSettings},
    },
    tween::Tween,
};
use std::collections::HashMap;
use std::path::Path;

/// Main audio system managing sounds and spatial audio.
pub struct AudioSystem {
    manager: AudioManager,
    spatial_scene: SpatialSceneHandle,
    listener: ListenerHandle,
    sounds: HashMap<String, StaticSoundData>,
    active_sounds: Vec<StaticSoundHandle>,
}

impl AudioSystem {
    /// Create a new audio system.
    pub fn new() -> Result<Self> {
        let mut manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())?;
        
        let mut spatial_scene = manager.add_spatial_scene(SpatialSceneSettings::default())?;
        
        let listener = spatial_scene.add_listener(
            mint::Vector3 { x: 0.0, y: 0.0, z: 0.0 },
            mint::Quaternion { v: mint::Vector3 { x: 0.0, y: 0.0, z: 0.0 }, s: 1.0 },
            ListenerSettings::default(),
        )?;

        Ok(Self {
            manager,
            spatial_scene,
            listener,
            sounds: HashMap::new(),
            active_sounds: Vec::new(),
        })
    }

    /// Load a sound from a file.
    pub fn load_sound(&mut self, name: &str, path: &Path) -> Result<()> {
        let sound_data = StaticSoundData::from_file(path)?;
        self.sounds.insert(name.to_string(), sound_data);
        Ok(())
    }

    /// Load a sound from bytes.
    pub fn load_sound_from_bytes(&mut self, name: &str, data: &'static [u8]) -> Result<()> {
        let cursor = std::io::Cursor::new(data);
        let sound_data = StaticSoundData::from_cursor(cursor)?;
        self.sounds.insert(name.to_string(), sound_data);
        Ok(())
    }

    /// Play a 2D sound (UI, music).
    pub fn play(&mut self, name: &str) -> Result<()> {
        if let Some(sound_data) = self.sounds.get(name) {
            let handle = self.manager.play(sound_data.clone())?;
            self.active_sounds.push(handle);
        }
        Ok(())
    }

    /// Play a sound with volume control.
    pub fn play_with_volume(&mut self, name: &str, volume: f64) -> Result<()> {
        if let Some(sound_data) = self.sounds.get(name) {
            let settings = StaticSoundSettings::new().volume(volume);
            let modified = sound_data.clone().with_settings(settings);
            let handle = self.manager.play(modified)?;
            self.active_sounds.push(handle);
        }
        Ok(())
    }

    /// Create a spatial emitter at a position.
    pub fn create_emitter(&mut self, position: Vec3) -> Result<EmitterHandle> {
        let emitter = self.spatial_scene.add_emitter(
            mint::Vector3 { x: position.x, y: position.y, z: position.z },
            EmitterSettings::default(),
        )?;
        Ok(emitter)
    }

    /// Play a sound at a 3D position.
    pub fn play_at_position(&mut self, name: &str, position: Vec3) -> Result<()> {
        // Clone the sound data first to avoid borrow conflict
        let sound_data = self.sounds.get(name).cloned();
        if let Some(sound_data) = sound_data {
            let emitter = self.create_emitter(position)?;
            let settings = StaticSoundSettings::new()
                .output_destination(&emitter);
            let modified = sound_data.with_settings(settings);
            let handle = self.manager.play(modified)?;
            self.active_sounds.push(handle);
            // Note: emitter will be dropped, but sound continues playing
            // For persistent emitters, store them elsewhere
        }
        Ok(())
    }

    /// Update listener position and orientation (call each frame).
    pub fn update_listener(&mut self, position: Vec3, forward: Vec3, up: Vec3) {
        // Compute orientation quaternion from forward and up vectors
        let right = forward.cross(up).normalize();
        let corrected_up = right.cross(forward).normalize();
        
        // Build rotation matrix and convert to quaternion
        let rotation = glam::Mat3::from_cols(right, corrected_up, -forward);
        let quat = glam::Quat::from_mat3(&rotation);

        self.listener.set_position(
            mint::Vector3 { x: position.x, y: position.y, z: position.z },
            Tween::default(),
        );
        self.listener.set_orientation(
            mint::Quaternion { 
                v: mint::Vector3 { x: quat.x, y: quat.y, z: quat.z }, 
                s: quat.w 
            },
            Tween::default(),
        );
    }

    /// Clean up finished sounds.
    pub fn cleanup(&mut self) {
        self.active_sounds.retain(|handle| handle.state() != kira::sound::PlaybackState::Stopped);
    }

    /// Stop all sounds.
    pub fn stop_all(&mut self) {
        for handle in &mut self.active_sounds {
            let _ = handle.stop(Tween::default());
        }
        self.active_sounds.clear();
    }

    /// Set master volume (0.0 to 1.0).
    pub fn set_master_volume(&mut self, volume: f64) {
        let _ = self.manager.main_track().set_volume(volume, Tween::default());
    }
}

// Re-export for convenience
pub use kira;
