//! The influence overlay's field (UI-12, GP-31; M4a step 3, ADR 0020): the
//! same field the sim acts on, drawn from each town's [`Influence`].
//!
//! Pure data and arithmetic, with no ECS: [`InfluenceView::build`] takes the
//! towns the overlay copied out of the sim (every two seconds while the mode
//! is shown) and keeps
//!
//! - a coarse grid of the player's and the rival's shares ([`CELL`] tiles
//!   apart, from `influence::field_at`, quantised so a tiny change never
//!   rebuilds terrain), which tints the terrain: your colour to the rival's,
//!   the front colour where the shares meet (|net| under `influence.front`),
//!   fading to nothing where nobody pulls;
//! - the dashed sphere edges (the contour of net `influence.sphere`, yours,
//!   and of `-sphere`, the rival's), cut from the grid by marching squares;
//! - the rings around independent towns being warmed, filling with their
//!   goodwill toward the invitation line (`invitations.goodwill`).
//!
//! The tint is read by terrain through `Overlay::tint` and
//! `Overlay::chunk_signature`; the dashes and rings are drawn with gizmos
//! by `overlay::influence_marks`.

use bevy::prelude::*;
use sim_core::FactionId;
use sim_military::influence::{Influence, InfluenceData, Shares, Source, field_at};

use super::settlement_radius;

/// Tiles between grid nodes.
pub const CELL: u32 = 8;

/// Shares are kept in 1/`LEVELS`, so equal grids tint identically.
const LEVELS: f32 = 64.0;

/// How strongly the field covers the terrain where someone pulls hard.
const STRENGTH: f32 = 0.5;

/// The pull (both shares added) at which the tint reaches full strength.
const FULL_PULL: f32 = 0.35;

/// Tiles in one dash (and one gap) of a sphere edge.
const DASH: f32 = 2.0;

/// A ring's radius beyond its town's disc, in tiles.
const RING_MARGIN: f32 = 3.0;

/// Rings keep their fill in 1/`FILL_STEPS`.
const FILL_STEPS: f32 = 32.0;

/// One town as the view reads it, copied out of the sim.
#[derive(Debug, Clone, Copy)]
pub struct TownView {
    /// Its SpawnId.
    pub id: u64,
    /// Its centre tile.
    pub center: IVec2,
    /// Its people (the size of its ring).
    pub people: u32,
    /// Who holds it.
    pub owner: FactionId,
    /// Whether it is the rival's seat (never swayed, never ringed).
    pub seat: bool,
    /// Its goodwill toward the player.
    pub goodwill: f32,
    /// What the sim last computed there.
    pub influence: Influence,
}

/// The colours the field is drawn in, linear RGBA.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Palette {
    /// The player's side.
    pub player: Vec4,
    /// The rival's side.
    pub rival: Vec4,
    /// Where the two meet.
    pub front: Vec4,
}

impl Palette {
    /// The colour of a place whose net is `net`: the rival's colour at a
    /// net of `-front` or less, the front colour at 0, the player's at
    /// `front` or more.
    pub fn hue(&self, net: f32, front: f32) -> Vec4 {
        let t = ((net + front) / (2.0 * front.max(1e-3))).clamp(0.0, 1.0);
        if t < 0.5 {
            self.rival.lerp(self.front, t * 2.0)
        } else {
            self.front.lerp(self.player, (t - 0.5) * 2.0)
        }
    }
}

/// Whose sphere an edge bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Camp {
    /// The player's (net at least `sphere`).
    Player,
    /// The rival's (net at most `-sphere`).
    Rival,
}

/// One dash of a sphere's edge, in tiles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dash {
    /// Where it starts.
    pub from: Vec2,
    /// Where it ends.
    pub to: Vec2,
    /// Whose edge it is.
    pub camp: Camp,
}

/// How far along a ring is toward an invitation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingState {
    /// The town warms (net at least `warming`) but lies outside the
    /// sphere: envoys cannot come yet.
    Warming,
    /// Inside the sphere: envoys come once it likes you enough.
    InSphere,
    /// It sends envoys today.
    Envoys,
}

/// A ring around an independent town being warmed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ring {
    /// The town's centre, in tiles.
    pub center: Vec2,
    /// Radius, in tiles.
    pub radius: f32,
    /// How much of the ring is drawn, 0..1: its goodwill toward you over
    /// the invitation line.
    pub fill: f32,
    /// How near the invitation is.
    pub state: RingState,
}

/// The influence overlay of one moment: the share grid, the sphere edges
/// and the rings.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InfluenceView {
    /// Nodes per side.
    side: usize,
    /// (player, rival) share at each node, row by row, in 1/[`LEVELS`].
    nodes: Vec<[u8; 2]>,
    /// The colours.
    palette: Palette,
    /// `influence.front`.
    front: f32,
    /// The dashed sphere edges, yours and the rival's.
    pub dashes: Vec<Dash>,
    /// The rings.
    pub rings: Vec<Ring>,
}

fn quantise(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * LEVELS).round() as u8
}

/// 0 below `x = 0`, 1 above `x = 1`, smooth between.
fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl InfluenceView {
    /// Builds the view of a `map_size`-tile island from its towns. The
    /// field is the sim's: the held towns' [`Influence::power`] summed with
    /// `field_at` at every grid node; the rings read each independent
    /// town's own net and goodwill against `invite_goodwill`. Towns are
    /// taken in SpawnId order whatever order they come in.
    pub fn build(
        data: &InfluenceData,
        invite_goodwill: f32,
        towns: &[TownView],
        map_size: u32,
        palette: Palette,
    ) -> Self {
        let mut towns = towns.to_vec();
        towns.sort_unstable_by_key(|t| t.id);
        // Tile centres, as the disc overlays are drawn.
        let sources: Vec<Source> = towns
            .iter()
            .filter_map(|t| Source::of(t.id, t.center, t.owner, &t.influence))
            .map(|s| Source {
                at: s.at + Vec2::splat(0.5),
                ..s
            })
            .collect();
        let side = map_size.div_ceil(CELL) as usize + 1;
        let mut nodes = Vec::with_capacity(side * side);
        for j in 0..side {
            let y = (j as u32 * CELL) as f32;
            // A source beyond its reach of the row adds exactly nothing.
            let row: Vec<&Source> = sources
                .iter()
                .filter(|s| (s.at.y - y).abs() < data.reach)
                .collect();
            for i in 0..side {
                let at = Vec2::new((i as u32 * CELL) as f32, y);
                let s = field_at(data, row.iter().copied(), at);
                nodes.push([quantise(s.player), quantise(s.rival)]);
            }
        }
        let mut view = Self {
            side,
            nodes,
            palette,
            front: data.front,
            dashes: Vec::new(),
            rings: Vec::new(),
        };
        view.dashes = view.edges(data.sphere);
        view.rings = rings(data, invite_goodwill, &towns);
        view
    }

    /// The colours the field is drawn in.
    pub fn palette(&self) -> Palette {
        self.palette
    }

    /// The shares at node `(i, j)`.
    fn node(&self, i: usize, j: usize) -> Shares {
        let [p, r] = self.nodes[j * self.side + i];
        Shares {
            player: f32::from(p) / LEVELS,
            rival: f32::from(r) / LEVELS,
        }
    }

    /// The shares at tile coordinate `(x, y)`, bilinear between the four
    /// nearest nodes; the grid's edge continues outward.
    pub fn shares_at(&self, x: f32, y: f32) -> Shares {
        if self.nodes.is_empty() {
            return Shares::default();
        }
        let last = (self.side - 1) as f32;
        let (gx, gy) = (
            (x / CELL as f32).clamp(0.0, last),
            (y / CELL as f32).clamp(0.0, last),
        );
        let (i, j) = (
            (gx.floor() as usize).min(self.side - 2),
            (gy.floor() as usize).min(self.side - 2),
        );
        let (fx, fy) = (gx - i as f32, gy - j as f32);
        let (a, b, c, d) = (
            self.node(i, j),
            self.node(i + 1, j),
            self.node(i + 1, j + 1),
            self.node(i, j + 1),
        );
        let mix = |a: f32, b: f32, c: f32, d: f32| {
            let top = a + (b - a) * fx;
            let bottom = d + (c - d) * fx;
            top + (bottom - top) * fy
        };
        Shares {
            player: mix(a.player, b.player, c.player, d.player),
            rival: mix(a.rival, b.rival, c.rival, d.rival),
        }
    }

    /// `base` tinted by the field at corner `(x, y)`: mixed toward
    /// [`Palette::hue`] by up to half, fading to nothing where nobody
    /// pulls.
    pub fn tint(&self, x: f32, y: f32, base: Vec4) -> Vec4 {
        if self.nodes.is_empty() {
            return base;
        }
        let s = self.shares_at(x, y);
        let cover = smooth((s.player + s.rival) / FULL_PULL);
        if cover <= 0.0 {
            return base;
        }
        let hue = self.palette.hue(s.net(), self.front);
        let a = STRENGTH * cover;
        base * (1.0 - a) + hue * a
    }

    /// Whether both views tint terrain the same way (the grid, the colours
    /// and the front); dashes and rings are drawn by gizmos and never
    /// rebuild terrain.
    pub fn same_tint(&self, other: &Self) -> bool {
        self.side == other.side
            && self.nodes == other.nodes
            && self.palette == other.palette
            && self.front == other.front
    }

    /// Folds the nodes a chunk's vertices read (the footprint `origin` and
    /// `side`, tiles) into `h`. A chunk nobody pulls on adds nothing, so
    /// its signature is the one it has with no field at all.
    pub fn hash_chunk(&self, origin: Vec2, side: f32, h: &mut u64) {
        if self.nodes.is_empty() {
            return;
        }
        let cell = CELL as f32;
        let last = self.side - 1;
        let lo = |v: f32| ((v / cell).floor().max(0.0) as usize).min(last);
        let (i0, i1) = (lo(origin.x), (lo(origin.x + side) + 1).min(last));
        let (j0, j1) = (lo(origin.y), (lo(origin.y + side) + 1).min(last));
        let mut touched = false;
        for j in j0..=j1 {
            for i in i0..=i1 {
                let [p, r] = self.nodes[j * self.side + i];
                if p == 0 && r == 0 {
                    continue;
                }
                touched = true;
                let v = (j * self.side + i) as u64 * 0x1_0000 + u64::from(p) * 0x100 + u64::from(r);
                *h = (*h ^ v).wrapping_mul(0x100_0000_01b3);
            }
        }
        if touched {
            for v in self
                .palette
                .player
                .to_array()
                .into_iter()
                .chain(self.palette.rival.to_array())
                .chain(self.palette.front.to_array())
                .chain([self.front])
            {
                *h = (*h ^ u64::from(v.to_bits())).wrapping_mul(0x100_0000_01b3);
            }
        }
    }

    /// The dashed sphere edges: the contour of net `+sphere` (yours) and of
    /// `-sphere` (the rival's) through the grid, by marching squares, each
    /// cut into dashes [`DASH`] tiles long with a gap of the same.
    fn edges(&self, sphere: f32) -> Vec<Dash> {
        let mut out = Vec::new();
        if self.side < 2 || sphere <= 0.0 {
            return out;
        }
        for (level, camp) in [(sphere, Camp::Player), (-sphere, Camp::Rival)] {
            let inside = |v: f32| match camp {
                Camp::Player => v >= level,
                Camp::Rival => v <= level,
            };
            for j in 0..self.side - 1 {
                for i in 0..self.side - 1 {
                    let corners = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
                    let v: [f32; 4] = corners.map(|(x, y)| self.node(x, y).net());
                    let at = |k: usize| {
                        let (x, y) = corners[k];
                        Vec2::new((x as u32 * CELL) as f32, (y as u32 * CELL) as f32)
                    };
                    // The crossing on the edge from corner k to k + 1.
                    let cross = |k: usize| -> Option<Vec2> {
                        let (a, b) = (k, (k + 1) % 4);
                        (inside(v[a]) != inside(v[b])).then(|| {
                            let t = (level - v[a]) / (v[b] - v[a]);
                            at(a).lerp(at(b), t.clamp(0.0, 1.0))
                        })
                    };
                    let points: Vec<(usize, Vec2)> =
                        (0..4).filter_map(|k| cross(k).map(|p| (k, p))).collect();
                    match points.len() {
                        2 => dashes(points[0].1, points[1].1, camp, &mut out),
                        4 => {
                            // A saddle: the centre decides which corners
                            // the two segments cut off.
                            let centre = v.iter().sum::<f32>() / 4.0;
                            // The crossings come in edge order: AB, BC, CD, DA.
                            let seg = |a: usize, b: usize| (points[a].1, points[b].1);
                            let pairs = if inside(v[0]) == inside(centre) {
                                // Corners B and D are cut off.
                                [seg(0, 1), seg(2, 3)]
                            } else {
                                // Corners A and C are cut off.
                                [seg(3, 0), seg(1, 2)]
                            };
                            for (a, b) in pairs {
                                dashes(a, b, camp, &mut out);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        out
    }
}

/// Cuts the segment `from -> to` into dashes: an even number of pieces,
/// roughly [`DASH`] tiles each, drawing the first, third, fifth and so on,
/// so every segment ends in a gap and neighbours join into a regular
/// pattern.
fn dashes(from: Vec2, to: Vec2, camp: Camp, out: &mut Vec<Dash>) {
    let len = from.distance(to);
    if len < 1e-3 {
        return;
    }
    let pieces = 2 * ((len / (2.0 * DASH)).round() as usize).max(1);
    for k in (0..pieces).step_by(2) {
        out.push(Dash {
            from: from.lerp(to, k as f32 / pieces as f32),
            to: from.lerp(to, (k + 1) as f32 / pieces as f32),
            camp,
        });
    }
}

/// The rings: one around each independent town (not the seat) the player
/// warms (net at least `warming`), filling with its goodwill toward the
/// invitation line.
fn rings(data: &InfluenceData, invite_goodwill: f32, towns: &[TownView]) -> Vec<Ring> {
    towns
        .iter()
        .filter(|t| t.owner == FactionId::INDEPENDENT && !t.seat)
        .filter(|t| t.influence.net() >= data.warming)
        .map(|t| {
            let fill = if invite_goodwill > 0.0 {
                (t.goodwill / invite_goodwill).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let state = if t.influence.envoy {
                RingState::Envoys
            } else if t.influence.net() >= data.sphere {
                RingState::InSphere
            } else {
                RingState::Warming
            };
            Ring {
                center: t.center.as_vec2() + Vec2::splat(0.5),
                radius: settlement_radius(t.people) + RING_MARGIN,
                fill: (fill * FILL_STEPS).round() / FILL_STEPS,
                state,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: u32 = 1024;

    fn palette() -> Palette {
        Palette {
            player: Vec4::new(0.0, 0.0, 1.0, 1.0),
            rival: Vec4::new(1.0, 0.0, 0.0, 1.0),
            front: Vec4::new(1.0, 1.0, 0.0, 1.0),
        }
    }

    fn data() -> InfluenceData {
        InfluenceData::default()
    }

    /// A held town with the power of `people` at loyalty 0.7.
    fn held(id: u64, x: i32, owner: FactionId, people: u32) -> TownView {
        let d = data();
        let power = sim_military::influence::power(&d, people, 0.7, 0.0, false);
        TownView {
            id,
            center: IVec2::new(x, 500),
            people,
            owner,
            seat: false,
            goodwill: 0.0,
            influence: Influence {
                power,
                ..Default::default()
            },
        }
    }

    /// An independent town, with the net a field gives it.
    fn free(id: u64, x: i32, goodwill: f32, net: f32) -> TownView {
        TownView {
            id,
            center: IVec2::new(x, 500),
            people: 100,
            owner: FactionId::INDEPENDENT,
            seat: false,
            goodwill,
            influence: Influence {
                player: net.max(0.0),
                rival: (-net).max(0.0),
                ..Default::default()
            },
        }
    }

    fn view(towns: &[TownView]) -> InfluenceView {
        InfluenceView::build(&data(), 0.5, towns, SIZE, palette())
    }

    fn near(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    /// The grid holds the sim's field: at a node, the shares are
    /// `field_at` of the held towns (at tile centres), to one level.
    #[test]
    fn the_grid_holds_the_sims_field() {
        let towns = [
            held(1, 300, FactionId::PLAYER, 300),
            held(2, 520, FactionId::RIVAL, 450),
            free(3, 410, 0.0, 0.0),
        ];
        let v = view(&towns);
        let d = data();
        let sources: Vec<Source> = towns
            .iter()
            .filter_map(|t| Source::of(t.id, t.center, t.owner, &t.influence))
            .map(|s| Source {
                at: s.at + Vec2::splat(0.5),
                ..s
            })
            .collect();
        for (x, y) in [(304, 504), (408, 496), (520, 504), (0, 0), (1024, 1024)] {
            let want = field_at(&d, &sources, Vec2::new(x as f32, y as f32));
            let (i, j) = ((x / CELL as i32) as usize, (y / CELL as i32) as usize);
            let got = v.node(i, j);
            assert!(near(got.player, want.player, 1.0 / LEVELS), "({x},{y})");
            assert!(near(got.rival, want.rival, 1.0 / LEVELS), "({x},{y})");
        }
        // At a node the bilinear sample is the node.
        let n = v.node(38, 62);
        let s = v.shares_at(38.0 * CELL as f32, 62.0 * CELL as f32);
        assert!(near(s.player, n.player, 1e-6) && near(s.rival, n.rival, 1e-6));
        // Out past the island the edge continues; never a panic.
        let _ = v.shares_at(-50.0, 5000.0);
    }

    /// Between nodes the shares are the mean of the corners, so the tint
    /// never steps at a cell border.
    #[test]
    fn shares_blend_between_nodes() {
        let v = view(&[held(1, 300, FactionId::PLAYER, 300)]);
        let (a, b) = (v.node(37, 62), v.node(38, 62));
        let mid = v.shares_at(37.5 * CELL as f32, 62.0 * CELL as f32);
        assert!(near(mid.player, (a.player + b.player) / 2.0, 1e-6));
        assert!(near(mid.rival, 0.0, 1e-6));
    }

    /// UI-12: your colour near your towns, the rival's near theirs, the
    /// front colour where the shares meet, and the bare terrain where
    /// nobody pulls.
    #[test]
    fn the_tint_runs_from_your_colour_to_the_rivals() {
        let towns = [
            held(1, 200, FactionId::PLAYER, 300),
            held(2, 440, FactionId::RIVAL, 300),
        ];
        let v = view(&towns);
        let base = Vec4::new(0.3, 0.3, 0.3, 1.0);
        let at = |x: f32| v.tint(x, 500.5, base);
        let (near_you, near_them) = (at(205.0), at(436.0));
        assert!(near_you.z > near_you.x + 0.1, "bluish: {near_you:?}");
        assert!(near_them.x > near_them.z + 0.1, "reddish: {near_them:?}");
        // Equal towns meet in the middle: the front colour is yellow here.
        let mid = at(321.0);
        assert!(
            mid.x > mid.z + 0.05 && mid.y > mid.z + 0.05,
            "the front is yellow: {mid:?}"
        );
        assert!(mid.y > near_you.y && mid.y > near_them.y);
        // Nobody pulls beyond the reach of either: bare terrain.
        assert_eq!(v.tint(900.0, 100.0, base), base);
        // No field at all: bare terrain.
        assert_eq!(InfluenceView::default().tint(300.0, 500.0, base), base);
        // Alpha stays opaque.
        assert_eq!(near_you.w, 1.0);
    }

    /// The hue passes your colour, the front and the rival's, and clamps.
    #[test]
    fn the_hue_has_a_front_between_the_sides() {
        let p = palette();
        assert_eq!(p.hue(0.5, 0.1), p.player);
        assert_eq!(p.hue(-0.5, 0.1), p.rival);
        assert_eq!(p.hue(0.0, 0.1), p.front);
        assert_eq!(p.hue(0.1, 0.1), p.player);
        let half = p.hue(0.05, 0.1);
        assert!(half.z > 0.4 && half.x > 0.4, "between yellow and blue");
    }

    /// The sphere's edge is the net-0.5 contour: dashes exist, lie where
    /// the interpolated net is the sphere's (to a quantisation step) and
    /// are shorter than the pieces they came from; a rival edge appears
    /// for a rival town; two even dashes in a row leave a gap.
    #[test]
    fn the_sphere_edge_follows_the_half_net_contour() {
        let towns = [
            held(1, 250, FactionId::PLAYER, 400),
            held(2, 700, FactionId::RIVAL, 400),
        ];
        let v = view(&towns);
        let d = data();
        let yours: Vec<&Dash> = v.dashes.iter().filter(|d| d.camp == Camp::Player).collect();
        let theirs: Vec<&Dash> = v.dashes.iter().filter(|d| d.camp == Camp::Rival).collect();
        println!("dashes: {} yours, {} theirs", yours.len(), theirs.len());
        assert!(yours.len() > 8 && theirs.len() > 8);
        for dash in &yours {
            for p in [dash.from, dash.to] {
                let net = v.shares_at(p.x, p.y).net();
                // Marching squares interpolates linearly; the field is
                // not linear in a cell, so allow a little.
                assert!(near(net, d.sphere, 0.06), "yours at {p:?}: {net}");
            }
        }
        for dash in &theirs {
            for p in [dash.from, dash.to] {
                let net = v.shares_at(p.x, p.y).net();
                assert!(near(net, -d.sphere, 0.06), "theirs at {p:?}: {net}");
            }
        }
        // Yours rings your town, theirs rings theirs.
        let mean =
            |ds: &[&Dash]| ds.iter().map(|d| (d.from + d.to) / 2.0).sum::<Vec2>() / ds.len() as f32;
        assert!(near(mean(&yours).x, 250.0, 25.0), "{:?}", mean(&yours));
        assert!(near(mean(&theirs).x, 700.0, 25.0), "{:?}", mean(&theirs));
        // Dashes are short, and never touch end to end within a segment.
        assert!(
            v.dashes
                .iter()
                .all(|d| d.from.distance(d.to) <= 2.0 * CELL as f32)
        );
    }

    /// A dash pattern: an even count of pieces, the first, third ... drawn.
    #[test]
    fn a_segment_is_cut_into_dashes_and_gaps() {
        let mut out = Vec::new();
        dashes(Vec2::ZERO, Vec2::new(8.0, 0.0), Camp::Player, &mut out);
        assert_eq!(out.len(), 2, "8 tiles: two dashes, two gaps");
        assert_eq!(out[0].from, Vec2::ZERO);
        assert!(near(out[0].to.x, 2.0, 1e-5));
        assert!(near(out[1].from.x, 4.0, 1e-5) && near(out[1].to.x, 6.0, 1e-5));
        // A tiny segment is one dash and one gap; a degenerate one nothing.
        out.clear();
        dashes(Vec2::ZERO, Vec2::new(1.0, 0.0), Camp::Rival, &mut out);
        assert_eq!(out.len(), 1);
        assert!(near(out[0].to.x, 0.5, 1e-5));
        out.clear();
        dashes(Vec2::ONE, Vec2::ONE, Camp::Rival, &mut out);
        assert!(out.is_empty());
    }

    /// UI-12: the front moves as you take towns. A player town and the
    /// rival's seat meet at a front; taking the town between them moves the
    /// front (where the net is 0 along the line) toward the rival, and the
    /// sphere edge with it.
    #[test]
    fn the_front_moves_as_you_take_towns() {
        let mut seat = held(9, 800, FactionId::RIVAL, 450);
        seat.influence.power *= 2.0;
        let between = held(2, 500, FactionId::INDEPENDENT, 300);
        let you = held(1, 200, FactionId::PLAYER, 300);
        let front_x = |towns: &[TownView]| {
            let v = view(towns);
            // Scan the line for the net's zero crossing between the sides.
            let mut x = 200.0;
            while x < 800.0 && v.shares_at(x, 500.5).net() > 0.0 {
                x += 1.0;
            }
            x
        };
        let before = front_x(&[you, between, seat]);
        let mut taken = between;
        taken.owner = FactionId::PLAYER;
        let after = front_x(&[you, taken, seat]);
        println!("front at x = {before} before, {after} after taking the town between");
        assert!(after > before + 30.0, "{before} -> {after}");

        let edge_x = |towns: &[TownView]| {
            let v = view(towns);
            v.dashes
                .iter()
                .filter(|d| d.camp == Camp::Player)
                .map(|d| d.from.x.max(d.to.x))
                .fold(0.0f32, f32::max)
        };
        let (e0, e1) = (edge_x(&[you, between, seat]), edge_x(&[you, taken, seat]));
        println!("sphere edge reaches x = {e0:.0} before, {e1:.0} after");
        assert!(e1 > e0 + 30.0);
    }

    /// Rings go round independent towns being warmed, never owned ones or
    /// the seat; they fill with goodwill over the invitation line and name
    /// how near the invitation is.
    #[test]
    fn rings_fill_toward_an_invitation() {
        let d = data();
        let mut seat = free(7, 900, 0.3, 0.9);
        seat.seat = true;
        let mut sending = free(5, 400, 0.5, 0.7);
        sending.influence.envoy = true;
        let towns = [
            free(1, 100, 0.25, d.warming + 0.05),
            free(2, 200, 0.1, d.sphere + 0.1),
            free(3, 300, 0.0, d.warming - 0.05),
            held(4, 350, FactionId::PLAYER, 200),
            sending,
            seat,
            free(6, 500, 0.9, 0.6),
        ];
        let v = view(&towns);
        let ring = |x: f32| v.rings.iter().find(|r| r.center.x == x + 0.5).copied();
        let a = ring(100.0).expect("warming");
        assert_eq!(a.state, RingState::Warming);
        assert!(near(a.fill, 0.5, 1e-6), "0.25 of 0.5: {}", a.fill);
        let b = ring(200.0).expect("in the sphere");
        assert_eq!(b.state, RingState::InSphere);
        assert!(near(b.fill, 0.2, 0.02));
        assert!(ring(300.0).is_none(), "not warming yet");
        assert!(ring(350.0).is_none(), "yours");
        assert!(ring(900.0).is_none(), "the seat");
        let c = ring(400.0).expect("envoys");
        assert_eq!((c.state, c.fill), (RingState::Envoys, 1.0));
        let e = ring(500.0).expect("past the line");
        assert_eq!(e.fill, 1.0, "never more than a full ring");
        assert_eq!(v.rings.len(), 4);
        assert!(a.radius > settlement_radius(100));
        // No invitation line (a broken data file): full rings, no panic.
        let v = InfluenceView::build(&d, 0.0, &towns, SIZE, palette());
        assert!(v.rings.iter().all(|r| r.fill == 1.0));
    }

    /// The same towns build the same view whatever order they arrive in;
    /// a change that moves no colour leaves the tint equal (terrain does
    /// not rebuild), a change that moves one does not.
    #[test]
    fn views_are_stable_and_compare_by_tint() {
        let towns = [
            held(1, 300, FactionId::PLAYER, 300),
            held(2, 700, FactionId::RIVAL, 300),
            free(3, 500, 0.1, 0.3),
        ];
        let mut reversed = towns;
        reversed.reverse();
        assert_eq!(view(&towns), view(&reversed));
        // Goodwill moves a ring, not the tint.
        let mut warmer = towns;
        warmer[2].goodwill = 0.3;
        let (a, b) = (view(&towns), view(&warmer));
        assert_ne!(a, b);
        assert!(a.same_tint(&b));
        // More people move the field.
        let mut bigger = towns;
        bigger[0].influence.power *= 1.5;
        assert!(!a.same_tint(&view(&bigger)));
        // A new palette is a new tint.
        let mut other = palette();
        other.player.y = 0.5;
        assert!(!a.same_tint(&InfluenceView::build(&data(), 0.5, &towns, SIZE, other)));
    }

    /// A chunk nobody pulls on keeps the signature it has with no field,
    /// so far-off terrain never rebuilds; one under the field changes with
    /// it.
    #[test]
    fn chunk_signatures_follow_the_field() {
        let seed: u64 = 0xcbf2_9ce4_8422_2325;
        let sig = |v: &InfluenceView, origin: Vec2| {
            let mut h = seed;
            v.hash_chunk(origin, 64.0, &mut h);
            h
        };
        let v = view(&[held(1, 300, FactionId::PLAYER, 300)]);
        assert_eq!(sig(&v, Vec2::new(896.0, 896.0)), seed, "out of reach");
        assert_eq!(
            sig(&InfluenceView::default(), Vec2::new(256.0, 448.0)),
            seed
        );
        let here = sig(&v, Vec2::new(256.0, 448.0));
        assert_ne!(here, seed);
        let mut stronger = held(1, 300, FactionId::PLAYER, 300);
        stronger.influence.power *= 1.3;
        let w = view(&[stronger]);
        assert_ne!(sig(&w, Vec2::new(256.0, 448.0)), here);
        assert_eq!(sig(&v, Vec2::new(256.0, 448.0)), here, "stable");
    }

    /// A map that isn't a multiple of the cell still gets a grid that
    /// reaches its far edge.
    #[test]
    fn the_grid_reaches_the_islands_edge() {
        let v = InfluenceView::build(&data(), 0.5, &[], 100, palette());
        assert_eq!(v.side, 100u32.div_ceil(CELL) as usize + 1);
        assert!(v.dashes.is_empty() && v.rings.is_empty());
        assert_eq!(v.shares_at(100.0, 100.0), Shares::default());
    }
}
