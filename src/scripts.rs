//! Lua script functions for interact
//!
//! Implements the Lua API:
//! - InteractNearest(autoloot) - Interact with the nearest valid object

use crate::game::{self, ObjectType};
use crate::lua::{self, LuaState};
use std::ffi::{c_char, c_int, c_void};

// =============================================================================
// Error Messages (null-terminated for C)
// =============================================================================

const ERR_USAGE: &[u8] = b"Usage: InteractNearest(autoloot)\0";

// =============================================================================
// Constants
// =============================================================================

/// Maximum interaction distance in yards
const MAX_DISTANCE: f32 = 5.0;

/// Initial "infinite" distance for comparisons
const INITIAL_DISTANCE: f32 = 1000.0;

// =============================================================================
// Candidate tracking
// =============================================================================

/// Tracks the best candidate for a given priority level
#[derive(Default)]
struct Candidate {
    guid: u64,
    pointer: u32,
    obj_type: ObjectType,
    distance: f32,
}

impl Candidate {
    fn new() -> Self {
        Self {
            guid: 0,
            pointer: 0,
            obj_type: ObjectType::None,
            distance: INITIAL_DISTANCE,
        }
    }

    fn is_valid(&self) -> bool {
        self.obj_type != ObjectType::None
    }

    fn update(&mut self, guid: u64, pointer: u32, obj_type: ObjectType, distance: f32) {
        if distance < self.distance {
            self.guid = guid;
            self.pointer = pointer;
            self.obj_type = obj_type;
            self.distance = distance;
        }
    }
}

// =============================================================================
// Script_InteractNearest
// =============================================================================
//
// Lua: InteractNearest(autoloot)
//
// Finds and interacts with the nearest valid object within 5 yards.
// Returns no values to Lua (matching original C behavior).
//
// Parameters:
//   autoloot - 0 for normal interact, non-zero for auto-loot (number)
//
// Priority order:
//   1. Lootable corpses (dead units with loot)
//   2. Game objects (chests, herbs, mining nodes, etc.)
//   3. Skinnable corpses (dead units without loot but skinnable)
//   4. Alive units (NPCs)

#[no_mangle]
pub unsafe extern "fastcall" fn Script_InteractNearest(_lua_state: LuaState) -> c_int {
    // Check if player is in world (early exit like C version)
    if !game::is_in_world() {
        return 0;
    }

    let lua = lua::api();
    let l = lua.get_state();

    // Validate arguments
    if !lua.isnumber(l, 1) {
        lua.error(l, ERR_USAGE.as_ptr() as *const c_char);
    }

    // Find the best candidate
    let Some((candidate, autoloot)) = find_best_candidate(lua, l) else {
        return 0;
    };

    // Perform the interaction
    match candidate.obj_type {
        ObjectType::Unit => {
            game::set_target(candidate.guid);
            game::interact_unit(candidate.pointer, autoloot);
        }
        ObjectType::GameObject => {
            game::interact_object(candidate.pointer, autoloot);
        }
        _ => return 0,
    }

    1 // Return value count (C version returns 1 on success, 0 on failure)
}

/// Find the best interaction candidate based on priority rules
unsafe fn find_best_candidate(lua: &crate::lua::LuaApi, l: LuaState) -> Option<(Candidate, i32)> {
    let autoloot = lua.tonumber(l, 1) as i32;

    // Get visible objects manager
    let objects = game::get_visible_objects();
    let player_guid = game::get_player_guid(objects);
    let player = game::get_object_pointer(player_guid);
    let player_pos = game::get_unit_position(player);

    // Candidates for each priority level
    let mut lootable = Candidate::new();
    let mut gameobject = Candidate::new();
    let mut skinnable = Candidate::new();
    let mut alive_unit = Candidate::new();

    // Get blacklist (static would be better, but matching C behavior)
    let blacklist = game::get_blacklist();

    // Iterate through all visible objects
    let mut current = game::get_first_object(objects);

    while current != 0 && (current & 1) == 0 {
        let guid = game::get_object_guid(current);
        let pointer = game::get_object_pointer(guid);
        let obj_type = game::get_object_type(pointer);

        // Skip objects summoned by players
        if is_player_summoned(pointer) {
            current = game::get_next_object(current);
            continue;
        }

        // Get position and calculate distance
        let obj_pos = match obj_type {
            ObjectType::Unit => game::get_unit_position(current),
            ObjectType::GameObject => game::get_object_position(current),
            _ => {
                current = game::get_next_object(current);
                continue;
            }
        };

        let distance = player_pos.distance(&obj_pos);

        // Check if within interaction range
        if distance <= MAX_DISTANCE {
            match obj_type {
                ObjectType::Unit => {
                    process_unit(
                        current,
                        guid,
                        obj_type,
                        distance,
                        &mut lootable,
                        &mut skinnable,
                        &mut alive_unit,
                    );
                }
                ObjectType::GameObject => {
                    let id = game::get_gameobject_id(pointer);
                    if !blacklist.contains(&id) {
                        gameobject.update(guid, pointer, obj_type, distance);
                    }
                }
                _ => {}
            }
        }

        current = game::get_next_object(current);
    }

    // Select by priority: lootable > gameobject > skinnable > alive
    let candidate = if lootable.is_valid() {
        lootable
    } else if gameobject.is_valid() {
        gameobject
    } else if skinnable.is_valid() {
        skinnable
    } else if alive_unit.is_valid() {
        alive_unit
    } else {
        return None;
    };

    Some((candidate, autoloot))
}

/// Check if an object was summoned by a player
unsafe fn is_player_summoned(pointer: u32) -> bool {
    let summoned_by_guid = game::get_summoned_by_guid(pointer);
    if summoned_by_guid == 0 {
        return false;
    }

    let summoned_by = game::get_object_pointer(summoned_by_guid);
    if summoned_by == 0 {
        return false;
    }

    game::get_object_type(summoned_by) == ObjectType::Player
}

/// Process a unit and update the appropriate candidate
unsafe fn process_unit(
    current: u32,
    guid: u64,
    obj_type: ObjectType,
    distance: f32,
    lootable: &mut Candidate,
    skinnable: &mut Candidate,
    alive_unit: &mut Candidate,
) {
    let health = game::get_unit_health(current);

    if health == 0 {
        // Dead unit - check lootable/skinnable
        let is_lootable = game::is_unit_lootable(current);
        let is_skinnable = game::is_unit_skinnable(current);

        if is_lootable {
            lootable.update(guid, current, obj_type, distance);
        } else if is_skinnable {
            skinnable.update(guid, current, obj_type, distance);
        }
    } else if health > 0 {
        // Alive unit
        alive_unit.update(guid, current, obj_type, distance);
    }
}

// =============================================================================
// Function Registration
// =============================================================================

/// Register all Lua functions with the game
pub unsafe fn register_functions() {
    let lua = lua::api();

    lua.register_function(
        b"InteractNearest\0".as_ptr() as *const c_char,
        Script_InteractNearest as *const c_void,
    );

    debug_log!("Registered InteractNearest function");
}
