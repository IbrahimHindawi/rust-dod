// The reference: a query that remembers how far down the list of tables it has
// looked, and looks only past that.

// `unsafe` is an error, except where an item allows it by name.
#![deny(unsafe_code)]

#[path = "../dojo.rs"]
mod dojo;
// The harness: main hands it the program, and it checks the code below.
mod harness;

// Every bit of `all`, and none of `none`.
fn matches(signature: u64, all: u64, none: u64) -> bool {
    (signature & all) == all && (signature & none) == 0
}

pub struct Query {
    all: u64,
    none: u64,
    // The tables that match, by index, in the order they were found.
    tables: Vec<u32>,
    // The watermark: tables 0 to checked - 1 have been looked at.
    checked: usize,
    // How many tables update has looked at, ever. With the watermark, each
    // table once.
    scanned: usize,
}

impl Query {
    pub fn new(all: u64, none: u64) -> Query {
        Query { all, none, tables: Vec::new(), checked: 0, scanned: 0 }
    }

    // Tables are only ever added, at the end, so the ones past the watermark
    // are the only ones this query hasn't seen.
    pub fn update(&mut self, signatures: &[u64]) {
        for (ti, &signature) in signatures.iter().enumerate().skip(self.checked) {
            self.scanned += 1;
            if matches(signature, self.all, self.none) {
                self.tables.push(ti as u32);
            }
        }
        self.checked = signatures.len();
    }

    // The matching tables, in increasing order.
    pub fn tables(&self) -> &[u32] {
        &self.tables
    }

    // How many tables the query has been updated over: the watermark.
    pub fn checked(&self) -> usize {
        self.checked
    }

    // How many tables update() has looked at, in all.
    pub fn scanned(&self) -> usize {
        self.scanned
    }
}

fn main() {
    harness::run();
}
