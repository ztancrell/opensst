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
        // 5–9: Roger Young crew (Fleet, FleetOfficer, MobileInfantry, Marauder, Johnny Rico)
        5 => vec![
            DialogueNode {
                text: "Ship's running smooth. War table's that way if you're dropping.".to_string(),
                choices: vec![
                    ("What's our status?".to_string(), Some(1)),
                    ("Carry on.".to_string(), None),
                ],
            },
            DialogueNode {
                text: "All systems nominal. Pick a planet, get your mission, and head to the bay.".to_string(),
                choices: vec![("Thanks.".to_string(), None)],
            },
        ],
        6 => vec![
            DialogueNode {
                text: "We hold the line so you can drop. Don't make our job harder.".to_string(),
                choices: vec![
                    ("What's the word from Fleet?".to_string(), Some(1)),
                    ("Understood. Good hunting.".to_string(), None),
                ],
            },
            DialogueNode {
                text: "Same as always: kill bugs, hold ground, extract when you're done.".to_string(),
                choices: vec![("Goodbye.".to_string(), None)],
            },
        ],
        7 => vec![
            DialogueNode {
                text: "Ready to drop, trooper? War table's where you pick the mission.".to_string(),
                choices: vec![
                    ("Would you like to know more?".to_string(), Some(1)),
                    ("See you on the surface.".to_string(), None),
                ],
            },
            DialogueNode {
                text: "I'm from Buenos Aires, and I say kill 'em all!".to_string(),
                choices: vec![("Goodbye.".to_string(), None)],
            },
        ],
        8 => vec![
            DialogueNode {
                text: "Marauder squad's on standby. You need fire support, we're there.".to_string(),
                choices: vec![
                    ("What's the loadout?".to_string(), Some(1)),
                    ("Good to know. Thanks.".to_string(), None),
                ],
            },
            DialogueNode {
                text: "Heavy armor, heavy guns. We punch holes; you fill 'em.".to_string(),
                choices: vec![("Goodbye.".to_string(), None)],
            },
        ],
        9 => vec![
            DialogueNode {
                text: "Rico. Pick your planet and mission at the war table. Drop bay's aft.".to_string(),
                choices: vec![
                    ("What's our priority?".to_string(), Some(1)),
                    ("We'll get it done.".to_string(), None),
                ],
            },
            DialogueNode {
                text: "Same as always: protect the Federation. Now move out.".to_string(),
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
        /// None when talking to a ship NPC (Roger Young crew); Some when talking to an Earth citizen.
        speaker_entity: Option<Entity>,
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
