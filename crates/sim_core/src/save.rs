//! Save and resume (GP-12, ADR 0001, ADR 0016).
//!
//! A save is a small [`SaveHeader`] followed by a zstd-compressed payload
//! that holds every registered resource and component. Every type in the
//! state hash is also registered here ([`AppSaveExt`]); the integration test
//! `every_hashed_type_is_saved` checks it.
//!
//! - [`SaveState`] mirrors [`StateHash`](crate::StateHash): each type writes
//!   its fields to a [`SaveWriter`] and reads them back from a
//!   [`SaveReader`], in a fixed order. Entity references are written as
//!   their [`SpawnId`]s and mapped to the new entities on load.
//! - The macros [`save_fields!`](crate::save_fields),
//!   [`save_newtype!`](crate::save_newtype), [`save_unit!`](crate::save_unit)
//!   and [`save_serde!`](crate::save_serde) implement [`SaveState`] for plain
//!   structs, newtypes, markers and serde types (postcard, which stores enum
//!   variants by index: enums that are saved are append only).
//! - [`save_world`] and [`load_world`] write and restore a whole world.
//!   Save only between ticks: messages in flight are not saved. Tick 0
//!   before the first step counts: setup leaves no events or commands in
//!   flight, except a battle's (`sim_military::setup_battle` queues AI
//!   orders for the first tick, so a battle saved then diverges). Caches
//!   are not saved; [`AppSaveExt::on_load`] hooks rebuild them.
//!
//! Never bincode (unmaintained): serde, postcard and zstd only (CLAUDE.md).

use std::any::TypeId;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::Arc;

use bevy_app::App;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::command::{CommandLog, CommandScript, SimCommand};
use crate::components::SpawnId;
use crate::events::SimEvent;
use crate::rng::SimSeed;
use crate::run::{RunConfig, RunState};
use crate::schedule::SimTick;

/// The first four bytes of every save file.
pub const SAVE_MAGIC: [u8; 4] = *b"WLSV";

/// Bump when the payload layout or the meaning of a saved field changes.
/// Saves of another format are refused (no migration in M4).
/// 2 (M4a, ADR 0020): Lordship's skeleton (tenure, loyalty terms, the deed
/// log, edicts, influence, fallout, stewards, agents, sites, the rival's
/// realm and vows, `Veteran.trade`). Bumped once for all of M4a: a later
/// M4a change to one of these layouts amends it in place.
pub const SAVE_FORMAT: u32 = 2;

/// zstd level: fast, and saves are small anyway.
const ZSTD_LEVEL: i32 = 3;

/// Why a save could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// The bytes don't start with [`SAVE_MAGIC`].
    NotASave,
    /// The save was written in another [`SAVE_FORMAT`].
    Format {
        /// The save's format.
        found: u32,
        /// This build's format.
        expected: u32,
    },
    /// The data is damaged or ends early.
    Corrupt(String),
    /// The save doesn't match the types this build registers (another
    /// build), or a saved value is invalid (data that no longer parses).
    Mismatch(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::NotASave => write!(f, "not a save file"),
            SaveError::Format { found, expected } => write!(
                f,
                "save format {found} is not supported (expected {expected})"
            ),
            SaveError::Corrupt(why) => write!(f, "damaged save: {why}"),
            SaveError::Mismatch(why) => write!(f, "save does not match this build: {why}"),
        }
    }
}

impl std::error::Error for SaveError {}

fn corrupt(why: impl Into<String>) -> SaveError {
    SaveError::Corrupt(why.into())
}

/// What a save is, readable without decompressing it (the resume menu).
/// Stored as postcard right after [`SAVE_MAGIC`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveHeader {
    /// Always [`SAVE_MAGIC`].
    pub magic: [u8; 4],
    /// [`SAVE_FORMAT`] when written.
    pub format: u32,
    /// The writing build's crate version (`CARGO_PKG_VERSION`).
    pub build: String,
    /// The world seed (SIM-02).
    pub seed: u64,
    /// The worldgen preset ("developed", "run", or "custom").
    pub preset: String,
    /// The run's config, for a run (GP-21).
    pub run: Option<RunConfig>,
    /// The tick the save was taken at (the next tick to run).
    pub tick: u64,
}

/// Writes a type's state to a save, and reads it back (GP-12). Implemented
/// by every saved component and resource, field by field in a fixed order,
/// like [`StateHash`](crate::StateHash).
pub trait SaveState: Sized {
    /// Writes every field that is simulation state.
    fn save(&self, w: &mut SaveWriter);
    /// Reads what [`SaveState::save`] wrote, in the same order.
    fn load(r: &mut SaveReader) -> Result<Self, SaveError>;
}

/// The byte sink [`SaveState::save`] writes to. Integers are little-endian,
/// lengths are `u32`, floats keep their bits, and entities are written as
/// their [`SpawnId`] (plus one; 0 for an entity without one).
pub struct SaveWriter<'a> {
    buf: Vec<u8>,
    ids: &'a HashMap<Entity, SpawnId>,
}

impl<'a> SaveWriter<'a> {
    /// A writer that maps entities through `ids`.
    pub fn new(ids: &'a HashMap<Entity, SpawnId>) -> Self {
        Self {
            buf: Vec::new(),
            ids,
        }
    }

    /// The bytes written so far.
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// Writes raw bytes (no length).
    pub fn raw(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }
    /// Writes a length-prefixed byte string.
    pub fn bytes(&mut self, bytes: &[u8]) {
        self.count(bytes.len());
        self.raw(bytes);
    }
    /// Writes a length or count.
    pub fn count(&mut self, n: usize) {
        self.u32(u32::try_from(n).expect("saved collections hold fewer than 2^32 items"));
    }
    /// Writes a bool.
    pub fn bool(&mut self, v: bool) {
        self.u8(v as u8);
    }
    /// Writes a u8.
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    /// Writes a u16.
    pub fn u16(&mut self, v: u16) {
        self.raw(&v.to_le_bytes());
    }
    /// Writes a u32.
    pub fn u32(&mut self, v: u32) {
        self.raw(&v.to_le_bytes());
    }
    /// Writes a u64.
    pub fn u64(&mut self, v: u64) {
        self.raw(&v.to_le_bytes());
    }
    /// Writes a u128.
    pub fn u128(&mut self, v: u128) {
        self.raw(&v.to_le_bytes());
    }
    /// Writes an i32.
    pub fn i32(&mut self, v: i32) {
        self.raw(&v.to_le_bytes());
    }
    /// Writes an i64.
    pub fn i64(&mut self, v: i64) {
        self.raw(&v.to_le_bytes());
    }
    /// Writes an f32 by its bits.
    pub fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    /// Writes an f64 by its bits.
    pub fn f64(&mut self, v: f64) {
        self.u64(v.to_bits());
    }
    /// Writes an integer vector as two i32s.
    pub fn ivec2(&mut self, v: IVec2) {
        self.i32(v.x);
        self.i32(v.y);
    }
    /// Writes a float vector as two f32s.
    pub fn vec2(&mut self, v: Vec2) {
        self.f32(v.x);
        self.f32(v.y);
    }
    /// Writes a string.
    pub fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }
    /// Writes an entity reference as its SpawnId plus one, or 0 when the
    /// entity has none (despawned, or not a simulation entity).
    pub fn entity(&mut self, e: Entity) {
        self.u64(self.ids.get(&e).map_or(0, |id| id.0 + 1));
    }
    /// Writes a plain-data serde value with postcard (enums by variant
    /// index, so saved enums are append only).
    pub fn serde<T: Serialize + ?Sized>(&mut self, v: &T) {
        let buf = std::mem::take(&mut self.buf);
        self.buf = postcard::to_extend(v, buf).expect("postcard writes plain data");
    }
    /// Writes a data value as RON text (data resources: old saves keep
    /// their tuning, and the text is what the state hash reads).
    pub fn ron<T: Serialize + ?Sized>(&mut self, v: &T) {
        self.str(&ron::to_string(v).expect("data serialises to RON"));
    }
    /// Writes any [`SaveState`] value.
    pub fn save<T: SaveState>(&mut self, v: &T) {
        v.save(self);
    }

    /// Writes `blob`'s bytes, length-prefixed, as a section of this save.
    fn section(&mut self, blob: SaveWriter) {
        self.bytes(&blob.buf);
    }
}

/// Reads what a [`SaveWriter`] wrote; entities come back as the entities
/// spawned for their SpawnIds on load.
pub struct SaveReader<'a> {
    buf: &'a [u8],
    pos: usize,
    entities: &'a HashMap<u64, Entity>,
}

impl<'a> SaveReader<'a> {
    /// A reader over `buf` that maps SpawnIds through `entities`.
    pub fn new(buf: &'a [u8], entities: &'a HashMap<u64, Entity>) -> Self {
        Self {
            buf,
            pos: 0,
            entities,
        }
    }

    /// Bytes not read yet.
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    /// Reads `n` raw bytes.
    pub fn raw(&mut self, n: usize) -> Result<&'a [u8], SaveError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.buf.len())
            .ok_or_else(|| corrupt("unexpected end of data"))?;
        let out = &self.buf[self.pos..end];
        self.pos = end;
        Ok(out)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], SaveError> {
        Ok(self.raw(N)?.try_into().expect("N bytes"))
    }
    /// Reads a length-prefixed byte string.
    pub fn bytes(&mut self) -> Result<&'a [u8], SaveError> {
        let n = self.count()?;
        self.raw(n)
    }
    /// Reads a length or count.
    pub fn count(&mut self) -> Result<usize, SaveError> {
        let n = self.u32()? as usize;
        if n > self.remaining().max(1) * 64 {
            // Nothing saved is that sparse: a damaged length.
            return Err(corrupt(format!("implausible length {n}")));
        }
        Ok(n)
    }
    /// Reads a bool.
    pub fn bool(&mut self) -> Result<bool, SaveError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            b => Err(corrupt(format!("bad bool {b}"))),
        }
    }
    /// Reads a u8.
    pub fn u8(&mut self) -> Result<u8, SaveError> {
        Ok(self.raw(1)?[0])
    }
    /// Reads a u16.
    pub fn u16(&mut self) -> Result<u16, SaveError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    /// Reads a u32.
    pub fn u32(&mut self) -> Result<u32, SaveError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    /// Reads a u64.
    pub fn u64(&mut self) -> Result<u64, SaveError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    /// Reads a u128.
    pub fn u128(&mut self) -> Result<u128, SaveError> {
        Ok(u128::from_le_bytes(self.array()?))
    }
    /// Reads an i32.
    pub fn i32(&mut self) -> Result<i32, SaveError> {
        Ok(i32::from_le_bytes(self.array()?))
    }
    /// Reads an i64.
    pub fn i64(&mut self) -> Result<i64, SaveError> {
        Ok(i64::from_le_bytes(self.array()?))
    }
    /// Reads an f32.
    pub fn f32(&mut self) -> Result<f32, SaveError> {
        Ok(f32::from_bits(self.u32()?))
    }
    /// Reads an f64.
    pub fn f64(&mut self) -> Result<f64, SaveError> {
        Ok(f64::from_bits(self.u64()?))
    }
    /// Reads an integer vector.
    pub fn ivec2(&mut self) -> Result<IVec2, SaveError> {
        Ok(IVec2::new(self.i32()?, self.i32()?))
    }
    /// Reads a float vector.
    pub fn vec2(&mut self) -> Result<Vec2, SaveError> {
        Ok(Vec2::new(self.f32()?, self.f32()?))
    }
    /// Reads a string.
    pub fn str(&mut self) -> Result<String, SaveError> {
        String::from_utf8(self.bytes()?.to_vec()).map_err(|_| corrupt("a string is not UTF-8"))
    }
    /// Reads an entity reference: the entity now carrying the saved
    /// SpawnId, or [`Entity::PLACEHOLDER`] for none (which, like the
    /// despawned entity it stands for, refers to nothing).
    pub fn entity(&mut self) -> Result<Entity, SaveError> {
        let v = self.u64()?;
        Ok(match v {
            0 => Entity::PLACEHOLDER,
            id => self
                .entities
                .get(&(id - 1))
                .copied()
                .unwrap_or(Entity::PLACEHOLDER),
        })
    }
    /// Reads a value written by [`SaveWriter::serde`].
    pub fn serde<T: DeserializeOwned>(&mut self) -> Result<T, SaveError> {
        let rest = &self.buf[self.pos..];
        let (v, left) = postcard::take_from_bytes::<T>(rest)
            .map_err(|e| corrupt(format!("{}: {e}", std::any::type_name::<T>())))?;
        self.pos += rest.len() - left.len();
        Ok(v)
    }
    /// Reads a value written by [`SaveWriter::ron`].
    pub fn ron<T: DeserializeOwned>(&mut self) -> Result<T, SaveError> {
        let text = self.str()?;
        ron::from_str(&text)
            .map_err(|e| SaveError::Mismatch(format!("{}: {e}", std::any::type_name::<T>())))
    }
    /// Reads any [`SaveState`] value.
    pub fn load<T: SaveState>(&mut self) -> Result<T, SaveError> {
        T::load(self)
    }

    /// Fails unless everything was read: catches a `load` that doesn't
    /// mirror its `save`.
    fn finish(&self, what: &str) -> Result<(), SaveError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(corrupt(format!(
                "{what}: {} bytes not read",
                self.remaining()
            )))
        }
    }
}

macro_rules! save_le {
    ($($t:ident),*) => {$(
        impl SaveState for $t {
            fn save(&self, w: &mut SaveWriter) {
                w.$t(*self);
            }
            fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
                r.$t()
            }
        }
    )*};
}
save_le!(u8, u16, u32, u64, u128, i32, i64, f32, f64, bool);

impl SaveState for usize {
    fn save(&self, w: &mut SaveWriter) {
        w.u64(*self as u64);
    }
    fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
        usize::try_from(r.u64()?).map_err(|_| corrupt("usize out of range"))
    }
}

impl SaveState for IVec2 {
    fn save(&self, w: &mut SaveWriter) {
        w.ivec2(*self);
    }
    fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
        r.ivec2()
    }
}

impl SaveState for Vec2 {
    fn save(&self, w: &mut SaveWriter) {
        w.vec2(*self);
    }
    fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
        r.vec2()
    }
}

impl SaveState for Entity {
    fn save(&self, w: &mut SaveWriter) {
        w.entity(*self);
    }
    fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
        r.entity()
    }
}

impl SaveState for String {
    fn save(&self, w: &mut SaveWriter) {
        w.str(self);
    }
    fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
        r.str()
    }
}

impl<T: SaveState> SaveState for Option<T> {
    fn save(&self, w: &mut SaveWriter) {
        match self {
            Some(v) => {
                w.u8(1);
                v.save(w);
            }
            None => w.u8(0),
        }
    }
    fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
        match r.u8()? {
            0 => Ok(None),
            1 => Ok(Some(T::load(r)?)),
            t => Err(corrupt(format!("bad option tag {t}"))),
        }
    }
}

impl<T: SaveState> SaveState for Vec<T> {
    fn save(&self, w: &mut SaveWriter) {
        w.count(self.len());
        for v in self {
            v.save(w);
        }
    }
    fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
        let n = r.count()?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(T::load(r)?);
        }
        Ok(out)
    }
}

impl<T: SaveState> SaveState for VecDeque<T> {
    fn save(&self, w: &mut SaveWriter) {
        w.count(self.len());
        for v in self {
            v.save(w);
        }
    }
    fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
        Ok(Vec::<T>::load(r)?.into())
    }
}

impl<T: SaveState> SaveState for Arc<[T]> {
    fn save(&self, w: &mut SaveWriter) {
        w.count(self.len());
        for v in self.iter() {
            v.save(w);
        }
    }
    fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
        Ok(Vec::<T>::load(r)?.into())
    }
}

impl<T: SaveState, const N: usize> SaveState for [T; N] {
    fn save(&self, w: &mut SaveWriter) {
        for v in self {
            v.save(w);
        }
    }
    fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
        let v: Vec<T> = (0..N).map(|_| T::load(r)).collect::<Result<_, _>>()?;
        Ok(v.try_into().unwrap_or_else(|_| unreachable!("N items")))
    }
}

impl<K: SaveState + Ord, V: SaveState> SaveState for BTreeMap<K, V> {
    fn save(&self, w: &mut SaveWriter) {
        w.count(self.len());
        for (k, v) in self {
            k.save(w);
            v.save(w);
        }
    }
    fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
        let n = r.count()?;
        let mut out = BTreeMap::new();
        for _ in 0..n {
            let k = K::load(r)?;
            out.insert(k, V::load(r)?);
        }
        Ok(out)
    }
}

impl<T: SaveState + Ord> SaveState for BTreeSet<T> {
    fn save(&self, w: &mut SaveWriter) {
        w.count(self.len());
        for v in self {
            v.save(w);
        }
    }
    fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
        Ok(Vec::<T>::load(r)?.into_iter().collect())
    }
}

macro_rules! save_tuple {
    ($($n:tt $t:ident),*) => {
        impl<$($t: SaveState),*> SaveState for ($($t,)*) {
            fn save(&self, w: &mut SaveWriter) {
                $( self.$n.save(w); )*
            }
            fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
                Ok(($($t::load(r)?,)*))
            }
        }
    };
}
save_tuple!(0 A, 1 B);
save_tuple!(0 A, 1 B, 2 C);
save_tuple!(0 A, 1 B, 2 C, 3 D);

/// Implements [`SaveState`] for a struct with named fields, saving each
/// listed field in order. Every field must be listed (the struct literal
/// on load won't compile otherwise); invoke it where the fields are visible.
///
/// ```text
/// sim_core::save_fields!(Needs { hunger, rest, morale });
/// ```
#[macro_export]
macro_rules! save_fields {
    ($t:ident { $($f:ident),* $(,)? }) => {
        impl $crate::save::SaveState for $t {
            fn save(&self, w: &mut $crate::save::SaveWriter) {
                $( $crate::save::SaveState::save(&self.$f, w); )*
            }
            fn load(
                r: &mut $crate::save::SaveReader,
            ) -> ::core::result::Result<Self, $crate::save::SaveError> {
                ::core::result::Result::Ok(Self {
                    $( $f: $crate::save::SaveState::load(r)?, )*
                })
            }
        }
    };
}

/// Implements [`SaveState`] for a one-field tuple struct (`struct T(U)`).
#[macro_export]
macro_rules! save_newtype {
    ($($t:ident),* $(,)?) => {$(
        impl $crate::save::SaveState for $t {
            fn save(&self, w: &mut $crate::save::SaveWriter) {
                $crate::save::SaveState::save(&self.0, w);
            }
            fn load(
                r: &mut $crate::save::SaveReader,
            ) -> ::core::result::Result<Self, $crate::save::SaveError> {
                ::core::result::Result::Ok(Self($crate::save::SaveState::load(r)?))
            }
        }
    )*};
}

/// Implements [`SaveState`] for unit structs (markers): only presence is saved.
#[macro_export]
macro_rules! save_unit {
    ($($t:ident),* $(,)?) => {$(
        impl $crate::save::SaveState for $t {
            fn save(&self, _w: &mut $crate::save::SaveWriter) {}
            fn load(
                _r: &mut $crate::save::SaveReader,
            ) -> ::core::result::Result<Self, $crate::save::SaveError> {
                ::core::result::Result::Ok(Self)
            }
        }
    )*};
}

/// Implements [`SaveState`] through serde and postcard, for plain data
/// with no entity references (postcard stores enum variants by index, so
/// saved enums are append only).
#[macro_export]
macro_rules! save_serde {
    ($($t:ty),* $(,)?) => {$(
        impl $crate::save::SaveState for $t {
            fn save(&self, w: &mut $crate::save::SaveWriter) {
                w.serde(self);
            }
            fn load(
                r: &mut $crate::save::SaveReader,
            ) -> ::core::result::Result<Self, $crate::save::SaveError> {
                r.serde()
            }
        }
    )*};
}

type SaveResourceFn = fn(&World, &mut SaveWriter);
type LoadResourceFn = fn(&mut World, &mut SaveReader) -> Result<(), SaveError>;
type SaveComponentsFn = fn(&World, &HashMap<SpawnId, u32>, &mut SaveWriter) -> u32;
type LoadComponentsFn = fn(&mut World, &[Entity], u32, &mut SaveReader) -> Result<(), SaveError>;

struct SavedResource {
    name: &'static str,
    present: fn(&World) -> bool,
    save: SaveResourceFn,
    load: LoadResourceFn,
}

struct SavedComponent {
    name: &'static str,
    save: SaveComponentsFn,
    load: LoadComponentsFn,
}

/// Every saved resource and component type, in registration order, and
/// the hooks that rebuild caches after a load. Each `sim_*` plugin
/// registers its own types through [`AppSaveExt`].
#[derive(Resource, Default)]
pub struct SaveRegistry {
    resources: Vec<SavedResource>,
    components: Vec<SavedComponent>,
    types: BTreeMap<TypeId, &'static str>,
    hooks: Vec<fn(&mut World)>,
    preset: Option<fn(&World) -> String>,
}

impl SaveRegistry {
    /// Whether type `T` is saved.
    pub fn contains(&self, id: TypeId) -> bool {
        self.types.contains_key(&id)
    }
    /// `(TypeId, type name)` of every saved type.
    pub fn saved_types(&self) -> impl Iterator<Item = (TypeId, &'static str)> + '_ {
        self.types.iter().map(|(id, n)| (*id, *n))
    }
    /// Names of saved resources, in save order.
    pub fn resource_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.resources.iter().map(|r| r.name)
    }
    /// Names of saved components, in save order.
    pub fn component_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.components.iter().map(|c| c.name)
    }
}

/// Registers types in saves, and load hooks; every sim plugin uses it next
/// to [`AppHashExt`](crate::AppHashExt): a hashed type is also saved.
pub trait AppSaveExt {
    /// Saves component `T` on every entity with a [`SpawnId`].
    fn register_saved<T: Component + SaveState>(&mut self) -> &mut Self;
    /// Saves resource `T` (its presence too).
    fn register_saved_resource<T: Resource + SaveState>(&mut self) -> &mut Self;
    /// Saves resource `T` with hand-written functions, for resources that
    /// load from other resources (data resolved against the calendar, a map
    /// regenerated from the seed). `load` runs after every resource
    /// registered before `T` has loaded, and only if `T` was present.
    fn register_saved_resource_with<T: Resource>(
        &mut self,
        save: fn(&World, &mut SaveWriter),
        load: fn(&mut World, &mut SaveReader) -> Result<(), SaveError>,
    ) -> &mut Self;
    /// Runs `hook` after a load, once every resource and component is
    /// restored (in registration order): rebuilds caches and indexes.
    fn on_load(&mut self, hook: fn(&mut World)) -> &mut Self;
    /// Names the worldgen preset in save headers (sim_world).
    fn save_preset_name(&mut self, name: fn(&World) -> String) -> &mut Self;
}

impl AppSaveExt for App {
    fn register_saved<T: Component + SaveState>(&mut self) -> &mut Self {
        let mut reg = self.world_mut().get_resource_or_init::<SaveRegistry>();
        if reg
            .types
            .insert(TypeId::of::<T>(), std::any::type_name::<T>())
            .is_none()
        {
            reg.components.push(SavedComponent {
                name: std::any::type_name::<T>(),
                save: save_components::<T>,
                load: load_components::<T>,
            });
        }
        self
    }

    fn register_saved_resource<T: Resource + SaveState>(&mut self) -> &mut Self {
        self.register_saved_resource_with::<T>(
            |world, w| world.resource::<T>().save(w),
            |world, r| {
                let v = T::load(r)?;
                world.insert_resource(v);
                Ok(())
            },
        )
    }

    fn register_saved_resource_with<T: Resource>(
        &mut self,
        save: fn(&World, &mut SaveWriter),
        load: fn(&mut World, &mut SaveReader) -> Result<(), SaveError>,
    ) -> &mut Self {
        let mut reg = self.world_mut().get_resource_or_init::<SaveRegistry>();
        if reg
            .types
            .insert(TypeId::of::<T>(), std::any::type_name::<T>())
            .is_none()
        {
            reg.resources.push(SavedResource {
                name: std::any::type_name::<T>(),
                present: |world| world.contains_resource::<T>(),
                save,
                load,
            });
        }
        self
    }

    fn on_load(&mut self, hook: fn(&mut World)) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<SaveRegistry>()
            .hooks
            .push(hook);
        self
    }

    fn save_preset_name(&mut self, name: fn(&World) -> String) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<SaveRegistry>()
            .preset = Some(name);
        self
    }
}

fn save_components<T: Component + SaveState>(
    world: &World,
    index: &HashMap<SpawnId, u32>,
    w: &mut SaveWriter,
) -> u32 {
    let Some(mut q) = world.try_query::<(&SpawnId, &T)>() else {
        return 0;
    };
    let mut found: Vec<(u32, &T)> = q
        .iter(world)
        .filter_map(|(id, c)| index.get(id).map(|&i| (i, c)))
        .collect();
    found.sort_unstable_by_key(|(i, _)| *i);
    for (i, c) in &found {
        w.u32(*i);
        c.save(w);
    }
    found.len() as u32
}

fn load_components<T: Component + SaveState>(
    world: &mut World,
    entities: &[Entity],
    count: u32,
    r: &mut SaveReader,
) -> Result<(), SaveError> {
    let mut batch = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let i = r.u32()? as usize;
        let e = *entities
            .get(i)
            .ok_or_else(|| corrupt(format!("entity index {i} out of range")))?;
        batch.push((e, T::load(r)?));
    }
    world.insert_batch(batch);
    Ok(())
}

/// Reads a save's header without decompressing the rest.
pub fn read_header(bytes: &[u8]) -> Result<SaveHeader, SaveError> {
    split(bytes).map(|(h, _)| h)
}

fn split(bytes: &[u8]) -> Result<(SaveHeader, &[u8]), SaveError> {
    if !bytes.starts_with(&SAVE_MAGIC) {
        return Err(SaveError::NotASave);
    }
    let (format, _) = postcard::take_from_bytes::<u32>(&bytes[SAVE_MAGIC.len()..])
        .map_err(|e| corrupt(format!("header: {e}")))?;
    if format != SAVE_FORMAT {
        return Err(SaveError::Format {
            found: format,
            expected: SAVE_FORMAT,
        });
    }
    let (header, rest) = postcard::take_from_bytes::<SaveHeader>(bytes)
        .map_err(|e| corrupt(format!("header: {e}")))?;
    Ok((header, rest))
}

/// Writes the whole simulation state (GP-12): a [`SaveHeader`], then the
/// zstd-compressed payload: the SpawnIds of every simulation entity
/// (ascending), every registered resource in registration order, then each
/// registered component type with the entities that have it. Call between
/// ticks.
pub fn save_world(world: &mut World) -> Vec<u8> {
    let _span = tracing::info_span!("sim_core::save_world").entered();
    let mut entities: Vec<(SpawnId, Entity)> = world
        .query::<(Entity, &SpawnId)>()
        .iter(world)
        .map(|(e, id)| (*id, e))
        .collect();
    entities.sort_unstable();
    let ids: HashMap<Entity, SpawnId> = entities.iter().map(|&(id, e)| (e, id)).collect();
    let index: HashMap<SpawnId, u32> = entities
        .iter()
        .enumerate()
        .map(|(i, &(id, _))| (id, i as u32))
        .collect();
    let world: &World = world;
    let empty = SaveRegistry::default();
    let reg = world.get_resource::<SaveRegistry>().unwrap_or(&empty);

    let mut w = SaveWriter::new(&ids);
    w.count(entities.len());
    for (id, _) in &entities {
        w.u64(id.0);
    }
    w.count(reg.resources.len());
    for res in &reg.resources {
        w.str(res.name);
        let present = (res.present)(world);
        w.bool(present);
        let mut blob = SaveWriter::new(&ids);
        if present {
            (res.save)(world, &mut blob);
        }
        w.section(blob);
    }
    w.count(reg.components.len());
    for comp in &reg.components {
        w.str(comp.name);
        let mut blob = SaveWriter::new(&ids);
        let count = (comp.save)(world, &index, &mut blob);
        w.u32(count);
        w.section(blob);
    }
    let payload = w.into_bytes();

    let header = SaveHeader {
        magic: SAVE_MAGIC,
        format: SAVE_FORMAT,
        build: env!("CARGO_PKG_VERSION").to_string(),
        seed: world.get_resource::<SimSeed>().map_or(0, |s| s.0),
        preset: reg.preset.map_or_else(String::new, |f| f(world)),
        run: world.get_resource::<RunState>().and_then(|s| s.config),
        tick: world.get_resource::<SimTick>().map_or(0, |t| t.0),
    };
    let mut out = postcard::to_stdvec(&header).expect("the header serialises");
    out.extend(compress(&payload));
    out
}

fn compress(payload: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut enc = zstd::stream::Encoder::new(Vec::new(), ZSTD_LEVEL).expect("zstd encoder starts");
    enc.include_checksum(true).expect("zstd checksum flag");
    enc.write_all(payload).expect("writing to memory");
    enc.finish().expect("zstd finishes in memory")
}

/// A payload split into its sections, checked against the registry before
/// anything in the world changes.
struct Parsed<'a> {
    ids: Vec<u64>,
    resources: Vec<(bool, &'a [u8])>,
    components: Vec<(u32, &'a [u8])>,
}

fn parse<'a>(
    payload: &'a [u8],
    none: &'a HashMap<u64, Entity>,
    reg: &SaveRegistry,
) -> Result<Parsed<'a>, SaveError> {
    let mut r = SaveReader::new(payload, none);
    let n = r.count()?;
    let ids = (0..n).map(|_| r.u64()).collect::<Result<Vec<_>, _>>()?;
    if ids.windows(2).any(|w| w[0] >= w[1]) {
        return Err(corrupt("SpawnIds are not ascending"));
    }
    let n = r.count()?;
    if n != reg.resources.len() {
        return Err(SaveError::Mismatch(format!(
            "{n} saved resources, this build saves {}",
            reg.resources.len()
        )));
    }
    let mut resources = Vec::with_capacity(n);
    for res in &reg.resources {
        let name = r.str()?;
        if name != res.name {
            return Err(SaveError::Mismatch(format!(
                "resource {name} where {} was expected",
                res.name
            )));
        }
        let present = r.bool()?;
        resources.push((present, r.bytes()?));
    }
    let n = r.count()?;
    if n != reg.components.len() {
        return Err(SaveError::Mismatch(format!(
            "{n} saved components, this build saves {}",
            reg.components.len()
        )));
    }
    let mut components = Vec::with_capacity(n);
    for comp in &reg.components {
        let name = r.str()?;
        if name != comp.name {
            return Err(SaveError::Mismatch(format!(
                "component {name} where {} was expected",
                comp.name
            )));
        }
        let count = r.u32()?;
        components.push((count, r.bytes()?));
    }
    r.finish("payload")?;
    Ok(Parsed {
        ids,
        resources,
        components,
    })
}

/// Restores a world written by [`save_world`] (GP-12). The world must have
/// been built by the same plugins (`sim_headless::load_app` builds one
/// without generating a map). It checks the header, the format and the
/// registered types before changing anything; then it despawns any
/// simulation entities, clears pending commands and events, spawns one
/// entity per saved SpawnId, restores resources (in registration order)
/// and components, and runs the [`AppSaveExt::on_load`] hooks. If a value
/// fails to load after that, the world is partly loaded: discard it.
pub fn load_world(world: &mut World, bytes: &[u8]) -> Result<(), SaveError> {
    let _span = tracing::info_span!("sim_core::load_world").entered();
    let (_, compressed) = split(bytes)?;
    let payload =
        zstd::stream::decode_all(compressed).map_err(|e| corrupt(format!("zstd: {e}")))?;
    let reg = world.remove_resource::<SaveRegistry>().unwrap_or_default();
    let result = load_payload(world, &payload, &reg);
    let hooks = reg.hooks.clone();
    world.insert_resource(reg);
    result?;
    for hook in hooks {
        hook(world);
    }
    Ok(())
}

fn load_payload(world: &mut World, payload: &[u8], reg: &SaveRegistry) -> Result<(), SaveError> {
    let none = HashMap::new();
    let parsed = parse(payload, &none, reg)?;

    // Out with the old world.
    let old: Vec<Entity> = world
        .query_filtered::<Entity, With<SpawnId>>()
        .iter(world)
        .collect();
    for e in old {
        world.despawn(e);
    }
    if let Some(mut m) = world.get_resource_mut::<Messages<SimCommand>>() {
        m.clear();
    }
    if let Some(mut m) = world.get_resource_mut::<Messages<SimEvent>>() {
        m.clear();
    }
    world.insert_resource(CommandScript::default());
    world.insert_resource(CommandLog::default());

    let entities: Vec<Entity> = parsed
        .ids
        .iter()
        .map(|&id| world.spawn(SpawnId(id)).id())
        .collect();
    let by_id: HashMap<u64, Entity> = parsed
        .ids
        .iter()
        .copied()
        .zip(entities.iter().copied())
        .collect();

    for (res, (present, blob)) in reg.resources.iter().zip(&parsed.resources) {
        if *present {
            let mut r = SaveReader::new(blob, &by_id);
            (res.load)(world, &mut r)?;
            r.finish(res.name)?;
        }
    }
    for (comp, (count, blob)) in reg.components.iter().zip(&parsed.components) {
        let mut r = SaveReader::new(blob, &by_id);
        (comp.load)(world, &entities, *count, &mut r)?;
        r.finish(comp.name)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AppHashExt, Chronicle, NameTable, NextSpawnId, Position, SimCorePlugin, SimStepper,
        compute_state_hash,
    };

    #[derive(Component, Debug, Clone, PartialEq)]
    struct Link {
        to: Entity,
        weight: f32,
        path: Vec<IVec2>,
        note: Option<String>,
    }
    save_fields!(Link {
        to,
        weight,
        path,
        note
    });
    impl crate::StateHash for Link {
        fn hash_state(&self, h: &mut crate::StateHasher) {
            h.entity(self.to);
            h.f32(self.weight);
            for p in &self.path {
                h.ivec2(*p);
            }
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(SimCorePlugin { seed: 7, run: None })
            .register_hashed::<Link>()
            .register_saved::<Link>()
            .prepare_sim();
        app
    }

    /// GP-12: a world saved and loaded into a fresh app hashes the same,
    /// entity references follow their SpawnIds, and both keep agreeing.
    #[test]
    fn a_saved_world_loads_identically() {
        let mut a = app();
        a.step(3);
        let world = a.world_mut();
        world.resource_mut::<NameTable>().push("Ashford");
        let ids: Vec<SpawnId> = (0..3)
            .map(|_| world.resource_mut::<NextSpawnId>().allocate())
            .collect();
        let first = world.spawn((ids[0], Position(IVec2::new(1, 2)))).id();
        let doomed = world.spawn(ids[1]).id();
        world.spawn((
            ids[2],
            Link {
                to: first,
                weight: 0.25,
                path: vec![IVec2::ONE, IVec2::X],
                note: Some("ok".into()),
            },
        ));
        world.spawn((
            Position(IVec2::ONE),
            Link {
                to: doomed,
                weight: 1.0,
                path: Vec::new(),
                note: None,
            },
        ));
        world.despawn(doomed);
        let before = compute_state_hash(a.world_mut());
        let bytes = save_world(a.world_mut());

        let header = read_header(&bytes).unwrap();
        assert_eq!(&header.magic, b"WLSV");
        assert_eq!((header.seed, header.tick, header.run), (7, 3, None));

        let mut b = app();
        load_world(b.world_mut(), &bytes).unwrap();
        assert_eq!(compute_state_hash(b.world_mut()), before);
        let mut q = b.world_mut().query::<(&SpawnId, &Link)>();
        let links: Vec<_> = q.iter(b.world()).map(|(i, l)| (*i, l.clone())).collect();
        assert_eq!(links.len(), 1, "entities without a SpawnId are not saved");
        let to = links[0].1.to;
        assert_eq!(b.world().get::<SpawnId>(to), Some(&ids[0]));
        assert_eq!(
            b.world().get::<Position>(to),
            Some(&Position(IVec2::new(1, 2)))
        );
        assert_eq!(
            b.world().resource::<NameTable>().get(crate::NameId(0)),
            "Ashford"
        );

        a.step(5);
        b.step(5);
        assert_eq!(
            compute_state_hash(a.world_mut()),
            compute_state_hash(b.world_mut())
        );
        assert_eq!(
            a.world().resource::<Chronicle>(),
            b.world().resource::<Chronicle>()
        );
    }

    #[test]
    fn bad_saves_are_refused_before_anything_changes() {
        let mut a = app();
        a.step(2);
        let bytes = save_world(a.world_mut());
        let mut b = app();
        b.step(1);
        let hash = compute_state_hash(b.world_mut());

        assert_eq!(load_world(b.world_mut(), b"nope"), Err(SaveError::NotASave));
        let mut other = bytes.clone();
        other[4] = SAVE_FORMAT as u8 + 1;
        assert!(matches!(
            load_world(b.world_mut(), &other),
            Err(SaveError::Format { .. })
        ));
        let mut damaged = bytes.clone();
        let last = damaged.len() - 6;
        damaged[last] ^= 0xff;
        assert!(load_world(b.world_mut(), &damaged).is_err());
        let truncated = &bytes[..bytes.len() - 10];
        assert!(load_world(b.world_mut(), truncated).is_err());
        assert_eq!(compute_state_hash(b.world_mut()), hash, "nothing changed");

        let mut fewer = App::new();
        fewer
            .add_plugins(SimCorePlugin { seed: 7, run: None })
            .prepare_sim();
        assert!(matches!(
            load_world(fewer.world_mut(), &bytes),
            Err(SaveError::Mismatch(_))
        ));
    }

    /// M4a: a save of format 1 (the M4 build) is refused with the
    /// existing message, before anything changes.
    #[test]
    fn a_format_1_save_is_refused() {
        assert_eq!(SAVE_FORMAT, 2, "M4a bumps the format once, to 2");
        let mut a = app();
        a.step(1);
        let mut bytes = save_world(a.world_mut());
        // The header is postcard right after the magic: `[magic; 4]` as four
        // bytes, then the format as a varint (1 and 2 are one byte each).
        assert_eq!(bytes[4], SAVE_FORMAT as u8);
        bytes[4] = 1;
        let hash = compute_state_hash(a.world_mut());
        let err = load_world(a.world_mut(), &bytes).unwrap_err();
        assert_eq!(
            err,
            SaveError::Format {
                found: 1,
                expected: 2
            }
        );
        assert_eq!(
            err.to_string(),
            "save format 1 is not supported (expected 2)"
        );
        assert_eq!(compute_state_hash(a.world_mut()), hash, "nothing changed");
    }

    #[test]
    fn readers_refuse_short_and_bad_data() {
        let none = HashMap::new();
        let mut r = SaveReader::new(&[1, 2], &none);
        assert!(r.u32().is_err());
        let mut r = SaveReader::new(&[2], &none);
        assert!(r.bool().is_err());
        let mut r = SaveReader::new(&[5], &none);
        assert!(Option::<u8>::load(&mut r).is_err());
    }
}
