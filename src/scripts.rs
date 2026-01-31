//! Lua script functions for interact
//!
//! Implements the Lua API:
//! - InteractNearest(autoloot) - Interact with the nearest valid object

use crate::game::{self, ObjectType};
use crate::lua::{self, LuaState};
use std::ffi::{c_int, c_void};

// =============================================================================
// Error Messages (null-terminated for C)
// =============================================================================

const ERR_USAGE: &std::ffi::CStr = c"Usage: InteractNearest(autoloot)";

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
        lua.error(l, ERR_USAGE.as_ptr());
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
    let player = game::get_object_pointer(player_guid)?;
    let player_pos = game::get_unit_position(player.get());

    // Candidates for each priority level
    let mut lootable = Candidate::new();
    let mut gameobject = Candidate::new();
    let mut skinnable = Candidate::new();
    let mut alive_unit = Candidate::new();

    // Blacklist is now lazily initialized - no allocation per call

    // Iterate through all visible objects.
    // The object manager uses a linked list where:
    // - current == 0 indicates end of list (null pointer)
    // - (current & 1) != 0 indicates an invalid/sentinel pointer
    //   WoW uses the low bit as a tag to mark end-of-list or invalid entries,
    //   since valid object pointers are always aligned (low bits are 0).
    let mut current = game::get_first_object(objects);

    while current != 0 && (current & 1) == 0 {
        let guid = game::get_object_guid(current);
        let Some(pointer) = game::get_object_pointer(guid) else {
            current = game::get_next_object(current);
            continue;
        };
        let pointer_raw = pointer.get();
        let obj_type = game::get_object_type(pointer_raw);

        // Skip objects summoned by players
        if is_player_summoned(pointer_raw) {
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
                    let id = game::get_gameobject_id(pointer_raw);
                    if !game::is_blacklisted(id) {
                        gameobject.update(guid, pointer_raw, obj_type, distance);
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

    let Some(summoned_by) = game::get_object_pointer(summoned_by_guid) else {
        return false;
    };

    game::get_object_type(summoned_by.get()) == ObjectType::Player
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
        c"InteractNearest".as_ptr(),
        Script_InteractNearest as *const c_void,
    );

    debug_log!("Registered InteractNearest function");
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // Exact float comparisons are intentional in these tests

    use super::*;

    // -------------------------------------------------------------------------
    // Candidate tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_candidate_new_is_invalid() {
        let c = Candidate::new();
        assert!(!c.is_valid());
        assert_eq!(c.obj_type, ObjectType::None);
        assert_eq!(c.distance, INITIAL_DISTANCE);
    }

    #[test]
    fn test_candidate_update_makes_valid() {
        let mut c = Candidate::new();
        c.update(123, 456, ObjectType::Unit, 3.0);

        assert!(c.is_valid());
        assert_eq!(c.guid, 123);
        assert_eq!(c.pointer, 456);
        assert_eq!(c.obj_type, ObjectType::Unit);
        assert_eq!(c.distance, 3.0);
    }

    #[test]
    fn test_candidate_update_closer_replaces() {
        let mut c = Candidate::new();
        c.update(100, 200, ObjectType::Unit, 5.0);
        c.update(101, 201, ObjectType::Unit, 3.0);

        // Should have the closer one
        assert_eq!(c.guid, 101);
        assert_eq!(c.pointer, 201);
        assert_eq!(c.distance, 3.0);
    }

    #[test]
    fn test_candidate_update_farther_ignored() {
        let mut c = Candidate::new();
        c.update(100, 200, ObjectType::Unit, 3.0);
        c.update(101, 201, ObjectType::Unit, 5.0);

        // Should still have the closer one
        assert_eq!(c.guid, 100);
        assert_eq!(c.pointer, 200);
        assert_eq!(c.distance, 3.0);
    }

    #[test]
    fn test_candidate_update_same_distance_ignored() {
        let mut c = Candidate::new();
        c.update(100, 200, ObjectType::Unit, 3.0);
        c.update(101, 201, ObjectType::Unit, 3.0);

        // First one should win (not strictly less than)
        assert_eq!(c.guid, 100);
        assert_eq!(c.pointer, 200);
    }

    #[test]
    fn test_candidate_is_valid_check() {
        let mut c = Candidate::new();
        assert!(!c.is_valid());

        c.obj_type = ObjectType::Unit;
        assert!(c.is_valid());

        c.obj_type = ObjectType::GameObject;
        assert!(c.is_valid());

        c.obj_type = ObjectType::None;
        assert!(!c.is_valid());
    }

    // -------------------------------------------------------------------------
    // Constants tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_max_distance_is_5_yards() {
        assert_eq!(MAX_DISTANCE, 5.0);
    }

    #[test]
    fn test_initial_distance_is_large() {
        // Use const block for compile-time assertion
        const _: () = assert!(INITIAL_DISTANCE > MAX_DISTANCE);
        assert_eq!(INITIAL_DISTANCE, 1000.0);
    }

    #[test]
    fn test_error_message_is_valid_cstr() {
        // CStr is guaranteed to be null-terminated, so we just verify it's valid
        assert!(!ERR_USAGE.to_bytes().is_empty());
        assert!(ERR_USAGE.to_str().is_ok());
    }

    // -------------------------------------------------------------------------
    // Priority selection tests (simulated)
    // -------------------------------------------------------------------------

    #[test]
    fn test_priority_lootable_wins_over_all() {
        let mut lootable = Candidate::new();
        let mut gameobject = Candidate::new();
        let mut skinnable = Candidate::new();
        let mut alive = Candidate::new();

        lootable.update(1, 100, ObjectType::Unit, 4.0);
        gameobject.update(2, 200, ObjectType::GameObject, 2.0);
        skinnable.update(3, 300, ObjectType::Unit, 1.0);
        alive.update(4, 400, ObjectType::Unit, 0.5);

        // Simulate priority selection
        let winner = if lootable.is_valid() {
            &lootable
        } else if gameobject.is_valid() {
            &gameobject
        } else if skinnable.is_valid() {
            &skinnable
        } else {
            &alive
        };

        assert_eq!(winner.guid, 1); // Lootable wins even though farther
    }

    #[test]
    fn test_priority_gameobject_wins_over_skinnable_and_alive() {
        let lootable = Candidate::new();
        let mut gameobject = Candidate::new();
        let mut skinnable = Candidate::new();
        let mut alive = Candidate::new();

        // No lootable
        gameobject.update(2, 200, ObjectType::GameObject, 4.0);
        skinnable.update(3, 300, ObjectType::Unit, 2.0);
        alive.update(4, 400, ObjectType::Unit, 1.0);

        let winner = if lootable.is_valid() {
            &lootable
        } else if gameobject.is_valid() {
            &gameobject
        } else if skinnable.is_valid() {
            &skinnable
        } else {
            &alive
        };

        assert_eq!(winner.guid, 2); // GameObject wins
    }

    #[test]
    fn test_priority_skinnable_wins_over_alive() {
        let lootable = Candidate::new();
        let gameobject = Candidate::new();
        let mut skinnable = Candidate::new();
        let mut alive = Candidate::new();

        // No lootable or gameobject
        skinnable.update(3, 300, ObjectType::Unit, 4.0);
        alive.update(4, 400, ObjectType::Unit, 1.0);

        let winner = if lootable.is_valid() {
            &lootable
        } else if gameobject.is_valid() {
            &gameobject
        } else if skinnable.is_valid() {
            &skinnable
        } else {
            &alive
        };

        assert_eq!(winner.guid, 3); // Skinnable wins
    }

    #[test]
    fn test_priority_alive_is_last_resort() {
        let lootable = Candidate::new();
        let gameobject = Candidate::new();
        let skinnable = Candidate::new();
        let mut alive = Candidate::new();

        // Only alive unit
        alive.update(4, 400, ObjectType::Unit, 1.0);

        let winner = if lootable.is_valid() {
            &lootable
        } else if gameobject.is_valid() {
            &gameobject
        } else if skinnable.is_valid() {
            &skinnable
        } else {
            &alive
        };

        assert_eq!(winner.guid, 4);
    }

    #[test]
    fn test_no_candidates_returns_none() {
        let lootable = Candidate::new();
        let gameobject = Candidate::new();
        let skinnable = Candidate::new();
        let alive = Candidate::new();

        let has_winner = lootable.is_valid()
            || gameobject.is_valid()
            || skinnable.is_valid()
            || alive.is_valid();

        assert!(!has_winner);
    }
}
