//! A budgeted decode: each bound at its edge, each charge held to what the
//! allocator saw the decoded value keep, nesting, the charge before the read,
//! scopes, and a decode outside one as upstream's.
use sourcetrait_cereal_bitcode as bitcode;

use bitcode::{decode, decode_budgeted, encode, Budget, BudgetError, Decode, Encode, Exceeded};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, LinkedList, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

/// Counts the heap this thread holds, so a decode's allocations are read
/// apart from other tests' threads.
struct Counting;

std::thread_local! {
    static LIVE: Cell<isize> = const { Cell::new(0) };
}

fn held(delta: isize) {
    let _ = LIVE.try_with(|live| live.set(live.get() + delta));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            held(layout.size() as isize);
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) };
        held(-(layout.size() as isize));
    }

    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, layout, size) };
        if !q.is_null() {
            held(size as isize - layout.size() as isize);
        }
        q
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// A budget no input reaches.
const OPEN: Budget = Budget {
    elements: u64::MAX,
    heap: u64::MAX,
    depth: u32::MAX,
};

/// The bound a budgeted decode of `bytes` passed, panicking on anything else.
fn refused<T: for<'a> Decode<'a> + std::fmt::Debug>(bytes: &[u8], budget: Budget) -> Exceeded {
    match decode_budgeted::<T>(bytes, budget) {
        Err(BudgetError::Exceeded(bound)) => bound,
        other => panic!("not refused by a bound: {other:?}"),
    }
}

/// The heap the decoded value of `bytes` keeps, as the allocator counted it.
fn kept<T: for<'a> Decode<'a>>(bytes: &[u8]) -> u64 {
    let before = LIVE.with(Cell::get);
    let value = decode_budgeted::<T>(bytes, OPEN).expect("decoded");
    let after = LIVE.with(Cell::get);
    drop(value);
    (after - before) as u64
}

/// The least heap a budgeted decode of `bytes` passes with: the charge.
fn charged<T: for<'a> Decode<'a>>(bytes: &[u8]) -> u64 {
    let fits = |heap| decode_budgeted::<T>(bytes, Budget { heap, ..OPEN }).is_ok();
    assert!(fits(u64::MAX >> 1), "decodes at all");
    let (mut low, mut high) = (0u64, u64::MAX >> 1);
    while low < high {
        let mid = low + (high - low) / 2;
        if fits(mid) {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    low
}

/// Holds each charge to what the allocator saw the value keep.
fn covers<T: for<'a> Decode<'a> + Encode>(name: &str, value: &T) {
    let bytes = encode(value);
    let (kept, charged) = (kept::<T>(&bytes), charged::<T>(&bytes));
    println!("{name}: kept {kept}, charged {charged}");
    assert!(
        kept <= charged,
        "{name}: kept {kept} past its charge {charged}"
    );
}

/// A value aligned past its size's natural alignment.
#[derive(Encode, Decode, Debug, PartialEq, Clone, Copy)]
#[repr(align(64))]
struct Aligned([u8; 8]);

#[derive(Encode, Decode, Debug, PartialEq)]
struct Row {
    name: String,
    tags: Vec<String>,
    flag: Option<bool>,
}

fn rows(n: usize) -> Vec<Row> {
    (0..n)
        .map(|i| Row {
            name: "n".repeat(i % 7),
            tags: (0..i % 5).map(|j| j.to_string()).collect(),
            flag: (i % 3 != 0).then_some(i % 2 == 0),
        })
        .collect()
}

#[test]
fn each_charge_covers_what_the_decoded_value_keeps() {
    covers("Vec<u32>", &(0..1000u32).collect::<Vec<_>>());
    covers("Vec<u8>", &vec![7u8; 5000]);
    covers("String", &"x".repeat(3000));
    covers(
        "Vec<String>",
        &(0..300).map(|i| "s".repeat(i % 40)).collect::<Vec<_>>(),
    );
    covers("VecDeque<u16>", &(0..500u16).collect::<VecDeque<_>>());
    covers("BinaryHeap<u32>", &(0..500u32).collect::<BinaryHeap<_>>());
    covers("LinkedList<u8> of one", &LinkedList::from([9u8]));
    covers("LinkedList<u8>", &(0..100u8).collect::<LinkedList<_>>());
    covers(
        "LinkedList<Aligned>",
        &(0..10u8)
            .map(|b| Aligned([b; 8]))
            .collect::<LinkedList<_>>(),
    );
    covers("Box<u64>", &Box::new(41u64));
    covers("Rc<u8>", &Rc::new(9u8));
    covers("Rc<Aligned>", &Rc::new(Aligned([1; 8])));
    covers("Arc<u64>", &Arc::new(41u64));
    covers("Rc<[u16]>", &Rc::<[u16]>::from(vec![3u16; 100]));
    covers("Rc<[u8]> of three", &Rc::<[u8]>::from(vec![1u8, 2, 3]));
    covers("Rc<[Aligned]> empty", &Rc::<[Aligned]>::from(Vec::new()));
    covers(
        "Rc<[Aligned]> of one",
        &Rc::<[Aligned]>::from(vec![Aligned([1; 8])]),
    );
    covers(
        "Rc<[Aligned]>",
        &Rc::<[Aligned]>::from(vec![Aligned([2; 8]); 5]),
    );
    covers("Arc<[Aligned]> empty", &Arc::<[Aligned]>::from(Vec::new()));
    covers(
        "Arc<[Aligned]> of one",
        &Arc::<[Aligned]>::from(vec![Aligned([1; 8])]),
    );
    covers(
        "Arc<[Aligned]>",
        &Arc::<[Aligned]>::from(vec![Aligned([2; 8]); 5]),
    );
    covers("Rc<str>", &Rc::<str>::from("shared text"));
    covers("Arc<str>", &Arc::<str>::from("shared text"));
    covers("Box<str>", &Box::<str>::from("boxed text"));
    covers(
        "Box<[Aligned]>",
        &vec![Aligned([3; 8]); 3].into_boxed_slice(),
    );
    covers("Vec<Row>", &rows(64));
    covers(
        "Vec<Vec<Option<u64>>>",
        &(0..40).map(|i| vec![Some(i as u64); i]).collect::<Vec<_>>(),
    );
}

#[test]
fn a_vec_takes_its_elements_and_their_heap_exactly() {
    let v: Vec<u32> = (0..1000).collect();
    let bytes = encode(&v);
    let exact = Budget {
        elements: 1000,
        heap: 4000,
        depth: 1,
    };
    assert_eq!(decode_budgeted::<Vec<u32>>(&bytes, exact).unwrap(), v);
    let tight = |budget| refused::<Vec<u32>>(&bytes, budget);
    assert_eq!(
        tight(Budget {
            elements: 999,
            ..exact
        }),
        Exceeded::Elements
    );
    assert_eq!(
        tight(Budget {
            heap: 3999,
            ..exact
        }),
        Exceeded::Heap
    );
    assert_eq!(tight(Budget { depth: 0, ..exact }), Exceeded::Depth);
}

#[test]
fn bytes_and_strings_take_heap_alone() {
    let bytes = vec![7u8; 5000];
    let encoded = encode(&bytes);
    let exact = Budget {
        elements: 0,
        heap: 5000,
        depth: 1,
    };
    assert_eq!(decode_budgeted::<Vec<u8>>(&encoded, exact).unwrap(), bytes);
    assert_eq!(
        refused::<Vec<u8>>(
            &encoded,
            Budget {
                heap: 4999,
                ..exact
            }
        ),
        Exceeded::Heap
    );

    let signed = vec![-3i8; 300];
    let budget = Budget { heap: 300, ..exact };
    assert_eq!(
        decode_budgeted::<Vec<i8>>(&encode(&signed), budget).unwrap(),
        signed
    );

    // A string nests nothing, so it takes no level.
    let text = "x".repeat(3000);
    let encoded = encode(&text);
    let exact = Budget {
        elements: 0,
        heap: 3000,
        depth: 0,
    };
    assert_eq!(decode_budgeted::<String>(&encoded, exact).unwrap(), text);
    assert_eq!(
        refused::<String>(
            &encoded,
            Budget {
                heap: 2999,
                ..exact
            }
        ),
        Exceeded::Heap
    );
}

#[test]
fn a_linked_list_counts_each_node_a_byte_s_included() {
    let list: LinkedList<u8> = (0..100u8).collect();
    let encoded = encode(&list);
    let heap = charged::<LinkedList<u8>>(&encoded);
    let exact = Budget {
        elements: 100,
        heap,
        depth: 1,
    };
    assert_eq!(
        decode_budgeted::<LinkedList<u8>>(&encoded, exact).unwrap(),
        list
    );
    assert_eq!(
        refused::<LinkedList<u8>>(
            &encoded,
            Budget {
                elements: 99,
                ..exact
            }
        ),
        Exceeded::Elements
    );
    // A contiguous byte buffer stays a byte string: heap alone.
    let deque: VecDeque<u8> = (0..100u8).collect();
    let bytes = Budget {
        elements: 0,
        heap: 100,
        depth: 1,
    };
    assert_eq!(
        decode_budgeted::<VecDeque<u8>>(&encode(&deque), bytes).unwrap(),
        deque
    );
}

#[test]
fn a_b_tree_or_a_hash_collection_is_refused_inside_a_scope_alone() {
    let map: BTreeMap<u8, u16> = (0..20u8).map(|k| (k, u16::from(k))).collect();
    let hashed: HashMap<u8, u16> = map.iter().map(|(k, v)| (*k, *v)).collect();
    let set: BTreeSet<u8> = (0..20u8).collect();
    let hash_set: HashSet<u8> = (0..20u8).collect();
    let unbudgeted =
        |result: Result<(), BudgetError>| assert!(matches!(result, Err(BudgetError::Unbudgeted)));
    unbudgeted(decode_budgeted::<BTreeMap<u8, u16>>(&encode(&map), OPEN).map(drop));
    unbudgeted(decode_budgeted::<HashMap<u8, u16>>(&encode(&hashed), OPEN).map(drop));
    unbudgeted(decode_budgeted::<BTreeSet<u8>>(&encode(&set), OPEN).map(drop));
    unbudgeted(decode_budgeted::<HashSet<u8>>(&encode(&hash_set), OPEN).map(drop));
    // An absent one is no refusal, and outside a scope each decodes as upstream's.
    let none: Option<BTreeMap<u8, u16>> = None;
    assert_eq!(
        decode_budgeted::<Option<BTreeMap<u8, u16>>>(&encode(&none), OPEN).unwrap(),
        none
    );
    assert_eq!(decode::<BTreeMap<u8, u16>>(&encode(&map)).unwrap(), map);
    assert_eq!(decode::<HashSet<u8>>(&encode(&hash_set)).unwrap(), hash_set);
}

#[test]
fn a_pointer_takes_a_level() {
    let boxed = Box::new(41u64);
    let encoded = encode(&boxed);
    let exact = Budget {
        elements: 0,
        heap: 8,
        depth: 1,
    };
    assert_eq!(decode_budgeted::<Box<u64>>(&encoded, exact).unwrap(), boxed);
    assert_eq!(
        refused::<Box<u64>>(&encoded, Budget { heap: 7, ..exact }),
        Exceeded::Heap
    );
    assert_eq!(
        refused::<Box<u64>>(&encoded, Budget { depth: 0, ..exact }),
        Exceeded::Depth
    );
}

#[test]
fn an_absent_or_empty_collection_takes_no_level() {
    let none: Vec<Vec<u8>> = Vec::new();
    let flat = Budget { depth: 0, ..OPEN };
    assert_eq!(
        decode_budgeted::<Vec<Vec<u8>>>(&encode(&none), flat).unwrap(),
        none
    );
    let one_empty: Vec<Vec<u8>> = vec![Vec::new()];
    let one = Budget { depth: 1, ..OPEN };
    assert_eq!(
        decode_budgeted::<Vec<Vec<u8>>>(&encode(&one_empty), one).unwrap(),
        one_empty
    );
    assert_eq!(
        refused::<Vec<Vec<u8>>>(&encode(&one_empty), flat),
        Exceeded::Depth
    );
    let absent: Option<Vec<u8>> = None;
    assert_eq!(
        decode_budgeted::<Option<Vec<u8>>>(&encode(&absent), flat).unwrap(),
        absent
    );
    let present = Some(vec![1u8]);
    assert_eq!(
        decode_budgeted::<Option<Vec<u8>>>(&encode(&present), one).unwrap(),
        present
    );
    assert_eq!(
        refused::<Option<Vec<u8>>>(&encode(&present), flat),
        Exceeded::Depth
    );
}

#[test]
fn each_nested_collection_takes_a_level_and_gives_it_back() {
    let nested: Vec<Vec<Vec<u8>>> = vec![vec![vec![1, 2], vec![]], vec![vec![3]]];
    let encoded = encode(&nested);
    let deep = Budget { depth: 3, ..OPEN };
    assert_eq!(
        decode_budgeted::<Vec<Vec<Vec<u8>>>>(&encoded, deep).unwrap(),
        nested
    );
    assert_eq!(
        refused::<Vec<Vec<Vec<u8>>>>(&encoded, Budget { depth: 2, ..OPEN }),
        Exceeded::Depth
    );
    // Siblings share a level: two vectors side by side nest no deeper than one.
    let siblings = (vec![1u8], vec![2u16]);
    let one = Budget { depth: 1, ..OPEN };
    assert_eq!(
        decode_budgeted::<(Vec<u8>, Vec<u16>)>(&encode(&siblings), one).unwrap(),
        siblings
    );
}

#[test]
fn a_struct_s_collections_add_up() {
    let rows = rows(10);
    let encoded = encode(&rows);
    let tags: usize = rows.iter().map(|r| r.tags.len()).sum();
    let elements = (rows.len() + tags) as u64;
    let exact = Budget {
        elements,
        heap: charged::<Vec<Row>>(&encoded),
        depth: 2,
    };
    assert_eq!(decode_budgeted::<Vec<Row>>(&encoded, exact).unwrap(), rows);
    let tight = |budget| refused::<Vec<Row>>(&encoded, budget);
    assert_eq!(
        tight(Budget {
            elements: elements - 1,
            ..exact
        }),
        Exceeded::Elements
    );
    assert_eq!(
        tight(Budget {
            heap: exact.heap - 1,
            ..exact
        }),
        Exceeded::Heap
    );
    assert_eq!(tight(Budget { depth: 1, ..exact }), Exceeded::Depth);
}

#[test]
fn a_bound_refuses_a_collection_before_its_elements_are_read() {
    let encoded = encode(&vec![0u64; 100_000]);
    let tight = Budget {
        elements: 10,
        ..OPEN
    };
    // The shortest prefix the bound refuses still lacks the elements, which
    // the unbudgeted decode reads and finds missing.
    let cut = (1..encoded.len())
        .find(|&end| {
            matches!(
                decode_budgeted::<Vec<u64>>(&encoded[..end], tight),
                Err(BudgetError::Exceeded(Exceeded::Elements))
            )
        })
        .expect("a prefix the bound refuses");
    assert!(cut < encoded.len());
    assert!(decode::<Vec<u64>>(&encoded[..cut]).is_err());
    assert!(matches!(
        decode_budgeted::<Vec<u64>>(&encoded[..cut], OPEN),
        Err(BudgetError::Decode(_))
    ));
}

#[test]
fn a_malformed_input_is_its_own_error_not_a_bound() {
    let mut encoded = encode(&vec![1u32, 2, 3]);
    encoded.push(0);
    assert!(matches!(
        decode_budgeted::<Vec<u32>>(&encoded, OPEN),
        Err(BudgetError::Decode(_))
    ));
}

#[test]
fn a_decode_outside_a_scope_charges_nothing() {
    let big: Vec<u32> = (0..50_000).collect();
    let encoded = encode(&big);
    assert_eq!(decode::<Vec<u32>>(&encoded).unwrap(), big);
    // A refused decode leaves the thread without a budget.
    assert_eq!(
        refused::<Vec<u32>>(
            &encoded,
            Budget {
                elements: 1,
                ..OPEN
            }
        ),
        Exceeded::Elements
    );
    assert_eq!(decode::<Vec<u32>>(&encoded).unwrap(), big);
}

#[test]
fn a_refused_decode_leaves_the_next_one_its_whole_budget() {
    let small = encode(&vec![9u16; 10]);
    let large = encode(&vec![9u16; 11]);
    let budget = Budget {
        elements: 10,
        heap: 20,
        depth: 1,
    };
    for _ in 0..3 {
        assert_eq!(refused::<Vec<u16>>(&large, budget), Exceeded::Elements);
        assert_eq!(
            decode_budgeted::<Vec<u16>>(&small, budget).unwrap(),
            vec![9u16; 10]
        );
    }
}

#[test]
fn each_thread_holds_its_own_scope() {
    let threads: Vec<_> = (0..8u64)
        .map(|t| {
            std::thread::spawn(move || {
                let n = 100 + t as usize;
                let encoded = encode(&vec![t as u8; n]);
                let fits = Budget {
                    elements: 0,
                    heap: n as u64,
                    depth: 1,
                };
                for round in 0..200 {
                    if (round + t) % 2 == 0 {
                        assert_eq!(decode_budgeted::<Vec<u8>>(&encoded, fits).unwrap().len(), n);
                    } else {
                        let short = Budget {
                            heap: n as u64 - 1,
                            ..fits
                        };
                        assert_eq!(refused::<Vec<u8>>(&encoded, short), Exceeded::Heap);
                    }
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().expect("a thread's decodes");
    }
}

#[test]
fn a_budgeted_decode_reads_what_upstream_s_reads() {
    let rows = rows(64);
    let encoded = encode(&rows);
    assert_eq!(
        decode_budgeted::<Vec<Row>>(&encoded, OPEN).unwrap(),
        decode::<Vec<Row>>(&encoded).unwrap()
    );
    let lists: Vec<LinkedList<u32>> = (0..20).map(|i| (0..i).collect()).collect();
    let encoded = encode(&lists);
    assert_eq!(
        decode_budgeted::<Vec<LinkedList<u32>>>(&encoded, OPEN).unwrap(),
        decode::<Vec<LinkedList<u32>>>(&encoded).unwrap()
    );
}
