// Level 3: a blank page. Write the allocator from memory.
//
// Start it with: python tools/dojo.py start free_list 3
//
// The harness needs exactly this, and checks what each does:
//
//   use crate::{Dead, Handle};       Handle { index: u32, generation: NonZeroU32 }
//
//   pub struct Handles
//   Handles::new() -> Handles
//   alloc(&mut self) -> Handle                   reuses a freed index first
//   free(&mut self, h: Handle) -> Result<(), Dead>
//   alive(&self, h: Handle) -> bool
//   live(&self) -> usize                         handles alive now
//   slots(&self) -> usize                        indices handed out, alive or free
