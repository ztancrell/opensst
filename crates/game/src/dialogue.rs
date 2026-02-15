//! Dialogue system for Earth settlement citizens — Starship Troopers flavor (Federation, service, propaganda).

use hecs::Entity;

/// One dialogue node: line of text and optional choices (label, next node index or None to close).
#[derive(Debug, Clone)]
pub struct DialogueNode {
    pub text: String,
    pub choices: Vec<(String, Option<usize>)>,
}

/// Predefined dialogue trees per dialogue_id (0..5). Advanced dialogue: topics and responses.
pub fn dialogue_content(dialogue_id: usize) -> Vec<DialogueNode> {
    let nodes: Vec<DialogueNode> = match dialogue_id {
        0 => vec![
            DialogueNode {
                text: "Citizen! Doing your part today? The Federation needs every hand.".to_string(),
                choices: vec![
                    ("What's the situation?".to_string(), Some(1)),
                    ("I'm with the MI. Hold the line.".to_string(), Some(2)),
                    ("Stay safe. Goodbye.".to_string(), None),
                ],
            },
            DialogueNode {
                text: "Bugs pushed back from the perimeter last week. We're rebuilding. Would do it again for Earth.".to_string(),
                choices: vec![("I'm with the MI. Hold the line.".to_string(), Some(2)), ("Stay safe.".to_string(), None)],
            },
            DialogueNode {
                text: "Thank you, trooper. We see the drop pods. Good hunting.".to_string(),
                choices: vec![("Goodbye.".to_string(), None)],
            },
        ],
        1 => vec![
            DialogueNode {
                text: "You're one of the Mobile Infantry? We heard the Roger Young was in orbit.".to_string(),
                choices: vec![
                    ("That's right. Defending the homeworld.".to_string(), Some(1)),
                    ("How's the colony holding up?".to_string(), Some(2)),
                    ("Carry on, citizen.".to_string(), None),
                ],
            },
            DialogueNode {
                text: "Earth is worth it. We're all doing our part.".to_string(),
                choices: vec![("Goodbye.".to_string(), None)],
            },
            DialogueNode {
                text: "We keep the power and water running. You keep the bugs off our doorstep.".to_string(),
                choices: vec![("We will. Goodbye.".to_string(), None)],
            },
        ],
        2 => vec![
            DialogueNode {
                text: "Service guarantees citizenship. You're living proof.".to_string(),
                choices: vec![
                    ("What do you do here?".to_string(), Some(1)),
                    ("Would you like to know more?".to_string(), Some(2)),
                    ("Goodbye.".to_string(), None),
                ],
            },
            DialogueNode {
                text: "Logistics. Food, ammo, repairs. The colony runs so you can fight.".to_string(),
                choices: vec![("Thank you. Goodbye.".to_string(), None)],
            },
            DialogueNode {
                text: "I'm from Buenos Aires, and I say kill 'em all!".to_string(),
                choices: vec![("Goodbye.".to_string(), None)],
            },
        ],
        3 => vec![
            DialogueNode {
                text: "Rough weather. Stay dry, trooper.".to_string(),
                choices: vec![
                    ("How often does it storm here?".to_string(), Some(1)),
                    ("You too. Goodbye.".to_string(), None),
                ],
            },
            DialogueNode {
                text: "More than we'd like. We get under cover. You get the bugs.".to_string(),
                choices: vec![("Goodbye.".to_string(), None)],
            },
        ],
        4 => vec![
            DialogueNode {
                text: "The only good bug is a dead bug. We're counting on you.".to_string(),
                choices: vec![
                    ("We'll hold the line.".to_string(), Some(1)),
                    ("Goodbye.".to_string(), None),
                ],
            },
            DialogueNode {
                text: "Earth is worth fighting for. We remember.".to_string(),
                choices: vec![("Goodbye.".to_string(), None)],
            },
        ],
        _ => vec![DialogueNode {
            text: "Citizen. Good day.".to_string(),
            choices: vec![("Goodbye.".to_string(), None)],
        }],
    };
    nodes
}

/// Current dialogue UI state.
#[derive(Debug, Clone)]
pub enum DialogueState {
    Closed,
    Open {
        speaker_entity: Entity,
        speaker_name: String,
        dialogue_id: usize,
        node_index: usize,
        showing_choices: bool,
    },
}

impl Default for DialogueState {
    fn default() -> Self {
        DialogueState::Closed
    }
}

impl DialogueState {
    pub fn is_open(&self) -> bool {
        matches!(self, DialogueState::Open { .. })
    }

    /// Get current line and choices for overlay.
    pub fn current_line_and_choices(&self) -> Option<(String, Vec<(String, Option<usize>)>)> {
        match self {
            DialogueState::Open {
                dialogue_id,
                node_index,
                ..
            } => {
                let nodes = dialogue_content(*dialogue_id);
                let node = nodes.get(*node_index)?;
                Some((
                    node.text.clone(),
                    node.choices.clone(),
                ))
            }
            _ => None,
        }
    }

    /// Select choice by index (0-based). Returns true if dialogue closed.
    pub fn select_choice(&mut self, choice_index: usize) -> bool {
        let (dialogue_id, node_index, speaker_entity, speaker_name) = match self {
            DialogueState::Open {
                speaker_entity,
                speaker_name,
                dialogue_id,
                node_index,
                ..
            } => (*dialogue_id, *node_index, *speaker_entity, speaker_name.clone()),
            _ => return true,
        };
        let nodes = dialogue_content(dialogue_id);
        let node = match nodes.get(node_index) {
            Some(n) => n,
            None => return true,
        };
        let (_, next) = match node.choices.get(choice_index) {
            Some(c) => c,
            None => return true,
        };
        match next {
            None => {
                *self = DialogueState::Closed;
                true
            }
            Some(next_idx) => {
                *self = DialogueState::Open {
                    speaker_entity,
                    speaker_name,
                    dialogue_id,
                    node_index: *next_idx,
                    showing_choices: true,
                };
                false
            }
        }
    }
}
