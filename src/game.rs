//! Game interface for WoW 1.12.1
//!
//! Provides functions to interact with game objects, units, and the game world.
//!
//! Note on unit position: We read directly from unit + 0x9B8/0x9BC/0x9C0.
//! UnitXP uses an alternative approach via CMovement (unit + 0x118) + 0x10,
//! which handles transport coordinates. Our direct method matches the
//! original Interact C implementation.

use crate::offsets;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::mem::transmute;
use std::num::NonZeroU32;

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

/// Lazily initialized blacklist set - only created once
static BLACKLIST: Lazy<HashSet<u32>> = Lazy::new(|| BLACKLISTED_OBJECTS.iter().copied().collect());

/// Check if a game object ID is blacklisted
#[inline]
pub fn is_blacklisted(id: u32) -> bool {
    BLACKLIST.contains(&id)
}

/// Get a reference to the blacklist set (for testing)
#[cfg(test)]
pub fn get_blacklist() -> &'static HashSet<u32> {
    &BLACKLIST
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

/// Read a value from a memory address.
///
/// # Safety
/// - `addr` must point to valid, initialized memory within the WoW process
/// - `addr` must be properly aligned for type `T`
/// - The memory at `addr` must contain a valid bit pattern for `T`
/// - The address must be a known WoW 1.12.1.5875 memory location
#[inline]
unsafe fn read<T: Copy>(addr: u32) -> T {
    // SAFETY: Caller guarantees addr is a valid WoW 1.12.1 memory address.
    // All addresses used are documented in wow_offsets_reference.md and
    // have been verified against the 1.12.1.5875 client.
    *(addr as *const T)
}

/// Read a value at base + offset.
///
/// # Safety
/// - `base + offset` must not overflow
/// - The resulting address must point to valid, initialized memory
/// - The address must be properly aligned for type `T`
/// - Used for reading fields from WoW's internal object structures
#[inline]
unsafe fn read_offset<T: Copy>(base: u32, offset: u32) -> T {
    // SAFETY: Caller guarantees base is a valid object pointer obtained from
    // the game's object manager, and offset is a documented field offset.
    // Object structure offsets are from wow_offsets_reference.md.
    read(base + offset)
}

// =============================================================================
// Game State
// =============================================================================

/// Check if the player is currently in the game world.
///
/// Reads the `IsIngame` flag at `0xB4B424`.
#[inline]
pub unsafe fn is_in_world() -> bool {
    // SAFETY: IS_IN_WORLD (0xB4B424) is a static game variable that exists
    // for the lifetime of the WoW process. See wow_offsets_reference.md: Player.IsIngame
    read::<u8>(offsets::game::IS_IN_WORLD as u32) != 0
}

/// Get the visible objects manager pointer.
///
/// Returns the base pointer to the object manager at `0xB41414`.
#[inline]
pub unsafe fn get_visible_objects() -> u32 {
    // SAFETY: VISIBLE_OBJECTS (0xB41414) is the static object manager base pointer.
    // See wow_offsets_reference.md: ObjectManager.ManagerBase
    read::<u32>(offsets::game::VISIBLE_OBJECTS as u32)
}

// =============================================================================
// Object Accessors
// =============================================================================

/// Get a pointer to a game object by its GUID.
///
/// Calls the game's `GetPtrForGuid` function at `0x464870`.
/// Returns `None` if the object is not found (null pointer).
///
/// Using `Option<NonZeroU32>` provides:
/// - Explicit null checking at the type level
/// - Same memory layout as `u32` (niche optimization)
/// - Prevents accidental use of null pointers
#[inline]
pub unsafe fn get_object_pointer(guid: u64) -> Option<NonZeroU32> {
    // SAFETY: GET_OBJECT_POINTER (0x464870) is the game's GetPtrForGuid function.
    // It safely returns 0 for invalid GUIDs. See wow_offsets_reference.md: Functions.GetPtrForGuid
    let func: GetObjectPointerFn = transmute(offsets::game::GET_OBJECT_POINTER);
    NonZeroU32::new(func(guid))
}

/// Get a raw pointer to a game object by its GUID (legacy API).
///
/// Prefer `get_object_pointer` which returns `Option<NonZeroU32>` for type safety.
/// This function is provided for cases where you need the raw u32 value.
#[inline]
#[allow(dead_code)] // Utility function for future use or external callers
pub unsafe fn get_object_pointer_raw(guid: u64) -> u32 {
    // SAFETY: This function's safety relies on get_object_pointer.
    // It provides a raw u32 pointer, returning 0 for None.
    get_object_pointer(guid).map_or(0, NonZeroU32::get)
}

/// Get the player's GUID from the visible objects manager.
///
/// Reads at offset `0xC0` from the object manager base.
#[inline]
pub unsafe fn get_player_guid(objects: u32) -> u64 {
    // SAFETY: objects is the object manager pointer from get_visible_objects().
    // Offset 0xC0 is PlayerGuid. See wow_offsets_reference.md: ObjectManager.PlayerGuid
    read_offset(objects, 0xC0)
}

/// Get the first object in the visible objects list.
///
/// Reads at offset `0xAC` from the object manager base.
#[inline]
pub unsafe fn get_first_object(objects: u32) -> u32 {
    // SAFETY: objects is the object manager pointer.
    // Offset 0xAC is FirstObj. See wow_offsets_reference.md: ObjectManager.FirstObj
    read_offset(objects, 0xAC)
}

/// Get the next object in the linked list.
///
/// Reads at offset `0x3C` from the current object.
#[inline]
pub unsafe fn get_next_object(current: u32) -> u32 {
    // SAFETY: current is an object list entry pointer.
    // Offset 0x3C is NextObj. See wow_offsets_reference.md: ObjectManager.NextObj
    read_offset(current, 0x3C)
}

/// Get the GUID of an object from its list entry.
///
/// Reads at offset `0x30` from the current object.
#[inline]
pub unsafe fn get_object_guid(current: u32) -> u64 {
    // SAFETY: current is an object list entry pointer.
    // Offset 0x30 is CurObjGuid. See wow_offsets_reference.md: ObjectManager.CurObjGuid
    read_offset(current, 0x30)
}

/// Get the object type from a pointer.
///
/// Reads at offset `0x14` from the object pointer.
#[inline]
pub unsafe fn get_object_type(pointer: u32) -> ObjectType {
    // SAFETY: pointer is a valid object pointer from get_object_pointer().
    // Offset 0x14 is ObjType. See wow_offsets_reference.md: ObjectManager.ObjType
    ObjectType::from(read_offset::<u32>(pointer, 0x14))
}

/// Get the "summoned by" GUID for an object.
///
/// First reads the descriptor pointer at offset `0x8`, then reads
/// the summoned-by GUID at offset `0x30` from the descriptor.
#[inline]
pub unsafe fn get_summoned_by_guid(pointer: u32) -> u64 {
    // SAFETY: pointer is a valid object pointer.
    // Offset 0x8 is DescriptorOffset, 0x30 is SummonedByGuid.
    // See wow_offsets_reference.md: ObjectManager.DescriptorOffset, Descriptors.SummonedByGuid
    let descriptor: u32 = read_offset(pointer, 0x8);
    read_offset(descriptor, 0x30)
}

/// Get the game object ID.
///
/// Reads at offset `0x294` from the object pointer.
#[inline]
pub unsafe fn get_gameobject_id(pointer: u32) -> u32 {
    // SAFETY: pointer is a valid GameObject pointer.
    // Offset 0x294 contains the game object's entry ID.
    read_offset(pointer, 0x294)
}

// =============================================================================
// Unit Functions
// =============================================================================

/// Get the position of a unit.
///
/// Reads X/Y/Z coordinates from offsets `0x9B8`/`0x9BC`/`0x9C0`.
/// Note: WoW uses Y, X, Z order in memory.
#[inline]
pub unsafe fn get_unit_position(unit: u32) -> C3Vector {
    // SAFETY: unit is a valid unit pointer from the object list.
    // Offsets are from wow_offsets_reference.md: Unit.PosX/PosY/PosZ
    C3Vector {
        y: read_offset(unit, 0x09B8),
        x: read_offset(unit, 0x09BC),
        z: read_offset(unit, 0x09C0),
    }
}

/// Get the position of a game object.
///
/// First reads a position structure pointer at offset `0x110`, then
/// reads the coordinates from that structure.
#[inline]
pub unsafe fn get_object_position(pointer: u32) -> C3Vector {
    // SAFETY: pointer is a valid GameObject pointer.
    // Offset 0x110 points to a position structure.
    // The position structure has Y/X/Z at offsets 0x24/0x28/0x2C.
    let pos_ptr: u32 = read_offset(pointer, 0x110);
    C3Vector {
        y: read_offset(pos_ptr, 0x24),
        x: read_offset(pos_ptr, 0x28),
        z: read_offset(pos_ptr, 0x2C),
    }
}

/// Get the health of a unit.
///
/// Reads health from the unit's descriptor at offset `0x58`.
#[inline]
pub unsafe fn get_unit_health(unit: u32) -> i32 {
    // SAFETY: unit is a valid unit pointer.
    // Offset 0x8 is DescriptorOffset, 0x58 is Health.
    // See wow_offsets_reference.md: Descriptors.Health
    let descriptor: u32 = read_offset(unit, 0x8);
    read_offset(descriptor, 0x58)
}

/// Check if a unit is lootable (has loot flag set).
///
/// Checks bit 0 of the DynamicFlags at descriptor offset `0x23C`.
#[inline]
pub unsafe fn is_unit_lootable(unit: u32) -> bool {
    // SAFETY: unit is a valid unit pointer.
    // Offset 0x8 is DescriptorOffset, 0x23C is DynamicFlags.
    // Bit 0 of DynamicFlags indicates lootable.
    // See wow_offsets_reference.md: Descriptors.DynamicFlags
    let descriptor: u32 = read_offset(unit, 0x8);
    let flags: i32 = read_offset(descriptor, 0x23C);
    (flags & 0x1) != 0
}

/// Check if a unit is skinnable.
///
/// Checks bit 26 (`0x0400_0000`) of Flags at descriptor offset `0xB8`.
#[inline]
pub unsafe fn is_unit_skinnable(unit: u32) -> bool {
    // SAFETY: unit is a valid unit pointer.
    // Offset 0x8 is DescriptorOffset, 0xB8 is Flags.
    // Bit 26 (0x04000000) indicates skinnable.
    // See wow_offsets_reference.md: Descriptors.Flags
    let descriptor: u32 = read_offset(unit, 0x8);
    let flags: i32 = read_offset(descriptor, 0xB8);
    (flags & 0x0400_0000) != 0
}

// =============================================================================
// Interaction Functions
// =============================================================================

/// Set the current target by GUID.
///
/// Calls the game's `SetTarget` function at `0x493540`.
#[inline]
pub unsafe fn set_target(guid: u64) {
    // SAFETY: SET_TARGET (0x493540) is the game's SetTarget function.
    // It handles invalid GUIDs gracefully (clears target).
    // See wow_offsets_reference.md: Functions.SetTarget
    let func: SetTargetFn = transmute(offsets::game::SET_TARGET);
    func(guid);
}

/// Interact with a unit (right-click).
///
/// Calls the game's `OnRightClickUnit` function at `0x60BEA0`.
#[inline]
pub unsafe fn interact_unit(pointer: u32, autoloot: i32) {
    // SAFETY: RIGHT_CLICK_UNIT (0x60BEA0) is OnRightClickUnit.
    // pointer must be a valid unit pointer from get_object_pointer().
    // See wow_offsets_reference.md: Functions.OnRightClickUnit
    let func: RightClickFn = transmute(offsets::game::RIGHT_CLICK_UNIT);
    func(pointer, autoloot);
}

/// Interact with a game object (right-click).
///
/// Calls the game's `OnRightClickObject` function at `0x5F8660`.
#[inline]
pub unsafe fn interact_object(pointer: u32, autoloot: i32) {
    // SAFETY: RIGHT_CLICK_OBJECT (0x5F8660) is OnRightClickObject.
    // pointer must be a valid GameObject pointer from get_object_pointer().
    // See wow_offsets_reference.md: Functions.OnRightClickObject
    let func: RightClickFn = transmute(offsets::game::RIGHT_CLICK_OBJECT);
    func(pointer, autoloot);
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // Exact float comparisons are intentional in these tests

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
        assert!(is_blacklisted(179830));
        assert!(is_blacklisted(179831));
        assert!(is_blacklisted(179785));
        assert!(is_blacklisted(179786));
    }

    #[test]
    fn test_blacklist_size() {
        let blacklist = get_blacklist();
        assert_eq!(blacklist.len(), 4);
    }

    #[test]
    fn test_blacklist_does_not_contain_other_ids() {
        assert!(!is_blacklisted(0));
        assert!(!is_blacklisted(1));
        assert!(!is_blacklisted(179829));
        assert!(!is_blacklisted(179832));
        assert!(!is_blacklisted(u32::MAX));
    }
}
