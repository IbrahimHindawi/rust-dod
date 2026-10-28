// The harness for dojo 4, a type-erased column. It is fixed. Your work is
// kata.rs, beside it: kata.rs brings this file in as a module and its main
// calls run(), so the two are one program. The reference, solution.rs, and
// each level file do the same, so each of them builds on its own, and this
// file never does. Build and run yours with
//
//   python tools/dojo.py column
//   python tools/dojo.py column --miri       the same under Miri, which checks
//                                            your unsafe lines (nightly)
//
// First a short scenario: rows written through the typed view and read back
// as bytes, a swap-remove, and types the column must refuse. Then a fuzz run
// for each of four row types, from a 3-byte one to a 16-byte one aligned to
// 16: random pushes, writes and swap-removes, each checked against a Vec of
// that type. The model doesn't care how the column stores its bytes, only
// that the rows come back right and aligned, and that a full column grows by
// at least half, so pushing stays cheap on average.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

use crate::dojo::{Rng, Run};
use crate::Column;
use std::fmt;

/// Plain old data: a type a column can hold as bytes and hand back.
///
/// # Safety
///
/// The type is Copy, has no padding bytes, and every bit pattern is a valid
/// value of it.
#[allow(unsafe_code)]
pub unsafe trait Pod: Copy + 'static {}

#[allow(unsafe_code)]
unsafe impl Pod for u32 {}
#[allow(unsafe_code)]
unsafe impl<const N: usize> Pod for [u8; N] {}

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(C)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}
#[allow(unsafe_code)]
unsafe impl Pod for Vec3 {}

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(C, align(16))]
struct Vec4 {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}
#[allow(unsafe_code)]
unsafe impl Pod for Vec4 {}

// A tag: a component with no data, zero bytes. Rows of nothing still count.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Tag;
#[allow(unsafe_code)]
unsafe impl Pod for Tag {}

// A row type for the fuzz run: how to make one from the random numbers, and
// its bytes, written out field by field, so the harness needs no unsafe to
// know what the column should hold.
trait Sample: Pod + PartialEq + fmt::Debug {
    const NAME: &'static str;
    fn make(rng: &mut Rng) -> Self;
    fn bytes(&self) -> Vec<u8>;
}

// Small whole numbers and quarters: exact as f32s, and never NaN, so == works.
fn float(rng: &mut Rng) -> f32 {
    rng.below(4000) as f32 * 0.25
}

impl Sample for u32 {
    const NAME: &'static str = "u32, 4 bytes aligned to 4";
    fn make(rng: &mut Rng) -> u32 {
        rng.next() as u32
    }
    fn bytes(&self) -> Vec<u8> {
        self.to_ne_bytes().to_vec()
    }
}

impl Sample for [u8; 3] {
    const NAME: &'static str = "[u8; 3], 3 bytes aligned to 1";
    fn make(rng: &mut Rng) -> [u8; 3] {
        let n = rng.next().to_ne_bytes();
        [n[0], n[1], n[2]]
    }
    fn bytes(&self) -> Vec<u8> {
        self.to_vec()
    }
}

impl Sample for Vec3 {
    const NAME: &'static str = "Vec3, 12 bytes aligned to 4";
    fn make(rng: &mut Rng) -> Vec3 {
        Vec3 { x: float(rng), y: float(rng), z: float(rng) }
    }
    fn bytes(&self) -> Vec<u8> {
        [self.x, self.y, self.z].iter().flat_map(|f| f.to_ne_bytes()).collect()
    }
}

impl Sample for Vec4 {
    const NAME: &'static str = "Vec4, 16 bytes aligned to 16";
    fn make(rng: &mut Rng) -> Vec4 {
        Vec4 { x: float(rng), y: float(rng), z: float(rng), w: float(rng) }
    }
    fn bytes(&self) -> Vec<u8> {
        [self.x, self.y, self.z, self.w].iter().flat_map(|f| f.to_ne_bytes()).collect()
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

// Whether `f` panics, with the panic message kept off the screen.
fn panics(f: impl FnOnce()) -> bool {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err();
    std::panic::set_hook(hook);
    panicked
}

fn scenario(run: &Run) {
    println!("scenario");
    let mut c = Column::new(size_of::<Vec3>(), align_of::<Vec3>());
    for _ in 0..3 {
        c.push_zeroed();
    }
    run.same("push 3 zeroed rows: len", c.len(), "3");
    run.same("row 2 as bytes", hex(c.row(2)), "00 00 00 00 00 00 00 00 00 00 00 00");
    let rows = [Vec3 { x: 1.0, y: 2.0, z: 3.0 }, Vec3 { x: 4.0, y: 5.0, z: 6.0 }, Vec3 { x: 7.0, y: 8.0, z: 9.0 }];
    c.slice_mut::<Vec3>().copy_from_slice(&rows);
    let view: Vec<String> = c.slice::<Vec3>().iter().map(|v| format!("({} {} {})", v.x, v.y, v.z)).collect();
    run.same("write 3 Vec3s through slice_mut, read them through slice", view.join(" "), "(1 2 3) (4 5 6) (7 8 9)");
    run.same("row 1 as bytes is 4.0, 5.0 and 6.0", hex(c.row(1)), &hex(&rows[1].bytes()));
    c.row_mut(0).copy_from_slice(&Vec3 { x: -1.0, y: -2.0, z: -3.0 }.bytes());
    run.same("write row 0 as bytes, read it as a Vec3", format!("{:?}", c.slice::<Vec3>()[0]), "Vec3 { x: -1.0, y: -2.0, z: -3.0 }");
    c.swap_remove(0);
    run.same("swap_remove row 0: the last row moves in; len", c.len(), "2");
    run.same("row 0 is now", format!("{:?}", c.slice::<Vec3>()[0]), "Vec3 { x: 7.0, y: 8.0, z: 9.0 }");
    run.same("row 2, past the rows in use: panics", panics(|| _ = c.row(2)), "true");
    run.same("read it as u32, the wrong size: panics", panics(|| _ = c.slice::<u32>()), "true");
    run.same("read it as [u8; 12], the right size, aligned to 1: panics", panics(|| _ = c.slice::<[u8; 12]>()), "true");
    run.same("a column aligned to 32: panics", panics(|| _ = Column::new(32, 32)), "true");
    run.same("a column aligned to 3: panics", panics(|| _ = Column::new(3, 3)), "true");
    let mut big = Column::new(16, 16);
    for _ in 0..100 {
        big.push_zeroed();
    }
    run.same("push 100 rows of 16 bytes aligned to 16: len", big.len(), "100");
    let at = big.slice::<Vec4>().as_ptr() as usize;
    run.same("the typed view starts on a 16-byte boundary", at.is_multiple_of(16), "true");
    let mut tags = Column::new(0, 1);
    for _ in 0..3 {
        tags.push_zeroed();
    }
    tags.swap_remove(1);
    run.same("a column of zero-byte tags: push 3, swap_remove 1, tags in slice", tags.slice::<Tag>().len(), "2");
    run.same("a tag's row as bytes", format!("[{}]", hex(tags.row(1))), "[]");
}

// One row type's share of the fuzz run, against a Vec<T>. `op` counts across
// all four types, so --stop finds the same operation every time.
fn fuzz_type<T: Sample>(run: &Run, rng: &mut Rng, op: &mut u64, ops: u64) {
    println!("    {}", T::NAME);
    let mut c = Column::new(size_of::<T>(), align_of::<T>());
    let mut model: Vec<T> = Vec::new();
    for _ in 0..ops {
        *op += 1;
        let op = *op;
        let roll = rng.below(100);
        if roll < 45 || model.is_empty() {
            let value = T::make(rng);
            let through_bytes = rng.below(2) == 0;
            run.before(op, format_args!("push_zeroed(), then write {value:?} into it"));
            let (len, capacity) = (c.len(), c.capacity());
            let row = c.push_zeroed();
            run.check(op, row == len, || format!("push_zeroed returned row {row}, the new row is {len}"));
            run.check(op, c.row(row).iter().all(|&b| b == 0), || format!("push_zeroed's row {row} is {}, not zeroed", hex(c.row(row))));
            if len == capacity {
                run.check(op, c.capacity() > capacity && c.capacity() >= capacity + capacity / 2, || {
                    format!("the column was full at {capacity} rows and grew to {}; it should grow by at least half", c.capacity())
                });
            } else {
                run.check(op, c.capacity() == capacity, || {
                    format!("capacity went from {capacity} to {} with room to spare", c.capacity())
                });
            }
            if through_bytes {
                c.row_mut(row).copy_from_slice(&value.bytes());
            } else {
                c.slice_mut::<T>()[row] = value;
            }
            model.push(value);
        } else if roll < 75 {
            let row = rng.below(model.len() as u64) as usize;
            run.before(op, format_args!("swap_remove({row}) of {} rows", model.len()));
            c.swap_remove(row);
            model.swap_remove(row);
        } else {
            let row = rng.below(model.len() as u64) as usize;
            let value = T::make(rng);
            run.before(op, format_args!("write {value:?} into row {row}"));
            if rng.below(2) == 0 {
                c.row_mut(row).copy_from_slice(&value.bytes());
            } else {
                c.slice_mut::<T>()[row] = value;
            }
            model[row] = value;
        }
        run.check(op, c.len() == model.len(), || format!("len() is {}, the model says {}", c.len(), model.len()));
        run.check(op, c.capacity() >= c.len(), || format!("capacity() {} is less than len() {}", c.capacity(), c.len()));
        if op.is_multiple_of(32) || model.len() < 4 {
            let view = c.slice::<T>();
            let at = view.as_ptr() as usize;
            run.check(op, at.is_multiple_of(align_of::<T>()), || format!("slice() starts at {at:#x}, not aligned to {}", align_of::<T>()));
            for (row, want) in model.iter().enumerate() {
                run.check(op, view[row] == *want, || format!("row {row} is {:?} through slice(), the model says {want:?}", view[row]));
                run.check(op, c.row(row) == want.bytes(), || {
                    format!("row {row} is {} through row(), the model says {}", hex(c.row(row)), hex(&want.bytes()))
                });
            }
        }
    }
}

fn fuzz(run: &Run) {
    run.fuzz();
    let mut rng = Rng::new(run.seed);
    let mut op = 0;
    let share = run.ops / 4;
    fuzz_type::<[u8; 3]>(run, &mut rng, &mut op, share);
    fuzz_type::<u32>(run, &mut rng, &mut op, share);
    fuzz_type::<Vec3>(run, &mut rng, &mut op, share);
    fuzz_type::<Vec4>(run, &mut rng, &mut op, run.ops - 3 * share);
    run.pass();
}

// The kata's main calls this.
pub fn run() {
    let run = Run::new("column", "4. a type-erased column");
    scenario(&run);
    fuzz(&run);
}
