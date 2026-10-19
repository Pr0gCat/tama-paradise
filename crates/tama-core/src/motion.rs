//! Motion-script player (firmware `FUN_100016d0`, docs/spec/07-animation.md §2.2).
//!
//! A script is a list of commands run at 20 fps. Each command may wait W frames afterwards.
//! A node runs one script; `spawn_script` adds child nodes (effects such as Zzz or hearts).

/// Script end marker.
const END: u16 = 0xFCBD;
/// Wait value that means "stop here forever"; also "keep current"/"no override" in arguments.
pub const KEEP: i16 = 0x7fff;
/// Max commands run in one frame before giving up (the firmware asserts at 1000).
const MAX_STEPS: usize = 1000;
/// Argument presence bits in the opcode word, in reading order.
const ARGS: [(u16, usize); 6] = [(15, 0), (14, 1), (13, 2), (11, 3), (10, 4), (9, 5)];

/// Character placeholder ids that mean "the player's pet". 0x1f also hides the eyes and mouth.
pub const PET: u16 = 0;
pub const PET_NO_FACE: u16 = 0x1f;
pub const PET_IN_CELL: u16 = 1;
/// The planet (HOME_SPACE character, wearing the planet outfits).
pub const PLANET: u16 = 2;
/// The pet placeholder (chr 0 / 0x1f) with role B > 0 becomes `ROLE + B`: a partner the scene
/// substitutes (travel companions, hunt team, visitors; 07-animation A2).
pub const ROLE: u16 = 0x7f00;
/// The planet placeholder with role B > 0 (`PLANET_ROLE + B`): the rank-up shows old and new looks.
pub const PLANET_ROLE: u16 = 0x7e00;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Tween {
    from: i32,
    to: i32,
    t: u16,
    len: u16,
}

impl Tween {
    fn value(&self) -> i32 {
        if self.t >= self.len {
            self.to
        } else {
            // ponytail: linear; the firmware has curves 0–6 whose formulas are not decoded yet.
            self.from + (self.to - self.from) * self.t as i32 / self.len as i32
        }
    }

    fn start(&mut self, from: i16, to: i16, frames: i16) {
        self.from = if from == KEEP { self.value() } else { from as i32 };
        self.to = to as i32;
        self.t = 0;
        self.len = frames.max(0) as u16;
    }

    fn step(&mut self) {
        self.t = self.t.saturating_add(1);
    }
}

/// What a node shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    None,
    /// Character pose. `eyes`/`mouth` = frame override or `KEEP`.
    Chr {
        chr: u16,
        anim: u16,
        eyes: i16,
        mouth: i16,
    },
    Sprite {
        id: u16,
        frame: u16,
    },
}

#[derive(Clone, Debug)]
pub struct Node {
    script: u16,
    pc: usize,
    wait: u16,
    loop_pc: usize,
    pub done: bool,
    pub look: Look,
    /// Offset from where the node was spawned, pixels.
    x: Tween,
    y: Tween,
    /// 0–256, 256 = opaque.
    alpha: Tween,
    pub flip: bool,
    pub visible: bool,
    /// Palette set of the node's sprite (op 0x03, or set by game code).
    pub palette: u8,
    /// `signal -1`: the scene may move on. `signal 4`: wait for a button press (`resume`).
    pub finished: bool,
    pub paused: bool,
    /// Spawn position relative to the parent, and draw order (negative = behind the parent).
    pub at: (i32, i32),
    pub z: i16,
    pub children: Vec<Node>,
    /// Sounds started this frame: (melody id, volume 0–100).
    pub sounds: Vec<(u16, u8)>,
}

/// Something to draw, in back-to-front order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Draw {
    pub look: Look,
    pub x: i32,
    pub y: i32,
    pub flip: bool,
    /// 0–64 like the blitter.
    pub alpha: u8,
    pub palette: u8,
}

impl Node {
    pub fn new(script: u16) -> Self {
        Node {
            script,
            pc: 1,
            wait: 0,
            loop_pc: 1,
            done: false,
            look: Look::None,
            x: Tween::default(),
            y: Tween::default(),
            alpha: Tween { from: 256, to: 256, t: 0, len: 0 },
            flip: false,
            visible: true,
            palette: 0,
            finished: false,
            paused: false,
            at: (0, 0),
            z: 0,
            children: Vec::new(),
            sounds: Vec::new(),
        }
    }

    pub fn script(&self) -> u16 {
        self.script
    }

    /// Advance one frame.
    pub fn tick(&mut self, scripts: &[Vec<u16>]) {
        for c in &mut self.children {
            c.tick(scripts);
        }
        self.children.retain(|c| !c.done);
        self.x.step();
        self.y.step();
        self.alpha.step();
        if self.done || self.paused || self.wait == KEEP as u16 {
            return;
        }
        if self.wait > 0 {
            self.wait -= 1;
            if self.wait > 0 {
                return;
            }
        }
        let Some(words) = scripts.get(self.script as usize) else {
            self.done = true;
            return;
        };
        let looping = words.first().is_some_and(|w| w & 1 != 0);
        for _ in 0..MAX_STEPS {
            if self.pc >= words.len() || words[self.pc] == END {
                if !looping {
                    self.done = true;
                    return;
                }
                self.pc = self.loop_pc;
                continue;
            }
            let op = words[self.pc];
            self.pc += 1;
            let mut a = [0i16; 6];
            for (bit, slot) in ARGS {
                if op >> bit & 1 != 0 {
                    a[slot] = *words.get(self.pc).unwrap_or(&0) as i16;
                    self.pc += 1;
                }
            }
            self.run(op & 0xff, a);
            // Children spawned this frame get their first tick now, like the firmware.
            for c in self.children.iter_mut().filter(|c| c.pc == 1 && c.wait == 0 && !c.done) {
                c.tick(scripts);
            }
            self.wait = a[5] as u16;
            if self.wait != 0 || self.paused {
                return;
            }
        }
    }

    fn run(&mut self, op: u16, [a, b, c, d, e, _w]: [i16; 6]) {
        match op {
            0x01 => self.look = Look::Sprite { id: a as u16, frame: (c & 0x1ff) as u16 },
            0x02 => {
                if let Look::Sprite { frame, .. } = &mut self.look {
                    *frame = (c & 0x1ff) as u16;
                }
            }
            0x03 => self.palette = c as u8,
            0x04 => self.x.start(c, d, e),
            0x05 => self.y.start(c, d, e),
            0x06 => {
                self.x.start(KEEP, c, e);
                self.y.start(KEEP, d, e);
            }
            0x0a | 0x0c => self.alpha.start(if c == KEEP { KEEP } else { c >> 4 }, d >> 4, e),
            0x0d => self.flip = c & 1 != 0,
            0x0e => self.sounds.push((a as u16, c.clamp(0, 100) as u8)),
            0x10 => {
                let mut child = Node::new(a as u16);
                child.at = (c as i32, d as i32);
                child.z = b;
                self.children.push(child);
            }
            0x14 => self.visible = c & 1 != 0,
            0x1a if c == -1 => self.finished = true,
            0x1a if c == 4 => self.paused = true,
            0x1b => self.loop_pc = self.pc,
            0x1c => {
                let chr = match a as u16 {
                    PET | PET_NO_FACE if b > 0 => ROLE + b as u16,
                    PLANET if b > 0 => PLANET_ROLE + b as u16,
                    c => c,
                };
                self.look = Look::Chr { chr, anim: c as u16, eyes: d, mouth: KEEP };
            }
            0x1d | 0x1e | 0x24 => {
                if let Look::Chr { anim, eyes, mouth, .. } = &mut self.look {
                    match op {
                        0x1d => (*anim, *eyes) = (c as u16, d),
                        0x1e => *eyes = c,
                        _ => *mouth = c,
                    }
                }
            }
            0x20 => self.children.clear(),
            // Scale, rotation, sound, scene effects and actor signals are not used yet.
            _ => {}
        }
    }

    /// Collect and clear the sounds started by this node and its children.
    pub fn take_sounds(&mut self, out: &mut Vec<(u16, u8)>) {
        out.append(&mut self.sounds);
        for c in &mut self.children {
            c.take_sounds(out);
        }
    }

    /// The script (or one of its children) signalled completion, or everything has ended.
    pub fn is_finished(&self) -> bool {
        self.finished || self.done || self.children.iter().any(|c| c.is_finished() && !c.done)
    }

    pub fn is_paused(&self) -> bool {
        self.paused || self.children.iter().any(Node::is_paused)
    }

    /// Continue after a `signal 4` pause.
    pub fn resume(&mut self) {
        self.paused = false;
        for c in &mut self.children {
            c.resume();
        }
    }

    /// Flatten into draw calls (back to front) with the node placed at (x, y).
    pub fn draws(&self, x: i32, y: i32, out: &mut Vec<Draw>) {
        let (px, py) = (x + self.at.0 + self.x.value(), y + self.at.1 + self.y.value());
        let mut kids: Vec<&Node> = self.children.iter().collect();
        kids.sort_by_key(|c| c.z);
        for c in kids.iter().filter(|c| c.z < 0) {
            c.draws_tinted(px, py, self.palette, out);
        }
        if self.visible && self.look != Look::None {
            let alpha = (self.alpha.value().clamp(0, 256) / 4) as u8;
            out.push(Draw { look: self.look, x: px, y: py, flip: self.flip, alpha, palette: self.palette });
        }
        for c in kids.iter().filter(|c| c.z >= 0) {
            c.draws_tinted(px, py, self.palette, out);
        }
    }

    /// Children inherit the parent's palette unless they set their own.
    fn draws_tinted(&self, x: i32, y: i32, palette: u8, out: &mut Vec<Draw>) {
        let start = out.len();
        self.draws(x, y, out);
        if self.palette == 0 {
            for d in &mut out[start..] {
                if d.palette == 0 {
                    d.palette = palette;
                }
            }
        }
    }

    /// Move the node's anchor (game code placing a node).
    pub fn place(&mut self, x: i32, y: i32) {
        self.at = (x, y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u16 = 1 << 9;
    const C: u16 = 1 << 13;
    const D: u16 = 1 << 11;
    const A: u16 = 1 << 15;

    #[test]
    fn plays_poses_with_waits_and_loops() {
        // loop: chr anim 1 (wait 2), anim 2 (wait 3), END
        let s = vec![vec![(2 << 1) | 1, 0x1c | C | W, 1, 2, 0x1d | C | W, 2, 3, END]];
        let mut n = Node::new(0);
        let anim = |n: &Node| match n.look {
            Look::Chr { anim, .. } => anim,
            _ => 0,
        };
        let seen: Vec<u16> = (0..10)
            .map(|_| {
                n.tick(&s);
                anim(&n)
            })
            .collect();
        assert_eq!(seen, [1, 1, 2, 2, 2, 1, 1, 2, 2, 2]);
    }

    #[test]
    fn non_looping_script_finishes_and_children_are_removed() {
        let s = vec![vec![2 << 1, 0x10 | A | C | D, 1, 5, -7i16 as u16, END], vec![1 << 1, 0x01 | A | W, 99, 2, END]];
        let mut n = Node::new(0);
        n.tick(&s);
        assert!(n.done);
        assert_eq!(n.children.len(), 1);
        let mut d = Vec::new();
        n.draws(10, 10, &mut d);
        assert_eq!(d[0].look, Look::Sprite { id: 99, frame: 0 });
        assert_eq!((d[0].x, d[0].y), (15, 3));
        n.tick(&s);
        n.tick(&s);
        assert!(n.children.is_empty());
    }

    #[test]
    fn pause_waits_for_resume_then_finishes() {
        // signal 4 (pause), signal -1 (finished), wait forever
        let s = vec![vec![3 << 1, 0x1a | C, 4, 0x1a | C, -1i16 as u16, W, KEEP as u16, END]];
        let mut n = Node::new(0);
        n.tick(&s);
        assert!(n.is_paused() && !n.is_finished());
        n.tick(&s);
        assert!(n.is_paused());
        n.resume();
        n.tick(&s);
        assert!(n.is_finished() && !n.done);
    }

    #[test]
    fn tween_moves_linearly() {
        let s = vec![vec![1 << 1, 0x04 | C | D | (1 << 10) | W, 0, 10, 5, KEEP as u16, END]];
        let mut n = Node::new(0);
        n.tick(&s);
        for _ in 0..2 {
            n.tick(&s);
        }
        assert_eq!(n.x.value(), 4);
        for _ in 0..5 {
            n.tick(&s);
        }
        assert_eq!(n.x.value(), 10);
    }
}
