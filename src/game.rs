//! Game interface for WoW 1.12.1
//!
//! Provides functions to interact with game objects, units, and the game world.
//!
//! Note on unit position: We read directly from unit + 0x9B8/0x9BC/0x9C0.
//! UnitXP uses an alternative approach via CMovement (unit + 0x118) + 0x10,
//! which handles transport coordinates. Our direct method matches the
//! original Interact C implementation.

use crate::offsets;
use std::collections::HashSet;
use std::mem::transmute;

// =============================================================================
// Types
// =============================================================================

/// Object types in WoW
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObjectType {
    #[default]
    None = 0,
    Item = 1,
    Container = 2,
    Unit = 3,
    Player = 4,
    GameObject = 5,
    DynamicObject = 6,
    Corpse = 7,
}

impl From<u32> for ObjectType {
    fn from(value: u32) -> Self {
        match value {
            1 => Self::Item,
            2 => Self::Container,
            3 => Self::Unit,
            4 => Self::Player,
            5 => Self::GameObject,
            6 => Self::DynamicObject,
            7 => Self::Corpse,
            _ => Self::None,
        }
    }
}

/// 3D Vector for positions
/// Note: In WoW's coordinate system, Y comes before X in memory layout
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct C3Vector {
    pub y: f32,
    pub x: f32,
    pub z: f32,
}

impl C3Vector {
    /// Calculate 3D Euclidean distance to another point
    #[inline]
    pub fn distance(&self, other: &C3Vector) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        let dz = other.z - self.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

// =============================================================================
// Blacklisted Object IDs
// =============================================================================

/// Blacklisted game object IDs that should not be auto-interacted with
const BLACKLISTED_OBJECTS: &[u32] = &[179830, 179831, 179785, 179786];

/// Get the set of blacklisted game object IDs
pub fn get_blacklist() -> HashSet<u32> {
    BLACKLISTED_OBJECTS.iter().copied().collect()
}

// =============================================================================
// Game Function Types
// =============================================================================

type GetObjectPointerFn = unsafe extern "fastcall" fn(u64) -> u32;
type SetTargetFn = unsafe extern "stdcall" fn(u64);
type RightClickFn = unsafe extern "thiscall" fn(u32, i32);

// =============================================================================
// Memory offset helpers
// =============================================================================

/// Read a value from a memory address
#[inline]
unsafe fn read<T: Copy>(addr: u32) -> T {
    *(addr as *const T)
}

/// Read a value at base + offset
#[inline]
unsafe fn read_offset<T: Copy>(base: u32, offset: u32) -> T {
    *((base + offset) as *const T)
}

// =============================================================================
// Game State
// =============================================================================

/// Check if the player is currently in the game world
#[inline]
pub unsafe fn is_in_world() -> bool {
    read::<u8>(offsets::game::IS_IN_WORLD as u32) != 0
}

/// Get the visible objects manager pointer
#[inline]
pub unsafe fn get_visible_objects() -> u32 {
    read::<u32>(offsets::game::VISIBLE_OBJECTS as u32)
}

// =============================================================================
// Object Accessors
// =============================================================================

/// Get a pointer to a game object by its GUID
#[inline]
pub unsafe fn get_object_pointer(guid: u64) -> u32 {
    let func: GetObjectPointerFn = transmute(offsets::game::GET_OBJECT_POINTER);
    func(guid)
}

/// Get the player's GUID from the visible objects manager
#[inline]
pub unsafe fn get_player_guid(objects: u32) -> u64 {
    read_offset(objects, 0xC0)
}

/// Get the first object in the visible objects list
#[inline]
pub unsafe fn get_first_object(objects: u32) -> u32 {
    read_offset(objects, 0xAC)
}

/// Get the next object in the linked list
#[inline]
pub unsafe fn get_next_object(current: u32) -> u32 {
    read_offset(current, 0x3C)
}

/// Get the GUID of an object from its list entry
#[inline]
pub unsafe fn get_object_guid(current: u32) -> u64 {
    read_offset(current, 0x30)
}

/// Get the object type from a pointer
#[inline]
pub unsafe fn get_object_type(pointer: u32) -> ObjectType {
    ObjectType::from(read_offset::<u32>(pointer, 0x14))
}

/// Get the "summoned by" GUID for an object
#[inline]
pub unsafe fn get_summoned_by_guid(pointer: u32) -> u64 {
    let descriptor: u32 = read_offset(pointer, 0x8);
    read_offset(descriptor, 0x30)
}

/// Get the game object ID
#[inline]
pub unsafe fn get_gameobject_id(pointer: u32) -> u32 {
    read_offset(pointer, 0x294)
}

// =============================================================================
// Unit Functions
// =============================================================================

/// Get the position of a unit
#[inline]
pub unsafe fn get_unit_position(unit: u32) -> C3Vector {
    C3Vector {
        y: read_offset(unit, 0x09B8),
        x: read_offset(unit, 0x09BC),
        z: read_offset(unit, 0x09C0),
    }
}

/// Get the position of a game object
#[inline]
pub unsafe fn get_object_position(pointer: u32) -> C3Vector {
    let pos_ptr: u32 = read_offset(pointer, 0x110);
    C3Vector {
        y: read_offset(pos_ptr, 0x24),
        x: read_offset(pos_ptr, 0x28),
        z: read_offset(pos_ptr, 0x2C),
    }
}

/// Get the health of a unit
#[inline]
pub unsafe fn get_unit_health(unit: u32) -> i32 {
    let descriptor: u32 = read_offset(unit, 0x8);
    read_offset(descriptor, 0x58)
}

/// Check if a unit is lootable (has loot flag set)
#[inline]
pub unsafe fn is_unit_lootable(unit: u32) -> bool {
    let descriptor: u32 = read_offset(unit, 0x8);
    let flags: i32 = read_offset(descriptor, 0x23C);
    (flags & 0x1) != 0
}

/// Check if a unit is skinnable
#[inline]
pub unsafe fn is_unit_skinnable(unit: u32) -> bool {
    let descriptor: u32 = read_offset(unit, 0x8);
    let flags: i32 = read_offset(descriptor, 0xB8);
    (flags & 0x0400_0000) != 0
}

// =============================================================================
// Interaction Functions
// =============================================================================

/// Set the current target by GUID
#[inline]
pub unsafe fn set_target(guid: u64) {
    let func: SetTargetFn = transmute(offsets::game::SET_TARGET);
    func(guid)
}

/// Interact with a unit (right-click)
#[inline]
pub unsafe fn interact_unit(pointer: u32, autoloot: i32) {
    let func: RightClickFn = transmute(offsets::game::RIGHT_CLICK_UNIT);
    func(pointer, autoloot)
}

/// Interact with a game object (right-click)
#[inline]
pub unsafe fn interact_object(pointer: u32, autoloot: i32) {
    let func: RightClickFn = transmute(offsets::game::RIGHT_CLICK_OBJECT);
    func(pointer, autoloot)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // C3Vector tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_c3vector_distance_same_point() {
        let a = C3Vector {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let b = C3Vector {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        assert!((a.distance(&b) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_c3vector_distance_unit_x() {
        let a = C3Vector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let b = C3Vector {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        };
        assert!((a.distance(&b) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_c3vector_distance_unit_y() {
        let a = C3Vector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let b = C3Vector {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        };
        assert!((a.distance(&b) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_c3vector_distance_unit_z() {
        let a = C3Vector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let b = C3Vector {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        };
        assert!((a.distance(&b) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_c3vector_distance_3d() {
        // 3-4-5 triangle in 3D: sqrt(3^2 + 4^2 + 0^2) = 5
        let a = C3Vector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let b = C3Vector {
            x: 3.0,
            y: 4.0,
            z: 0.0,
        };
        assert!((a.distance(&b) - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_c3vector_distance_3d_diagonal() {
        // sqrt(1^2 + 1^2 + 1^2) = sqrt(3) ≈ 1.732
        let a = C3Vector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let b = C3Vector {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        };
        let expected = 3.0_f32.sqrt();
        assert!((a.distance(&b) - expected).abs() < 0.0001);
    }

    #[test]
    fn test_c3vector_distance_symmetric() {
        let a = C3Vector {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let b = C3Vector {
            x: 4.0,
            y: 6.0,
            z: 8.0,
        };
        assert!((a.distance(&b) - b.distance(&a)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_c3vector_distance_negative_coords() {
        let a = C3Vector {
            x: -1.0,
            y: -1.0,
            z: -1.0,
        };
        let b = C3Vector {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        };
        // Distance should be sqrt(4 + 4 + 4) = sqrt(12) ≈ 3.464
        let expected = 12.0_f32.sqrt();
        assert!((a.distance(&b) - expected).abs() < 0.0001);
    }

    #[test]
    fn test_c3vector_distance_within_5_yards() {
        let player = C3Vector {
            x: 100.0,
            y: 200.0,
            z: 50.0,
        };
        let nearby = C3Vector {
            x: 103.0,
            y: 204.0,
            z: 50.0,
        };
        // Distance = sqrt(9 + 16 + 0) = 5.0 exactly
        assert!(player.distance(&nearby) <= 5.0);
    }

    #[test]
    fn test_c3vector_distance_beyond_5_yards() {
        let player = C3Vector {
            x: 100.0,
            y: 200.0,
            z: 50.0,
        };
        let far = C3Vector {
            x: 106.0,
            y: 200.0,
            z: 50.0,
        };
        // Distance = 6.0
        assert!(player.distance(&far) > 5.0);
    }

    #[test]
    fn test_c3vector_default() {
        let v = C3Vector::default();
        assert_eq!(v.x, 0.0);
        assert_eq!(v.y, 0.0);
        assert_eq!(v.z, 0.0);
    }

    // -------------------------------------------------------------------------
    // ObjectType tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_object_type_from_valid_values() {
        assert_eq!(ObjectType::from(0), ObjectType::None);
        assert_eq!(ObjectType::from(1), ObjectType::Item);
        assert_eq!(ObjectType::from(2), ObjectType::Container);
        assert_eq!(ObjectType::from(3), ObjectType::Unit);
        assert_eq!(ObjectType::from(4), ObjectType::Player);
        assert_eq!(ObjectType::from(5), ObjectType::GameObject);
        assert_eq!(ObjectType::from(6), ObjectType::DynamicObject);
        assert_eq!(ObjectType::from(7), ObjectType::Corpse);
    }

    #[test]
    fn test_object_type_from_invalid_values() {
        assert_eq!(ObjectType::from(8), ObjectType::None);
        assert_eq!(ObjectType::from(100), ObjectType::None);
        assert_eq!(ObjectType::from(u32::MAX), ObjectType::None);
    }

    #[test]
    fn test_object_type_default() {
        assert_eq!(ObjectType::default(), ObjectType::None);
    }

    #[test]
    fn test_object_type_equality() {
        assert_eq!(ObjectType::Unit, ObjectType::Unit);
        assert_ne!(ObjectType::Unit, ObjectType::Player);
        assert_ne!(ObjectType::GameObject, ObjectType::None);
    }

    // -------------------------------------------------------------------------
    // Blacklist tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_blacklist_contains_expected_ids() {
        let blacklist = get_blacklist();
        assert!(blacklist.contains(&179830));
        assert!(blacklist.contains(&179831));
        assert!(blacklist.contains(&179785));
        assert!(blacklist.contains(&179786));
    }

    #[test]
    fn test_blacklist_size() {
        let blacklist = get_blacklist();
        assert_eq!(blacklist.len(), 4);
    }

    #[test]
    fn test_blacklist_does_not_contain_other_ids() {
        let blacklist = get_blacklist();
        assert!(!blacklist.contains(&0));
        assert!(!blacklist.contains(&1));
        assert!(!blacklist.contains(&179829));
        assert!(!blacklist.contains(&179832));
        assert!(!blacklist.contains(&u32::MAX));
    }
}
