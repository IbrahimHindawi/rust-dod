// 2.1 Components and signatures
//
// Part 1 hard-coded its components: a positions array, a velocities array, a
// flag for each. Every new component meant editing the world. Part 2 makes the
// world work for components it has never heard of, and this chapter lays the
// ground: what a component is, how the world tells components apart without
// knowing their types, and how a set of components is written down.
//
//   * A component is a plain struct.
//   * Each component type gets an id, a small number, decided at compile time.
//   * The world keeps a registry: for each id, the component's reflect record,
//     which says its name, size and alignment. That is all the world needs to
//     store, copy and move a component it knows nothing else about.
//   * A set of components is a signature: a u64 with bit `id` set for each
//     component in the set. Asking whether a set contains another is one AND.
//
// The compiler knows each type's name, size and alignment; a small macro asks
// it to write them down, so declaring a component is one line.
//
// No entities in this chapter. 2.3 puts the two halves together.

// Components are declared here and measured, never read, and from here on each
// chapter carries kayan code that not every program calls. Dead-code warnings
// would be noise.
#![allow(dead_code)]

use std::alloc::Layout;

#[derive(Clone, Copy, Default, Debug)]
struct Vec2 {
    x: f32,
    y: f32,
}

// A reflect record: what the world knows about a component it cannot name.
struct Info {
    name: &'static str,
    layout: Layout,
}

// A component: plain data that knows its id and its reflect record. Copy rules
// out anything that owns memory, a String or a Vec, so a component is its bytes
// and nothing else.
trait Component: Copy + 'static {
    const ID: u32;
    const INFO: &'static Info;
}

// One id per component type: its position in this enum.
#[derive(Clone, Copy)]
enum ComponentId {
    Position,
    Velocity,
    Health,
    Damage,
    Lifetime,
    Burning,
    Turret,
}

// Declares a component: the struct, and its Component impl. The id is the enum
// variant with the same name. The record is written by the compiler, at
// compile time: stringify! gives the name, Layout::new the size and alignment.
macro_rules! component {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Clone, Copy, Default, Debug)]
        struct $name {
            $($field: $ty),*
        }

        impl Component for $name {
            const ID: u32 = ComponentId::$name as u32;
            const INFO: &'static Info = &Info { name: stringify!($name), layout: Layout::new::<$name>() };
        }
    };
}

// The components. Plain data, nothing else: no methods, no references to other
// entities, no knowledge of the world.
component!(Position { at: Vec2 });
component!(Velocity { per_second: Vec2 });
component!(Health { points: f32, max: f32 });
component!(Damage { amount: f32 });
component!(Lifetime { seconds: f32 });
component!(Burning { per_second: f32 });
component!(Turret { cooldown: f32, timer: f32 });

// ---------------------------------------------------------------------------
// The world's side: ids and bits, never types
// ---------------------------------------------------------------------------

// The registry. The world stores components as bytes, so per id it needs to
// know how many bytes, at what alignment, and a name for printing. A reflect
// record has all three.
struct World {
    infos: [Option<&'static Info>; 64],
    component_count: u32,
}

// One bit per component. A signature is a u64, so a world has at most 64.
fn bit(id: u32) -> u64 {
    1 << id
}

// The typed layer: from a type to the bit the world works in.
fn mask_of<T: Component>() -> u64 {
    bit(T::ID)
}

// Does a set contain every component of another? One AND and one compare.
fn contains(signature: u64, wanted: u64) -> bool {
    (signature & wanted) == wanted
}

impl World {
    fn new() -> World {
        World { infos: [None; 64], component_count: 0 }
    }

    // Hands the world T's record under T's id. Generic code can read T::INFO
    // for any component T, so registering needs nothing but the type.
    fn register<T: Component>(&mut self) {
        assert!(T::ID < 64, "component id {} is past the 64 a signature can hold", T::ID);
        self.infos[T::ID as usize] = Some(T::INFO);
        self.component_count = self.component_count.max(T::ID + 1);
    }

    fn info(&self, id: u32) -> &'static Info {
        self.infos[id as usize].expect("every component in a signature is registered")
    }

    // A signature as names, {Position Velocity}.
    fn names(&self, signature: u64) -> String {
        let names: Vec<&str> = (0..self.component_count)
            .filter(|&id| signature & bit(id) != 0)
            .map(|id| self.info(id).name)
            .collect();
        format!("{{{}}}", names.join(" "))
    }

    // A signature as bits, highest component first, so each column of the
    // output is one component.
    fn bits(&self, signature: u64) -> String {
        (0..self.component_count).rev().map(|id| if signature & bit(id) != 0 { '1' } else { '0' }).collect()
    }
}

fn yes(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

fn main() {
    let mut w = World::new();
    w.register::<Position>();
    w.register::<Velocity>();
    w.register::<Health>();
    w.register::<Damage>();
    w.register::<Lifetime>();
    w.register::<Burning>();
    w.register::<Turret>();

    println!("the registry: {} components", w.component_count);
    println!("    id  name       size  align");
    for id in 0..w.component_count {
        let info = w.info(id);
        println!("    {id:2}  {:<9}  {:4}  {:5}", info.name, info.layout.size(), info.layout.align());
    }

    // What a thing is, written as the set of its components.
    let drone = mask_of::<Position>() | mask_of::<Velocity>() | mask_of::<Health>();
    let bullet = mask_of::<Position>() | mask_of::<Velocity>() | mask_of::<Damage>() | mask_of::<Lifetime>();
    let gun = mask_of::<Position>() | mask_of::<Turret>();
    let burning_drone = drone | mask_of::<Burning>();

    println!("signatures, one bit per component, highest id first:");
    println!("    drone          {}  {}", w.bits(drone), w.names(drone));
    println!("    bullet         {}  {}", w.bits(bullet), w.names(bullet));
    println!("    turret         {}  {}", w.bits(gun), w.names(gun));
    println!("    burning drone  {}  {}", w.bits(burning_drone), w.names(burning_drone));

    // Which of them would a movement system want? Everything that contains
    // {Position Velocity}.
    let moves = mask_of::<Position>() | mask_of::<Velocity>();
    println!("movement wants {}:", w.names(moves));
    println!("    drone {}, bullet {}, turret {}, burning drone {}",
        yes(contains(drone, moves)), yes(contains(bullet, moves)),
        yes(contains(gun, moves)), yes(contains(burning_drone, moves)));

    // Adding and removing a component is setting and clearing its bit.
    let put_out = burning_drone & !mask_of::<Burning>();
    println!("a burning drone with burning removed is a drone again: {}", yes(put_out == drone));
}
